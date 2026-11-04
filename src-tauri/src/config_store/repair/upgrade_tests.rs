use super::*;
use asb_core::contracts::{
    CodexCatalogEntry, CodexEndpoint, CodexProviderFile, CodexProviderProfile, CodexRouteMode,
    CodexUpstream, ProviderConnectionOptions, ProviderEndpoint, ProviderProfile,
    ResponsesRequestMode, CODEX_PROVIDER_SCHEMA_VERSION, DEFAULT_CODEX_CAPABILITIES,
};
use serde_json::json;

fn provider(position: u64) -> ProviderFile {
    let profile: ProviderProfile = serde_json::from_value(json!({
        "id": Uuid::new_v4().to_string(), "app": "claude", "routeMode": "custom",
        "name": format!("Saved provider {position}"), "model": "saved-model",
        "baseUrl": "https://relay.example", "apiKey": "isolated-saved-key",
        "upstreamProtocol": "anthropicMessages", "maxOutputTokens": null,
        "parameters": default_provider_parameters(AppKind::Claude),
        "claudeFragment": {"includeCoAuthoredBy": false},
        "notes": "saved notes", "websiteUrl": "https://relay.example"
    })).unwrap();
    ProviderFile::from_profile(&profile, position)
}

fn write_provider(store: &ConfigStore, value: &Value) -> (PathBuf, String) {
    let path = store.providers_dir(AppKind::Claude)
        .join(format!("{}.json", value["id"].as_str().unwrap()));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let original = serde_json::to_string_pretty(value).unwrap();
    fs::write(&path, &original).unwrap();
    (path, original)
}

#[test]
fn retired_automatic_effort_is_repaired_without_losing_saved_providers() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let mut originals = Vec::new();
    let mut expected = Vec::new();
    for position in [100, 200, 300, 400] {
        let file = provider(position);
        let mut old = serde_json::to_value(&file).unwrap();
        old["parameters"]["settings"].as_object_mut().unwrap()
            .insert("effortLevel".into(), json!({"mode": "automatic"}));
        old["parameters"]["settings"].as_object_mut().unwrap().remove("fastMode");
        originals.push(write_provider(&store, &old));
        expected.push(file);
    }
    assert!(store.list_providers().is_err());

    let report = store.repair().unwrap();
    let backup = PathBuf::from(report.backup_path.unwrap());
    assert_eq!(report.repaired_files.len(), 4);
    let records = store.list_providers().unwrap();
    assert_eq!(records.len(), 4);
    for ((record, file), (path, original)) in records.iter().zip(expected).zip(originals) {
        assert_eq!(record.position, file.position);
        assert!(record.profile == file.into_profile(AppKind::Claude));
        let relative = path.strip_prefix(store.configuration_dir()).unwrap();
        assert_eq!(fs::read_to_string(backup.join(relative)).unwrap(), original);
        let saved: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert!(saved["parameters"]["settings"].get("effortLevel").is_none());
    }
    let repeated = store.repair().unwrap();
    assert!(repeated.repaired_files.is_empty());
    assert!(repeated.backup_path.is_none());
}

#[test]
fn retired_effort_only_change_is_persisted_even_without_missing_settings() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let mut old = serde_json::to_value(provider(100)).unwrap();
    old["parameters"]["settings"]["effortLevel"] = json!({"mode": "automatic"});
    write_provider(&store, &old);
    let report = store.repair().unwrap();
    assert_eq!(report.repaired_files.len(), 1);
    assert_eq!(store.list_providers().unwrap().len(), 1);
}

#[test]
fn conflicting_effort_rejects_repair_without_changing_any_file() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let mut automatic = serde_json::to_value(provider(100)).unwrap();
    automatic["parameters"]["settings"]["effortLevel"] = json!({"mode": "automatic"});
    write_provider(&store, &automatic);
    let mut explicit = serde_json::to_value(provider(200)).unwrap();
    explicit["parameters"]["settings"]["effortLevel"] = json!({"mode": "explicit", "value": "high"});
    explicit["claudeFragment"]["effortLevel"] = json!("low");
    write_provider(&store, &explicit);
    let before = file_tree(&store.configuration_dir()).unwrap();
    assert!(store.repair().unwrap_err().contains("effortLevel"));
    assert_eq!(file_tree(&store.configuration_dir()).unwrap(), before);
    assert_eq!(fs::read_dir(store.state_root()).unwrap().count(), 1);
}

#[test]
fn arbitrary_unknown_automatic_keys_are_never_discarded() {
    let mut settings = serde_json::to_value(default_provider_parameters(AppKind::Claude)).unwrap();
    settings["settings"]["unknownSavedOption"] = json!({"mode": "automatic"});
    assert!(!fill_automatic(&mut settings, AppKind::Claude, true).unwrap());
    let decoded: SettingsValues = serde_json::from_value(settings).unwrap();
    assert!(decoded.validate_provider_parameters(AppKind::Claude).is_err());
}

#[test]
fn version_prefixed_anthropic_bases_and_endpoint_candidates_are_repaired() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let mut prefixed_base = serde_json::to_value(provider(100)).unwrap();
    prefixed_base["baseUrl"] = json!("https://relay.example/v1");
    let (base_path, base_original) = write_provider(&store, &prefixed_base);
    let mut prefixed_endpoint = serde_json::to_value(provider(200)).unwrap();
    prefixed_endpoint["connection"]["customEndpoints"]["https://mirror.example/compatible-mode/v1"] =
        json!({"url": "https://mirror.example/compatible-mode/v1", "addedAt": 1});
    prefixed_endpoint["connection"]["customEndpoints"]["https://healthy.example/proxy"] =
        json!({"url": "https://healthy.example/proxy", "addedAt": 2, "lastUsed": 7});
    let (endpoint_path, endpoint_original) = write_provider(&store, &prefixed_endpoint);
    assert!(store.list_providers().is_err());

    let report = store.repair().unwrap();

    assert_eq!(report.repaired_files.len(), 2);
    let records = store.list_providers().unwrap();
    assert_eq!(records.len(), 2);
    let base_record = records
        .iter()
        .find(|record| Some(record.profile.id.as_str()) == prefixed_base["id"].as_str())
        .unwrap();
    assert_eq!(base_record.profile.base_url.as_deref(), Some("https://relay.example/"));
    let endpoint_record = records
        .iter()
        .find(|record| Some(record.profile.id.as_str()) == prefixed_endpoint["id"].as_str())
        .unwrap();
    assert_eq!(endpoint_record.profile.base_url.as_deref(), Some("https://relay.example"));
    let kept = &endpoint_record.profile.connection.custom_endpoints;
    assert_eq!(kept.len(), 1);
    assert_eq!(
        kept["https://healthy.example/proxy"],
        ProviderEndpoint {
            url: "https://healthy.example/proxy".to_string(),
            added_at: 2,
            last_used: Some(7),
        }
    );
    let backup = PathBuf::from(report.backup_path.unwrap());
    for (path, original) in [(base_path, base_original), (endpoint_path, endpoint_original)] {
        let relative = path.strip_prefix(store.configuration_dir()).unwrap();
        assert_eq!(fs::read_to_string(backup.join(relative)).unwrap(), original);
    }
    let repeated = store.repair().unwrap();
    assert!(repeated.repaired_files.is_empty());
    assert!(repeated.backup_path.is_none());
}

#[test]
fn full_url_anthropic_bases_keep_their_version_segments_during_repair() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let mut full_url = serde_json::to_value(provider(100)).unwrap();
    full_url["baseUrl"] = json!("https://relay.example/v1");
    full_url["connection"]["isFullUrl"] = json!(true);
    full_url["connection"]["customEndpoints"]["https://mirror.example/v1"] =
        json!({"url": "https://mirror.example/v1", "addedAt": 1});
    write_provider(&store, &full_url);
    assert!(store.list_providers().is_ok());
    let before = file_tree(&store.configuration_dir()).unwrap();

    let report = store.repair().unwrap();

    assert!(report.repaired_files.is_empty());
    assert!(report.backup_path.is_none());
    assert_eq!(file_tree(&store.configuration_dir()).unwrap(), before);
}

#[test]
fn openai_version_prefixed_bases_and_candidates_are_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let mut openai = serde_json::to_value(provider(100)).unwrap();
    openai["baseUrl"] = json!("https://relay.example/v1");
    openai["upstreamProtocol"] = json!("chatCompletions");
    openai["connection"]["customEndpoints"]["https://mirror.example/v1"] =
        json!({"url": "https://mirror.example/v1", "addedAt": 1});
    write_provider(&store, &openai);
    assert!(store.list_providers().is_ok());
    let before = file_tree(&store.configuration_dir()).unwrap();

    let report = store.repair().unwrap();

    assert!(report.repaired_files.is_empty());
    assert!(report.backup_path.is_none());
    assert_eq!(file_tree(&store.configuration_dir()).unwrap(), before);
}

#[test]
fn other_invalid_url_shapes_abort_repair_without_changing_any_file() {
    for invalid_url in [
        " https://relay.example/v1",
        "https://relay.example/v1\n",
        "https://relay.example\\proxy/v1",
        "https://user:password@relay.example/v1",
        "https://relay.example/v1?key=value",
        "https://relay.example/v1#fragment",
        "https:///relay.example/v1",
        "https://relay.example/v1/v1",
        "https://relay.example/v1/messages/v1",
    ] {
        for candidate in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let store = ConfigStore::new(directory.path().join("state"));
            let mut repairable = serde_json::to_value(provider(100)).unwrap();
            repairable["id"] = json!("00000000-0000-4000-8000-000000000001");
            repairable["baseUrl"] = json!("https://relay.example/v1");
            write_provider(&store, &repairable);
            let mut invalid = serde_json::to_value(provider(200)).unwrap();
            invalid["id"] = json!("00000000-0000-4000-8000-000000000002");
            if candidate {
                invalid["connection"]["customEndpoints"][invalid_url] =
                    json!({"url": invalid_url, "addedAt": 1});
            } else {
                invalid["baseUrl"] = json!(invalid_url);
            }
            let (path, _) = write_provider(&store, &invalid);
            let before = file_tree(&store.configuration_dir()).unwrap();

            let error = store.repair().unwrap_err();

            let label = path
                .strip_prefix(store.configuration_dir())
                .unwrap()
                .display()
                .to_string();
            assert!(error.contains(&label), "{invalid_url:?}: {error}");
            assert_eq!(file_tree(&store.configuration_dir()).unwrap(), before);
            assert_eq!(fs::read_dir(store.state_root()).unwrap().count(), 1);
        }
    }
}

#[test]
fn codex_anthropic_version_prefixes_are_repaired() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let file = CodexProviderFile {
        schema_version: CODEX_PROVIDER_SCHEMA_VERSION,
        position: 100,
        profile: CodexProviderProfile {
            id: Uuid::new_v4().to_string(),
            name: "Anthropic 中转".to_string(),
            route_mode: CodexRouteMode::Gateway,
            endpoint: CodexEndpoint("https://relay.example/v1".to_string()),
            api_key: "sk-isolated".to_string(),
            authentication: None,
            connection: ProviderConnectionOptions {
                custom_endpoints: [(
                    "https://mirror.example/compatible-mode/v1".to_string(),
                    ProviderEndpoint {
                        url: "https://mirror.example/compatible-mode/v1".to_string(),
                        added_at: 1,
                        last_used: None,
                    },
                )]
                .into_iter()
                .collect(),
                ..Default::default()
            },
            upstream: CodexUpstream::AnthropicMessages,
            request_mode: ResponsesRequestMode::Standard,
            default_model: "claude-relay".to_string(),
            catalog: vec![CodexCatalogEntry::default_entry("claude-relay")],
            model_routes: Vec::new(),
            subagent_route: None,
            capabilities: DEFAULT_CODEX_CAPABILITIES,
        },
        parameters: default_provider_parameters(AppKind::Codex),
        notes: None,
        website_url: None,
        usage_query: None,
    };
    let path = store
        .providers_dir(AppKind::Codex)
        .join(format!("{}.json", file.profile.id));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let original = serde_json::to_string_pretty(&file).unwrap();
    fs::write(&path, &original).unwrap();
    assert!(store.codex_provider_snapshots().is_err());

    let report = store.repair().unwrap();

    assert_eq!(report.repaired_files.len(), 1);
    let snapshots = store.codex_provider_snapshots().unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].0.profile.endpoint.0, "https://relay.example/");
    assert!(snapshots[0].0.profile.connection.custom_endpoints.is_empty());
    let backup = PathBuf::from(report.backup_path.unwrap());
    let relative = path.strip_prefix(store.configuration_dir()).unwrap();
    assert_eq!(fs::read_to_string(backup.join(relative)).unwrap(), original);
    let repeated = store.repair().unwrap();
    assert!(repeated.repaired_files.is_empty());
    assert!(repeated.backup_path.is_none());
}
