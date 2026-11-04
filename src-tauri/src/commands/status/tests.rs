use super::{report::active_profile_id, ConfigFileStatus};
use asb_core::{
    adapter, codex_official_draft, ownership, AppKind, ConfigWriteRecord, ProviderProfile,
    RouteMode, SwitchPlan, WriteOperation,
};

fn official() -> ProviderProfile {
    ProviderProfile::from_draft(
        "official-fixture".into(),
        codex_official_draft(ownership::default_provider_parameters(AppKind::Codex)),
    )
}

fn history(profile_id: Option<&str>, operation: WriteOperation) -> ConfigWriteRecord {
    ConfigWriteRecord {
        app: AppKind::Codex,
        profile_id: profile_id.map(str::to_owned),
        profile_name: None,
        content_hash: asb_switch::sha256_hex("old configuration"),
        backup_id: "isolated-backup".into(),
        at: "2026-10-16T00:00:00Z".into(),
        operation,
    }
}

#[test]
fn official_identity_is_independent_of_live_model_preferences_and_comments() {
    let profile = official();
    let profiles = [profile.clone()];
    let plan = SwitchPlan::direct(profile.clone(), ownership::default_client_settings(AppKind::Codex));
    for live in [
        "model = 'fixture-model'\nmodel_reasoning_effort = 'high'\nservice_tier = 'default'\n",
        "# Native defaults need no explicit provider key.\nmodel = 'other-model'\n",
        "model_provider = 'openai'\nmodel = 'fixture-model'\n[windows]\nsandbox = 'unelevated'\n",
    ] {
        assert_eq!(
            active_profile_id(&profiles, None, AppKind::Codex, live, None).unwrap(),
            Some(profile.id.clone()),
        );
        assert_eq!(adapter::route_state(AppKind::Codex, live).route_mode, RouteMode::Official);
        let preview = adapter::preview(live, &plan, "isolated-backups").unwrap();
        assert!(preview.changes.iter().any(|change| change.key == "model" && change.after.is_none()));
    }
}

#[test]
fn native_official_configuration_does_not_require_a_saved_profile() {
    let live = "model = 'fixture-model'\n";
    assert_eq!(active_profile_id(&[], None, AppKind::Codex, live, None).unwrap(), None);
    assert_eq!(adapter::route_state(AppKind::Codex, live).model.as_deref(), Some("fixture-model"));
}

#[test]
fn claude_identity_is_independent_of_live_model_and_language() {
    let mut draft = codex_official_draft(ownership::default_provider_parameters(AppKind::Claude));
    draft.app = AppKind::Claude;
    let profile = ProviderProfile::from_draft("claude-official-fixture".into(), draft);
    let live = r#"{"model":"live-claude-model","language":"Japanese"}"#;
    assert_eq!(
        active_profile_id(&[profile.clone()], None, AppKind::Claude, live, None).unwrap(),
        Some(profile.id),
    );
    assert_eq!(adapter::route_state(AppKind::Claude, live).model.as_deref(), Some("live-claude-model"));
}

#[test]
fn write_or_restore_history_cannot_override_observed_connection_identity() {
    let profiles = [official()];
    let live = "model = 'restored-model'\n";
    for record in [
        history(Some("unrelated-profile"), WriteOperation::Projection),
        history(None, WriteOperation::Restore),
    ] {
        assert_eq!(
            active_profile_id(&profiles, None, AppKind::Codex, live, Some(&record)).unwrap(),
            Some(profiles[0].id.clone()),
        );
    }
    let another_connection = "openai_base_url = 'https://relay.example/v1'\n";
    let last = history(Some(&profiles[0].id), WriteOperation::Projection);
    assert_eq!(
        active_profile_id(&profiles, None, AppKind::Codex, another_connection, Some(&last)).unwrap(),
        None,
    );
}

#[test]
fn invalid_preferences_preserve_their_diagnostic_without_changing_connection_identity() {
    let profiles = [official()];
    let live = "model = 'fixture-model'\napproval_policy = 123\n";
    let error = adapter::extract_client_settings(AppKind::Codex, live).unwrap_err();
    assert!(!error.to_string().is_empty());
    assert_eq!(
        active_profile_id(&profiles, None, AppKind::Codex, live, None).unwrap(),
        Some(profiles[0].id.clone()),
    );
}

#[test]
fn public_status_contains_only_observations_identity_and_write_history() {
    let live = "model = 'fixture-model'\n";
    let status = ConfigFileStatus {
        app: AppKind::Codex,
        path: "isolated-config".into(),
        exists: true,
        syntax_ok: true,
        route: Some(adapter::route_state(AppKind::Codex, live)),
        gateway_port: Some(47821),
        read_error: None,
        recovery_issue: Some("isolated recovery detail".into()),
        client_settings: Some(adapter::extract_client_settings(AppKind::Codex, live).unwrap()),
        client_settings_error: Some("isolated setting detail".into()),
        codex_permission_profile: Some(adapter::codex::permission_profile_status(live).unwrap()),
        active_profile_id: Some("official-fixture".into()),
        last_switch: Some(history(None, WriteOperation::Restore)),
    };
    let serialized = serde_json::to_value(&status).unwrap();
    let mut actual = serialized.as_object().unwrap().keys().map(String::as_str).collect::<Vec<_>>();
    actual.sort_unstable();
    let mut expected = vec![
        "app", "path", "exists", "syntaxOk", "route", "readError", "recoveryIssue",
        "clientSettings", "clientSettingsError", "codexPermissionProfile", "activeProfileId", "lastSwitch",
    ];
    expected.sort_unstable();
    assert_eq!(actual, expected);
    assert_eq!(serialized["route"]["model"], "fixture-model");
    assert_eq!(serialized["recoveryIssue"], "isolated recovery detail");
    assert_eq!(serialized["clientSettingsError"], "isolated setting detail");
    assert_eq!(serialized["lastSwitch"]["operation"], "restore");
}
