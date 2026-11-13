use super::prepare;
use crate::config_store::ConfigStore;
use crate::local_state::LocalState;
use asb_core::contracts::{AppKind, CodexProviderRecord, ProviderRecord};
use asb_core::provider_conversion::{
    ConversionStatus, ProviderConversion, ProviderConversionRequest, ProviderConversionSeed,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn required<T, E>(result: Result<T, E>, operation: &'static str) -> T {
    match result {
        Ok(value) => value,
        Err(_) => panic!("isolated provider conversion failed: {operation}; sensitive input withheld"),
    }
}

fn verified_workspace_path(path: &Path) -> PathBuf {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let ignored = required(workspace.join(".tmp").canonicalize(), "resolve ignored sandbox");
    let canonical = required(path.canonicalize(), "resolve fixture directory");
    assert!(canonical.starts_with(&ignored) && canonical != ignored, "fixtures must stay under the ignored workspace sandbox");
    canonical
}

fn copy_provider_files(source: &Path, destination: &Path) {
    for relative in ["providers/codex", "providers/codex/official", "providers/claude"] {
        let source_dir = source.join(relative);
        if !source_dir.exists() { continue; }
        let source_dir = verified_workspace_path(&source_dir);
        let target_dir = destination.join(relative);
        required(fs::create_dir_all(&target_dir), "create isolated provider directory");
        for entry in required(fs::read_dir(source_dir), "read copied providers") {
            let entry = required(entry, "read copied provider entry");
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") { continue; }
            assert!(required(entry.file_type(), "inspect copied provider").is_file(), "provider fixture must be a regular file");
            required(fs::copy(entry.path(), target_dir.join(entry.file_name())), "copy provider into writable test state");
        }
    }
}

fn fingerprints(root: &Path) -> BTreeMap<PathBuf, String> {
    fn visit(root: &Path, path: &Path, hashes: &mut BTreeMap<PathBuf, String>) {
        if !path.exists() { return; }
        for entry in required(fs::read_dir(path), "read isolated state fingerprint") {
            let entry = required(entry, "read isolated state entry");
            let kind = required(entry.file_type(), "inspect isolated state entry");
            assert!(!kind.is_symlink(), "isolated state cannot contain symbolic links");
            if kind.is_dir() { visit(root, &entry.path(), hashes); }
            else {
                let bytes = required(fs::read(entry.path()), "read isolated state bytes");
                hashes.insert(entry.path().strip_prefix(root).unwrap().to_path_buf(), format!("{:x}", Sha256::digest(bytes)));
            }
        }
    }
    let mut hashes = BTreeMap::new();
    visit(root, root, &mut hashes);
    hashes
}

fn request(app: AppKind, id: &str, hash: &str) -> ProviderConversionRequest {
    ProviderConversionRequest {
        source_app: app,
        source_profile_id: id.into(),
        expected_source_file_hash: hash.into(),
        target_app: match app { AppKind::Codex => AppKind::Claude, AppKind::Claude => AppKind::Codex },
    }
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Acceptance {
    source_codex: usize,
    source_claude: usize,
    converted_to_claude: usize,
    converted_to_codex: usize,
    blocked: usize,
    needs_input: usize,
    duplicate_names_created_independently: usize,
    stale_sources_rejected: usize,
    source_and_existing_files_unchanged: bool,
    no_client_or_gateway_writes: bool,
    real_upstream_requests: usize,
    completion_choices: &'static str,
}

fn check_preparation_guards(state: &LocalState, source: ProviderConversionRequest) {
    let mut stale = source.clone();
    stale.expected_source_file_hash = "not-the-current-source-revision".into();
    assert!(prepare(state, stale).is_err(), "stale source must fail before generating a seed");
    let mut same_client = source.clone();
    same_client.target_app = same_client.source_app;
    assert!(prepare(state, same_client).is_err(), "same-client conversion must fail");
    let mut missing = source;
    missing.source_profile_id = "00000000-0000-4000-8000-000000000000".into();
    assert!(prepare(state, missing).is_err(), "missing source must fail");
}

fn check_changed_source(state: &LocalState, input: &ProviderConversionRequest) {
    let directory = state.configuration().providers_dir(input.source_app);
    let ordinary = directory.join(format!("{}.json", input.source_profile_id));
    let path = if ordinary.is_file() { ordinary }
        else { directory.join("official").join(format!("{}.json", input.source_profile_id)) };
    let original = required(fs::read(&path), "read isolated source revision");
    let mut changed = original.clone();
    changed.push(b'\n');
    required(fs::write(&path, changed), "change only sandbox source revision");
    let stale = prepare(state, input.clone()).is_err();
    required(fs::write(&path, original), "restore sandbox source bytes");
    assert!(stale, "a source modified after selection must invalidate preparation");
}

fn client_sentinels() -> Vec<(PathBuf, BTreeMap<PathBuf, String>)> {
    [("CODEX_HOME", "config.toml", "# isolated conversion acceptance\n"),
        ("CLAUDE_CONFIG_DIR", "settings.json", "{\"isolatedConversionAcceptance\":true}\n")]
        .into_iter().map(|(variable, filename, content)| {
            let directory = std::env::var_os(variable).map(PathBuf::from).expect("client path isolation is mandatory");
            let directory = verified_workspace_path(&directory);
            let file = directory.join(filename);
            if !file.exists() { required(fs::write(file, content), "create isolated client sentinel"); }
            let hashes = fingerprints(&directory);
            (directory, hashes)
        }).collect()
}

fn classify(prepared: &ProviderConversion, report: &mut Acceptance) -> bool {
    match prepared.status {
        ConversionStatus::Blocked => {
            report.blocked += 1;
            assert!(prepared.seed.is_none(), "unsupported credentials must not produce an editable key seed");
            false
        }
        ConversionStatus::NeedsInput => { report.needs_input += 1; true }
        ConversionStatus::Ready => true,
    }
}

fn create_claude_twice(store: &ConfigStore, source: &CodexProviderRecord, prepared: ProviderConversion) {
    let Some(ProviderConversionSeed::Claude { draft }) = prepared.seed else { panic!("expected Claude conversion seed") };
    assert!(draft.api_key == source.profile.api_key, "static credentials must be preserved without logging them");
    assert!(draft.base_url.as_deref() == Some(source.profile.endpoint.0.as_str()), "upstream endpoint must be preserved");
    assert!(draft.upstream_protocol == Some(source.profile.upstream.protocol()), "upstream protocol must be preserved");
    assert!(draft.usage_query.is_none(), "usage query copy must be opt-in");
    assert!(draft.model.as_ref().is_some_and(|model| source.profile.resolve_model(&source.profile.default_model).ok().as_ref() == Some(model)),
        "default aliases must resolve to the real upstream model");
    let first_id = uuid::Uuid::new_v4().to_string();
    let second_id = uuid::Uuid::new_v4().to_string();
    let first = required(store.create_provider(first_id.clone(), draft.clone()), "create converted Claude provider");
    let second = required(store.create_provider(second_id.clone(), draft), "create same-name Claude provider independently");
    assert!(first.profile.id == first_id && second.profile.id == second_id, "create must retain its detached editor identity");
    assert!(first.profile.id != source.profile.id && second.profile.id != first.profile.id, "new identities must be independent");
    assert!(first.profile.name == second.profile.name && second.position > first.position, "same names must not overwrite existing rows");
    let read = required(store.find_provider_record(&first.profile.id), "read converted Claude provider");
    assert!(read.profile == first.profile, "converted Claude provider must round-trip through its current store");
}

fn create_codex_twice(store: &ConfigStore, source: &ProviderRecord, prepared: ProviderConversion) {
    let Some(ProviderConversionSeed::Codex { mut draft }) = prepared.seed else { panic!("expected Codex conversion seed") };
    assert!(draft.api_key == source.profile.api_key, "static credentials must be preserved without logging them");
    assert!(Some(draft.endpoint.0.as_str()) == source.profile.base_url.as_deref(), "upstream endpoint must be preserved");
    assert!(Some(draft.upstream.protocol()) == source.profile.upstream_protocol, "upstream protocol must be preserved");
    assert!(draft.usage_query.is_none(), "usage query copy must be opt-in");
    assert!(draft.clone().into_draft().is_err(), "unknown model limits must not silently turn into saved defaults");
    for entry in &mut draft.catalog {
        entry.context_window = Some(entry.context_window.unwrap_or(32_768));
        entry.max_output_tokens = Some(entry.max_output_tokens.unwrap_or(2_048));
    }
    assert!(!draft.catalog.is_empty(), "the copied source must offer at least one explicit model candidate");
    if draft.default_model.is_empty() { draft.default_model = draft.catalog[0].id.clone(); }
    let draft = required(draft.into_draft(), "complete conversion with explicit sandbox-only limit choices");
    let first_id = uuid::Uuid::new_v4().to_string();
    let second_id = uuid::Uuid::new_v4().to_string();
    let first = required(store.create_codex_provider(first_id.clone(), draft.clone()), "create converted Codex provider");
    let second = required(store.create_codex_provider(second_id.clone(), draft), "create same-name Codex provider independently");
    assert!(first.profile.id == first_id && second.profile.id == second_id, "create must retain its detached editor identity");
    assert!(first.profile.id != source.profile.id && second.profile.id != first.profile.id, "new identities must be independent");
    assert!(first.profile.name == second.profile.name && second.position > first.position, "same names must not overwrite existing rows");
    let listed = required(store.list_codex_providers(), "read converted Codex provider");
    assert!(listed.iter().any(|record| record.profile == first.profile), "converted Codex provider must round-trip through its strict store");
}

fn convert_real_records(state: &LocalState, report: &mut Acceptance) {
    let store = state.configuration();
    let codex = required(store.list_codex_providers(), "load copied real Codex sources");
    let generic = required(store.list_providers(), "load copied real Claude and official sources");
    report.source_codex = codex.len() + generic.iter().filter(|record| record.profile.app == AppKind::Codex).count();
    report.source_claude = generic.iter().filter(|record| record.profile.app == AppKind::Claude).count();
    for source in codex {
        let input = request(AppKind::Codex, &source.profile.id, &source.file_hash);
        check_preparation_guards(state, input.clone());
        check_changed_source(state, &input);
        report.stale_sources_rejected += 1;
        let prepared = required(prepare(state, input), "prepare copied real Codex provider");
        if !classify(&prepared, report) { continue; }
        create_claude_twice(&store, &source, prepared);
        report.converted_to_claude += 1;
        report.duplicate_names_created_independently += 1;
    }
    for source in generic {
        let input = request(source.profile.app, &source.profile.id, &source.file_hash);
        check_preparation_guards(state, input.clone());
        check_changed_source(state, &input);
        report.stale_sources_rejected += 1;
        let prepared = required(prepare(state, input), "prepare copied real generic provider");
        if !classify(&prepared, report) { continue; }
        assert!(source.profile.app == AppKind::Claude, "Codex official login must never convert as a static provider");
        create_codex_twice(&store, &source, prepared);
        report.converted_to_codex += 1;
        report.duplicate_names_created_independently += 1;
    }
}

#[test]
#[ignore = "requires explicitly copied real provider documents in an isolated sandbox"]
fn copied_real_provider_files_convert_and_round_trip_without_touching_existing_data() {
    let fixture = std::env::var_os("ASB_PROVIDER_CONVERSION_REAL_CONFIGURATION")
        .map(PathBuf::from).expect("real provider fixture path must be explicitly supplied");
    let fixture = verified_workspace_path(&fixture);
    let fixture_before = fingerprints(&fixture);
    let client_before = client_sentinels();
    let directory = required(tempfile::tempdir(), "create isolated real-data acceptance state");
    verified_workspace_path(directory.path());
    let state = LocalState::from_root(directory.path().join("state"));
    copy_provider_files(&fixture, &state.configuration().configuration_dir());
    required(state.initialize_configuration_store(), "initialize copied current-format provider store");
    let before = fingerprints(directory.path());
    let mut report = Acceptance { completion_choices: "Explicit sandbox choices only: missing contextWindow=32768, maxOutputTokens=2048; conservative declared capabilities; no vendor support inferred", ..Default::default() };
    convert_real_records(&state, &mut report);
    assert!(report.converted_to_codex > 0 && report.converted_to_claude > 0, "real copies must exercise both conversion directions");
    let after = fingerprints(directory.path());
    assert!(before.iter().all(|(path, hash)| after.get(path) == Some(hash)), "all source and existing provider files must remain byte-identical");
    assert!(fingerprints(&fixture) == fixture_before, "the read-only fixture snapshot must remain byte-identical");
    assert!(client_before.iter().all(|(path, hashes)| fingerprints(path) == *hashes), "isolated client configuration sentinels must remain byte-identical");
    let expected_new_files = (report.converted_to_codex + report.converted_to_claude) * 2;
    assert_eq!(after.len(), before.len() + expected_new_files, "only the new provider documents may be written");
    assert!(after.keys().filter(|path| !before.contains_key(*path)).all(|path| path.starts_with("state/configuration/providers")),
        "conversion save cannot write client, history, queue, or gateway files");
    report.source_and_existing_files_unchanged = true;
    report.no_client_or_gateway_writes = true;
    let output = std::env::var_os("ASB_PROVIDER_CONVERSION_EVIDENCE").map(PathBuf::from)
        .expect("sanitized acceptance evidence path must be supplied");
    verified_workspace_path(output.parent().expect("evidence has a parent"));
    required(fs::write(output, required(serde_json::to_vec_pretty(&report), "serialize sanitized counts")), "save sanitized acceptance counts");
}
