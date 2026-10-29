use super::*;
use asb_core::{AppKind, contracts::{ConfigValue, SettingValue}};

fn record(state: &LocalState) -> BackupRecord {
    BackupRecord {
        id: "test-snapshot".into(),
        app: AppKind::Claude,
        // Only metadata: these tests never read or write the resolved client target.
        target_path: state.target(AppKind::Claude).unwrap().to_string_lossy().into_owned(),
        backup_path: state.backup_dir().join("settings.json.test.bak").to_string_lossy().into_owned(),
        content_hash: asb_switch::sha256_hex("{}"),
        created_at: "2026-09-21T00:00:00Z".into(),
        reason: "client-configuration-native-defaults".into(),
        target_existed: false,
        linked_backup_id: None,
    }
}

#[test]
fn paired_backup_round_trips_exact_saved_intent_and_previews_settings_only_changes() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().to_path_buf());
    let backup = record(&state);
    let mut saved = asb_core::ownership::default_client_settings(AppKind::Claude);
    saved.settings.insert("spinnerTipsEnabled".into(), SettingValue::Explicit { value: ConfigValue::Bool(true) });
    save(&state, &backup, &saved).unwrap();
    save(&state, &backup, &saved).unwrap();
    assert_eq!(load(&state, &backup).unwrap(), Some(saved));
    assert!(diff(&state, &backup).unwrap().iter().any(|change| change.key.contains("spinnerTipsEnabled")));
    assert!(save(&state, &backup, &asb_core::ownership::default_client_settings(AppKind::Claude)).is_err());

    let mut wrong_identity = backup.clone();
    wrong_identity.id = "different-backup".into();
    assert!(load(&state, &wrong_identity).is_err());
    let snapshot_path = state.client_settings_backup_path(&backup).unwrap();
    let mut damaged: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&snapshot_path).unwrap()).unwrap();
    damaged["settingsHash"] = serde_json::json!("corrupted");
    std::fs::write(snapshot_path, serde_json::to_string(&damaged).unwrap()).unwrap();
    assert!(load(&state, &backup).is_err());
}

#[test]
fn missing_snapshot_never_guesses_settings_for_paired_operations() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().to_path_buf());
    let mut backup = record(&state);
    for reason in ["client-configuration-apply", "client-configuration-native-defaults", "client-configuration-native-defaults-unmanaged", "client-configuration-clear-extra-configuration", "manual-client-configuration", "restore-precheck"] {
        backup.reason = reason.into();
        assert!(load(&state, &backup).is_err());
    }
    backup.reason = "switch".into();
    assert_eq!(load(&state, &backup).unwrap(), None);
}
