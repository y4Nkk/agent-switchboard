use super::*;
use asb_core::contracts::{
    CodexCatalogEntry, CodexEndpoint, CodexModelRoute, CodexProviderDraft, CodexProviderFile,
    CodexUpstream, ProviderConnectionOptions, ProviderDraft, ProviderEndpoint, ProviderFile,
    ResponsesRequestMode, UsageQuery, DEFAULT_CODEX_CAPABILITIES,
};
use asb_core::ownership::default_provider_parameters;

const TRANSFER_TABLE_DDL: &str = "CREATE TABLE IF NOT EXISTS agent_switchboard_providers (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  position INTEGER NOT NULL,
  file TEXT NOT NULL
);";

/// Every store and scratch database below lives inside the sandbox root;
/// nothing in this module touches the real app state or home directory.
fn sandbox(tag: &str) -> (tempfile::TempDir, LocalState) {
    let dir = tempfile::Builder::new()
        .prefix(&format!("asb-transfer-{tag}-"))
        .tempdir()
        .expect("sandbox dir");
    let state = LocalState::from_root(dir.path().to_path_buf());
    state.initialize_schemas().expect("schemas");
    (dir, state)
}

fn claude_relay_draft(name: &str, api_key: &str) -> ProviderDraft {
    let mut draft: ProviderDraft = serde_json::from_value(serde_json::json!({
        "app": "claude",
        "routeMode": "custom",
        "name": name,
        "baseUrl": "https://relay.example.com",
        "apiKey": api_key,
        "upstreamProtocol": "anthropicMessages",
        "maxOutputTokens": null,
        "model": "claude-sonnet-4-5",
        "parameters": { "settings": {} },
        "notes": "迁移测试备注",
        "websiteUrl": "https://example.com",
        "display": { "icon": "relay", "iconColor": "#3366cc", "category": "中转", "createdAt": 1758000000 },
        "usageQuery": {
            "kind": "declarative",
            "url": "{{baseUrl}}/usage",
            "remainingPath": "data/balance",
            "unit": "USD",
            "refreshIntervalMinutes": 60
        }
    }))
    .expect("claude draft parses");
    draft.parameters = default_provider_parameters(AppKind::Claude);
    draft.connection.custom_endpoints.insert(
        "https://backup.example.com".to_string(),
        ProviderEndpoint {
            url: "https://backup.example.com".to_string(),
            added_at: 1758000000,
            last_used: None,
        },
    );
    draft
}

/// Same shape the cc-switch importer uses for its official Claude row.
fn claude_official_draft() -> ProviderDraft {
    let mut draft: ProviderDraft = serde_json::from_value(serde_json::json!({
        "app": "claude",
        "routeMode": "official",
        "name": "Claude 官方登录",
        "apiKey": "",
        "maxOutputTokens": null,
        "parameters": { "settings": {} }
    }))
    .expect("claude official draft parses");
    draft.parameters = default_provider_parameters(AppKind::Claude);
    draft
}

fn codex_custom_draft() -> CodexProviderDraft {
    CodexProviderDraft {
        name: "第三方 Codex".to_string(),
        endpoint: CodexEndpoint("https://api.example.com/v1".to_string()),
        api_key: "sk-codex-1".to_string(),
        authentication: None,
        connection: ProviderConnectionOptions::default(),
        upstream: CodexUpstream::ChatCompletions,
        request_mode: ResponsesRequestMode::Standard,
        default_model: "gpt-5.2".to_string(),
        catalog: vec![
            CodexCatalogEntry::default_entry("gpt-5.2"),
            CodexCatalogEntry::default_entry("gpt-5.2-mini"),
        ],
        model_routes: vec![CodexModelRoute {
            client_model: "gpt-5.2-mini".to_string(),
            upstream_model: "provider-model-x".to_string(),
        }],
        subagent_route: None,
        capabilities: DEFAULT_CODEX_CAPABILITIES,
        parameters: default_provider_parameters(AppKind::Codex),
        notes: None,
        website_url: None,
        usage_query: Some(UsageQuery::Declarative {
            url: "{{baseUrl}}/v1/usage".to_string(),
            remaining_path: Some("data/remaining".to_string()),
            used_path: None,
            total_path: None,
            unit: Some("USD".to_string()),
            refresh_interval_minutes: 30,
        }),
    }
}

fn seed_source_device(state: &LocalState) {
    let configuration = state.configuration();
    configuration
        .create_provider(uuid::Uuid::new_v4().to_string(), claude_relay_draft("中转 A", "sk-a"))
        .expect("claude A seeds");
    configuration
        .create_provider(uuid::Uuid::new_v4().to_string(), claude_relay_draft("中转 B", "sk-b"))
        .expect("claude B seeds");
    let (official, created) = configuration
        .ensure_codex_official_record()
        .expect("codex official seeds");
    assert!(created, "the sandbox source device starts without official");
    let _ = official.profile.id;
    configuration
        .create_codex_provider(uuid::Uuid::new_v4().to_string(), codex_custom_draft())
        .expect("codex custom seeds");
}

fn export_sql(state: &LocalState, path: &Path) {
    let configuration = state.configuration();
    let claude = load_provider_files(&configuration, AppKind::Claude).expect("claude files");
    let official = load_provider_files(&configuration, AppKind::Codex).expect("official files");
    let custom = configuration
        .codex_provider_snapshots()
        .expect("custom files")
        .into_iter()
        .map(|(file, _)| file)
        .collect::<Vec<_>>();
    let sql =
        asb_core::provider_transfer::write_transfer_sql(&claude, &official, &custom)
            .expect("sql writes");
    std::fs::write(path, sql).expect("sql file writes");
}

fn strip_positions_claude(files: &[ProviderFile]) -> Vec<ProviderFile> {
    let mut stripped = files.to_vec();
    for file in &mut stripped {
        file.position = 0;
    }
    stripped
}

fn strip_positions_custom(entries: &[(CodexProviderFile, String)]) -> Vec<CodexProviderFile> {
    entries
        .iter()
        .map(|(file, _)| {
            let mut file = file.clone();
            file.position = 0;
            file
        })
        .collect()
}

fn import_everything(state: &LocalState, sql_path: &Path) -> ProviderSqlImportOutcome {
    let scan = scan(state, sql_path.to_str().unwrap()).expect("scan");
    let ids: Vec<String> = scan.providers.iter().map(|item| item.key.clone()).collect();
    import(state, &ids, sql_path.to_str().unwrap()).expect("import")
}

fn assert_fresh_scan(scan: &ProviderSqlScan) {
    assert_eq!(scan.providers.len(), 4);
    assert!(scan.providers.iter().all(|item| !item.existing));
    assert!(scan.providers.iter().all(|item| item.warnings.is_empty()));
    assert!(scan.skipped.is_empty());
    assert_eq!(
        scan.providers
            .iter()
            .filter(|item| item.kind == TransferKind::Claude)
            .count(),
        2
    );
    assert_eq!(
        scan.providers
            .iter()
            .filter(|item| item.kind == TransferKind::CodexOfficial)
            .count(),
        1
    );
    let custom_item = scan
        .providers
        .iter()
        .find(|item| item.kind == TransferKind::CodexCustom)
        .expect("custom item");
    assert_eq!(custom_item.name, "第三方 Codex");
    assert_eq!(custom_item.base_url.as_deref(), Some("https://api.example.com/v1"));
    assert_eq!(custom_item.model.as_deref(), Some("gpt-5.2"));
}

#[test]
fn transfer_round_trips_complete_configuration_into_a_fresh_device() {
    let (dir_a, state_a) = sandbox("round-a");
    seed_source_device(&state_a);
    let sql_path = dir_a.path().join("agent-switchboard-providers.sql");
    export_sql(&state_a, &sql_path);

    let expected_claude =
        load_provider_files(&state_a.configuration(), AppKind::Claude).unwrap();
    let expected_official =
        load_provider_files(&state_a.configuration(), AppKind::Codex).unwrap();
    let expected_custom = state_a.configuration().codex_provider_snapshots().unwrap();

    let (_dir_b, state_b) = sandbox("round-b");
    let scan = scan(&state_b, sql_path.to_str().unwrap()).expect("scan");
    assert_fresh_scan(&scan);

    let outcome = import_everything(&state_b, &sql_path);
    assert_eq!(outcome.imported_count, 4);
    assert_eq!(outcome.updated_count, 0);
    assert!(outcome.not_imported.is_empty());

    let store = state_b.configuration();
    let claude = load_provider_files(&store, AppKind::Claude).unwrap();
    let official = load_provider_files(&store, AppKind::Codex).unwrap();
    let custom = store.codex_provider_snapshots().unwrap();
    // Complete configuration: every stored field round-trips; only the
    // sort positions may compact on the fresh device.
    assert_eq!(
        strip_positions_claude(&claude),
        strip_positions_claude(&expected_claude)
    );
    assert_eq!(
        strip_positions_claude(&official),
        strip_positions_claude(&expected_official)
    );
    assert_eq!(
        strip_positions_custom(&custom),
        strip_positions_custom(&expected_custom)
    );
    assert_eq!(
        claude.iter().map(|file| file.name.clone()).collect::<Vec<_>>(),
        vec!["中转 A".to_string(), "中转 B".to_string()]
    );
    assert_eq!(official.len(), 1);
    assert_eq!(official[0].id, expected_official[0].id);
    let mut positions: Vec<u64> = official
        .iter()
        .map(|file| file.position)
        .chain(custom.iter().map(|(file, _)| file.position))
        .collect();
    positions.sort_unstable();
    assert_eq!(positions, vec![100, 200]);
    // The scratch database never leaves the sandbox state root.
    assert!(state_b.root().join(SCRATCH_DIRECTORY).is_dir());
}

#[test]
fn re_importing_over_existing_records_updates_in_place_without_position_drift() {
    let (dir_a, state_a) = sandbox("update-a");
    seed_source_device(&state_a);
    let sql_path = dir_a.path().join("agent-switchboard-providers.sql");
    export_sql(&state_a, &sql_path);

    let (_dir_b, state_b) = sandbox("update-b");
    let first = import_everything(&state_b, &sql_path);
    assert_eq!(first.imported_count, 4);

    let store = state_b.configuration();
    let before: Vec<(String, u64)> = load_provider_files(&store, AppKind::Claude)
        .unwrap()
        .into_iter()
        .map(|file| (file.id.clone(), file.position))
        .chain(
            load_provider_files(&store, AppKind::Codex)
                .unwrap()
                .into_iter()
                .map(|file| (file.id.clone(), file.position)),
        )
        .chain(
            store
                .codex_provider_snapshots()
                .unwrap()
                .into_iter()
                .map(|(file, _)| (file.profile.id.clone(), file.position)),
        )
        .collect();

    let scan = scan(&state_b, sql_path.to_str().unwrap()).expect("re-scan");
    assert!(scan.providers.iter().all(|item| item.existing));
    let outcome = import_everything(&state_b, &sql_path);
    assert_eq!(outcome.imported_count, 0);
    assert_eq!(outcome.updated_count, 4);
    assert!(outcome.not_imported.is_empty());

    let store = state_b.configuration();
    let after: Vec<(String, u64)> = load_provider_files(&store, AppKind::Claude)
        .unwrap()
        .into_iter()
        .map(|file| (file.id.clone(), file.position))
        .chain(
            load_provider_files(&store, AppKind::Codex)
                .unwrap()
                .into_iter()
                .map(|file| (file.id.clone(), file.position)),
        )
        .chain(
            store
                .codex_provider_snapshots()
                .unwrap()
                .into_iter()
                .map(|(file, _)| (file.profile.id.clone(), file.position)),
        )
        .collect();
    assert_eq!(before, after, "overwrites keep the local sort positions");
}

#[test]
fn importing_keeps_the_target_devices_own_official_record() {
    let (dir_a, state_a) = sandbox("official-a");
    seed_source_device(&state_a);
    let sql_path = dir_a.path().join("agent-switchboard-providers.sql");
    export_sql(&state_a, &sql_path);

    let (_dir_c, state_c) = sandbox("official-c");
    let (own_official, created) = state_c
        .configuration()
        .ensure_codex_official_record()
        .expect("target official seeds");
    assert!(created);

    let scan = scan(&state_c, sql_path.to_str().unwrap()).expect("scan");
    let official_item = scan
        .providers
        .iter()
        .find(|item| item.kind == TransferKind::CodexOfficial)
        .expect("official item");
    assert!(!official_item.existing);
    assert!(official_item
        .warnings
        .iter()
        .any(|warning| warning.contains("官方登录记录")));

    let outcome = import_everything(&state_c, &sql_path);
    assert_eq!(outcome.imported_count, 3);
    assert_eq!(outcome.not_imported.len(), 1);
    assert!(outcome.not_imported[0].reason.contains("官方登录记录"));

    let official = load_provider_files(&state_c.configuration(), AppKind::Codex).unwrap();
    assert_eq!(official.len(), 1);
    assert_eq!(official[0].id, own_official.profile.id);
}

#[test]
fn apply_rejects_escaping_foreign_and_tampered_files() {
    let (dir, state) = sandbox("hostile");

    let attach = dir.path().join("attach.sql");
    std::fs::write(&attach, "BEGIN; ATTACH DATABASE 'evil.db' AS x; COMMIT;")
        .expect("attach fixture");
    let error = scan(&state, attach.to_str().unwrap()).expect_err("attach is rejected");
    assert!(error.contains("导出格式之外的语句"), "got: {error}");

    let foreign = dir.path().join("foreign.sql");
    std::fs::write(&foreign, "CREATE TABLE other (x TEXT);").expect("foreign fixture");
    let error = scan(&state, foreign.to_str().unwrap()).expect_err("foreign is rejected");
    assert!(error.contains("不是 Agent Switchboard 导出"), "got: {error}");

    let tampered = dir.path().join("tampered.sql");
    std::fs::write(
        &tampered,
        format!(
            "BEGIN TRANSACTION;\n{TRANSFER_TABLE_DDL}\
             INSERT OR REPLACE INTO agent_switchboard_providers (id, kind, position, file)\n\
             VALUES ('0f0e0d0c-1a2b-4c3d-8e9f-0a1b2c3d4e5f', 'claude', 100, \
             '{{\"id\":\"11111111-2222-4333-8444-555555555555\",\"name\":\"x\",\"position\":100,\
             \"routeMode\":\"custom\",\"apiKey\":\"\",\"maxOutputTokens\":null,\
             \"parameters\":{{\"settings\":{{}}}}}}');\n\
             COMMIT;\n"
        ),
    )
    .expect("tampered fixture");
    let tampered_scan = scan(&state, tampered.to_str().unwrap()).expect("tampered rows are skipped");
    assert!(tampered_scan.providers.is_empty());
    assert_eq!(tampered_scan.skipped.len(), 1);
    assert!(tampered_scan.skipped[0].reason.contains("不一致"), "got: {}", tampered_scan.skipped[0].reason);

    let escaping = dir.path().join("escaping.sql");
    for (name, statement) in [
        ("pragma", "PRAGMA journal_mode = WAL;"),
        (
            "user update",
            &format!("{TRANSFER_TABLE_DDL}UPDATE agent_switchboard_providers SET position = 1;"),
        ),
        (
            "hand-written index",
            &format!("{TRANSFER_TABLE_DDL}CREATE INDEX probe ON agent_switchboard_providers (kind);"),
        ),
    ] {
        std::fs::write(&escaping, statement).expect("escaping fixture");
        let error = scan(&state, escaping.to_str().unwrap())
            .expect_err(&format!("{name} is rejected"));
        assert!(
            error.contains("导出格式之外的语句"),
            "{name} was not denied: {error}"
        );
    }
}

#[test]
fn importing_keeps_the_target_devices_own_claude_official_record() {
    let (dir_a, state_a) = sandbox("claude-official-a");
    seed_source_device(&state_a);
    state_a
        .configuration()
        .create_provider(uuid::Uuid::new_v4().to_string(), claude_official_draft())
        .expect("claude official seeds");
    let sql_path = dir_a.path().join("agent-switchboard-providers.sql");
    export_sql(&state_a, &sql_path);

    let (_dir_c, state_c) = sandbox("claude-official-c");
    let own = state_c
        .configuration()
        .create_provider(uuid::Uuid::new_v4().to_string(), claude_official_draft())
        .expect("target claude official seeds")
        .profile
        .id
        .clone();

    let scan = scan(&state_c, sql_path.to_str().unwrap()).expect("scan");
    let official_item = scan
        .providers
        .iter()
        .find(|item| {
            item.kind == TransferKind::Claude
                && item
                    .warnings
                    .iter()
                    .any(|warning| warning.contains("官方登录记录"))
        })
        .expect("claude official item carries the singleton warning");
    assert!(!official_item.existing);

    let outcome = import_everything(&state_c, &sql_path);
    assert_eq!(outcome.imported_count, 4);
    assert_eq!(outcome.not_imported.len(), 1);
    assert!(outcome.not_imported[0].reason.contains("Claude 官方登录记录"));

    let claude = load_provider_files(&state_c.configuration(), AppKind::Claude).unwrap();
    let officials: Vec<&ProviderFile> = claude
        .iter()
        .filter(|file| file.route_mode == RouteMode::Official)
        .collect();
    assert_eq!(officials.len(), 1);
    assert_eq!(officials[0].id, own);
}
