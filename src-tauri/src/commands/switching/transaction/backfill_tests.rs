use super::*;
use crate::config_store::ConfigStore;
use std::fs;

struct Fixture {
    gateway: GatewayController,
    state: LocalState,
    _state_dir: tempfile::TempDir,
    _client_paths: crate::test_client_paths::ClientPathGuard,
    target: PathBuf,
    source: asb_core::contracts::CodexProviderFile,
    next: asb_core::contracts::CodexProviderFile,
    before_config: String,
}

impl Fixture {
    fn new() -> Self {
        let client_paths = crate::test_client_paths::redirect_client_paths();
        let state_dir = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(state_dir.path().join("state"));
        let gateway = GatewayController::start(&state);
        let store = state.configuration();
        let create = |name: &str| {
            let mut draft = crate::codex_common::test_draft();
            draft.name = name.into();
            let id = uuid::Uuid::new_v4().to_string();
            store.create_codex_provider(id.clone(), draft).unwrap();
            store.find_codex_provider_file(&id).unwrap()
        };
        let source = create("Original provider");
        let next = create("Next provider");
        let target = state.target(AppKind::Codex).unwrap();
        fs::write(&target, "model_provider = 'openai'\n").unwrap();
        fs::write(target.with_file_name("auth.json"),
            r#"{"auth_mode":"chatgpt","tokens":{"access_token":"at","refresh_token":"rt","id_token":"id"}}"#).unwrap();
        let plan = super::super::plan::build_codex_plan(&state, &gateway, source.clone()).unwrap();
        let preview = super::super::plan::preview_projection(&state, &plan).unwrap();
        super::super::plan::execute_projection(
            &state, &gateway, &plan, &preview.content_hash, &preview.rendered_hash,
        ).unwrap();
        let text = fs::read_to_string(&target).unwrap();
        let mut document = text.parse::<toml_edit::DocumentMut>().unwrap();
        document["model_context_window"] = toml_edit::value(source.profile.catalog[0].context_window as i64 + 1);
        let before_config = document.to_string();
        fs::write(&target, &before_config).unwrap();
        Self { gateway, state, _state_dir: state_dir, _client_paths: client_paths, target, source, next, before_config }
    }

    fn legacy_backfill(&self) -> SwitchIntent {
        let mut backfill = super::super::codex_backfill::prepare(
            &self.state, &self.gateway, &self.next.profile.id,
            &asb_switch::sha256_hex(&self.before_config),
        ).unwrap().expect("changed context window is backfilled");
        backfill.after_hash = asb_switch::sha256_hex(&serde_json::to_string_pretty(&backfill.candidate).unwrap());
        let auth = fs::read_to_string(self.target.with_file_name("auth.json")).unwrap();
        let auth_hash = asb_switch::sha256_hex(&auth);
        let catalog = CatalogArtifact::new(self.staged_catalog_path(), None, Some("{}".into()));
        begin_with_codex_backfill_and_auth(
            &self.state, &self.gateway, AppKind::Codex, Some(&self.next.profile.id),
            &asb_switch::sha256_hex("new config"), true, Some(catalog), Some(&backfill),
            Some(AuthIntent { before_hash: auth_hash.clone(), before_existed: true, after_hash: auth_hash, after_existed: true }),
        ).unwrap();
        self.state.configuration().update_codex_provider_file(backfill.candidate, &backfill.before_hash).unwrap();
        load(&self.state).unwrap().unwrap()
    }

    fn staged_catalog_path(&self) -> PathBuf {
        self.target.with_file_name("agent-switchboard-codex-unpublished.json")
    }

    fn source_path(&self) -> PathBuf {
        self.state.configuration().providers_dir(AppKind::Codex).join(format!("{}.json", self.source.profile.id))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.gateway.shutdown();
    }
}

#[test]
fn codex_backfill_revision_matches_the_profile_written_during_switch() {
    let f = Fixture::new();
    let plan = super::super::plan::build_codex_plan(&f.state, &f.gateway, f.next.clone()).unwrap();
    let preview = super::super::plan::preview_projection(&f.state, &plan).unwrap();
    let prepared = super::super::codex_backfill::prepare(&f.state, &f.gateway, &f.next.profile.id, &preview.content_hash)
        .unwrap().expect("outgoing profile needs backfill");
    super::super::plan::execute_projection(&f.state, &f.gateway, &plan, &preview.content_hash, &preview.rendered_hash).unwrap();
    let (stored, revision) = f.state.configuration().find_codex_provider_with_revision(&f.source.profile.id).unwrap();
    assert_eq!(stored, prepared.candidate);
    assert_eq!(revision, prepared.after_hash);
    assert_eq!(revision, ConfigStore::codex_provider_revision(&stored).unwrap());
    assert!(load(&f.state).unwrap().is_none());
}

#[test]
fn legacy_codex_backfill_recovers_without_rewriting_client_files() {
    let f = Fixture::new();
    let intent = f.legacy_backfill();
    let auth_path = f.target.with_file_name("auth.json");
    let auth = fs::read(&auth_path).unwrap();
    // Simulate the route being discarded on startup while the archive still
    // contains the backfilled provider, whose route revision no longer matches.
    f.gateway.restore_activation_snapshot(&f.state, &GatewayActivationSnapshot { app: AppKind::Codex, route: None }).unwrap();
    recover(&f.state, &f.gateway).unwrap();
    assert_eq!(f.state.configuration().find_codex_provider_file(&f.source.profile.id).unwrap(), f.source);
    assert_eq!(fs::read_to_string(&f.target).unwrap(), f.before_config);
    assert_eq!(fs::read(&auth_path).unwrap(), auth);
    assert_eq!(f.state.configuration().latest_config_write(AppKind::Codex).unwrap(), intent.previous_write);
    assert_eq!(f.gateway.active_profile_id(AppKind::Codex, &f.before_config).unwrap(), Some(f.source.profile.id.clone()));
    assert!(load(&f.state).unwrap().is_none());
    assert!(!f.staged_catalog_path().exists());

    // An interruption after restoring the profile and before clearing the
    // intent must also be recoverable, without rewriting the profile.
    journal::save(&f.state, &intent).unwrap();
    recover(&f.state, &f.gateway).unwrap();
    recover(&f.state, &f.gateway).unwrap();
    assert!(recovery_status(&f.state).is_none());
}

#[test]
fn legacy_codex_backfill_does_not_overwrite_a_changed_profile_or_catalog() {
    let f = Fixture::new();
    f.legacy_backfill();
    let profile = f.source_path();
    let written = fs::read_to_string(&profile).unwrap();
    let changed = format!("{written}\n");
    fs::write(&profile, &changed).unwrap();
    assert!(recover(&f.state, &f.gateway).unwrap_err().contains("拒绝覆盖"));
    assert_eq!(fs::read_to_string(&profile).unwrap(), changed);
    assert!(load(&f.state).unwrap().is_some());

    fs::write(&profile, &written).unwrap();
    let catalog = f.staged_catalog_path();
    fs::write(&catalog, "{\"changed\":true}").unwrap();
    assert!(recover(&f.state, &f.gateway).unwrap_err().contains("模型目录已变化"));
    assert_eq!(fs::read_to_string(&profile).unwrap(), written);
    assert_eq!(fs::read_to_string(&catalog).unwrap(), "{\"changed\":true}");
    assert!(load(&f.state).unwrap().is_some());
}

#[test]
fn legacy_codex_backfill_preserves_changed_client_or_authentication_files() {
    let f = Fixture::new();
    f.legacy_backfill();
    let profile = fs::read(f.source_path()).unwrap();
    let auth_path = f.target.with_file_name("auth.json");
    let auth = fs::read_to_string(&auth_path).unwrap();

    for (path, original) in [(&f.target, f.before_config.as_str()), (&auth_path, auth.as_str())] {
        let changed = format!("{original}\n");
        fs::write(path, &changed).unwrap();
        assert!(recover(&f.state, &f.gateway).unwrap_err().contains("安全回滚条件"));
        assert_eq!(fs::read_to_string(path).unwrap(), changed);
        assert_eq!(fs::read(f.source_path()).unwrap(), profile);
        assert!(load(&f.state).unwrap().is_some());
        fs::write(path, original).unwrap();
    }
}
