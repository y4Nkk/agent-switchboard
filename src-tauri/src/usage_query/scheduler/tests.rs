use super::*;
use asb_core::contracts::{ProviderDraft, UpstreamProtocol, UsageQuery};

fn claude(state: &LocalState, base_url: &str) -> ProviderProfile {
    state.configuration().create_provider(uuid::Uuid::new_v4().to_string(), ProviderDraft {
        app: AppKind::Claude,
        route_mode: RouteMode::Custom,
        name: "isolated-claude".into(),
        model: Some("test-model".into()),
        base_url: Some(base_url.into()),
        api_key: "isolated-key".into(),
        authentication: None,
        upstream_protocol: Some(UpstreamProtocol::AnthropicMessages),
        parameters: asb_core::ownership::default_provider_parameters(AppKind::Claude),
        claude_fragment: Default::default(),
        connection: Default::default(),
        responses_options: None,
        max_output_tokens: None.into(),
        model_options: None,
        notes: None,
        website_url: None,
        display: None,
        official_quota_refresh_interval_minutes: None,
        usage_query: Some(UsageQuery::Declarative {
            url: "{{baseUrl}}/usage".into(), remaining_path: Some("/remaining".into()),
            used_path: None, total_path: None, unit: None, refresh_interval_minutes: 0,
        }),
    }).unwrap().profile
}

#[test]
fn codex_store_errors_do_not_block_claude_cache_reads_or_scheduling() {
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let claude = claude(&state, "https://example.invalid");
    let gate = ConfigWriteGate::default();
    let (official, _) = state.configuration().ensure_codex_official_record().unwrap();
    let codex = state.configuration().providers_dir(AppKind::Codex);
    std::fs::write(codex.join("invalid.json"), "invalid").unwrap();
    std::fs::write(codex.join("official").join(format!("{}.json", official.profile.id)), "invalid").unwrap();
    let scheduled = profiles(&state, &gate).unwrap();
    assert_eq!(scheduled.len(), 1);
    assert_eq!(scheduled[0].id, claude.id);
    let resolved = usage_profile(&state, AppKind::Claude, &claude.id).unwrap();
    assert_eq!(resolved.id, claude.id);
    // Zero interval must return before any upstream access or cache write.
    assert!(!execute_once(&state, resolved.app, &resolved.id, false, &gate).unwrap());
    assert!(!state.root().join("usage-cache.json").exists());
}

#[test]
fn claude_store_errors_do_not_block_manual_only_official_profile() {
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let (official, _) = state.configuration().ensure_codex_official_record().unwrap();
    let claude = state.configuration().providers_dir(AppKind::Claude);
    std::fs::create_dir_all(&claude).unwrap();
    std::fs::write(claude.join("invalid.json"), "invalid").unwrap();
    let gate = ConfigWriteGate::default();
    let scheduled = profiles(&state, &gate).unwrap();
    assert_eq!(scheduled.len(), 1);
    assert_eq!(scheduled[0].id, official.profile.id);
    // This path must not resolve native credentials when automatic refresh is off.
    assert!(!crate::commands::quota_refresh::refresh(&state, &official.profile.id, false, &gate).unwrap());
}

#[test]
fn scheduled_inventory_waits_for_the_complete_configuration_directory() {
    use std::{fs, sync::mpsc};
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let profile = claude(&state, "https://example.invalid");
    let gate = ConfigWriteGate::default();
    let guard = gate.lock().unwrap();
    let live = state.configuration().configuration_dir();
    let retired = state.root().join("retired");
    fs::rename(&live, &retired).unwrap();
    let reader_gate = gate.clone();
    let reader_state = state.clone();
    let (sent, received) = mpsc::channel();
    let reader = thread::spawn(move || sent.send(profiles(&reader_state, &reader_gate)).unwrap());
    let early = received.recv_timeout(Duration::from_millis(50));
    fs::rename(retired, live).unwrap();
    drop(guard);
    reader.join().unwrap();
    assert!(matches!(early, Err(mpsc::RecvTimeoutError::Timeout)));
    let loaded = received.recv().unwrap().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, profile.id);
}

#[test]
fn missing_codex_query_reports_lookup_failure_before_any_network_or_cache_write() {
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let error = execute_once(&state, AppKind::Codex, "00000000-0000-0000-0000-000000000001",
        true, &ConfigWriteGate::default()).unwrap_err();
    assert_eq!(error.code, "codex-profile-not-found");
    assert!(!error.message.contains("格式"));
    assert!(!state.root().exists());
}

#[test]
fn script_execution_failure_preserves_valid_profiles_and_successful_readings_for_both_clients() {
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let script = UsageQuery::Script {
        source: "(() => { throw new Error('private-script-value'); })()".into(),
        refresh_interval_minutes: 2,
    };
    let claude = claude(&state, "https://example.invalid");
    let (app, mut file) = state.configuration().load_provider_file(&claude.id).unwrap();
    file.usage_query = Some(script.clone());
    state.configuration().overwrite_provider_file(app, file).unwrap();
    let mut draft = crate::codex_common::test_draft();
    draft.usage_query = Some(script);
    let codex = state.configuration().create_codex_provider(uuid::Uuid::new_v4().to_string(), draft).unwrap();
    let configuration = state.configuration().data_revision().unwrap();
    let summary = super::super::response::normalize_readings(vec![
        serde_json::from_value(serde_json::json!({"remaining":7})).unwrap(),
    ], "2026-11-13T00:00:00Z".into()).unwrap();
    let gate = ConfigWriteGate::default();
    for (app, id) in [(AppKind::Claude, claude.id), (AppKind::Codex, codex.profile.id)] {
        let profile = usage_profile(&state, app, &id).unwrap();
        usage_cache::record_success(&state, &profile, summary.clone(), chrono::Utc::now()).unwrap();
        let error = execute_once(&state, app, &id, true, &gate).unwrap_err();
        assert_eq!(error.code, "usage-query-failed");
        assert!(!error.message.contains("private-script-value"));
        let cached = usage_cache::get(&state, &profile).unwrap().unwrap();
        assert_eq!(cached.summary, Some(summary.clone()));
        assert!(cached.error.is_some());
    }
    assert_eq!(state.configuration().data_revision().unwrap(), configuration);
    let snapshot = crate::config_store::snapshot::read_configuration_snapshot(&state.configuration()).unwrap();
    crate::config_store::snapshot::validate_snapshot(&snapshot).unwrap();
    assert_eq!(snapshot.provider_count(), 2);
    assert!(state.configuration().repair().unwrap().repaired_files.is_empty());
}

#[test]
fn slow_query_releases_configuration_gate_and_waits_for_recovery_before_cache_commit() {
    use std::{fs, sync::mpsc};
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let upstream = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let profile = claude(&state, &format!("http://{}", upstream.server_addr()));
    let gate = ConfigWriteGate::default();
    let (request_sent, request_seen) = mpsc::channel();
    let (release_response, wait_response) = mpsc::channel();
    let server = thread::spawn(move || {
        let request = upstream.recv_timeout(Duration::from_secs(3)).unwrap().unwrap();
        request_sent.send(()).unwrap();
        wait_response.recv_timeout(Duration::from_secs(3)).unwrap();
        request.respond(tiny_http::Response::from_string(r#"{"remaining":7}"#)).unwrap();
    });
    let query_state = state.clone();
    let query_gate = gate.clone();
    let id = profile.id.clone();
    let (query_sent, query_done) = mpsc::channel();
    let query = thread::spawn(move || query_sent.send(execute_once(&query_state, AppKind::Claude, &id, true, &query_gate)).unwrap());
    request_seen.recv_timeout(Duration::from_secs(3)).unwrap();
    let writer_state = state.clone();
    let (writer_sent, writer_ready) = mpsc::channel();
    let (restore_directory, wait_restore) = mpsc::channel();
    let writer = thread::spawn(move || {
        let _guard = gate.lock().unwrap();
        let live = writer_state.configuration().configuration_dir();
        let retired = writer_state.root().join("retired");
        fs::rename(&live, &retired).unwrap();
        writer_sent.send(()).unwrap();
        wait_restore.recv_timeout(Duration::from_secs(3)).unwrap();
        fs::rename(retired, live).unwrap();
    });
    let released_during_network = writer_ready.recv_timeout(Duration::from_millis(300)).is_ok();
    release_response.send(()).unwrap();
    let early = query_done.recv_timeout(Duration::from_millis(50));
    restore_directory.send(()).unwrap();
    writer.join().unwrap(); server.join().unwrap(); query.join().unwrap();
    assert!(released_during_network, "network wait must not hold the configuration gate");
    assert!(matches!(early, Err(mpsc::RecvTimeoutError::Timeout)), "cache commit must await the complete directory");
    assert!(query_done.recv().unwrap().unwrap());
    let cached = usage_cache::get(&state, &profile).unwrap().unwrap();
    assert!(cached.error.is_none());
    assert!(cached.summary.is_some());
}
