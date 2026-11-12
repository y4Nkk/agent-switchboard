use super::*;

#[test]
fn malformed_preferences_are_preserved_before_defaults_are_published() {
    let temporary = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temporary.path().to_path_buf());
    let original = b"{incomplete-settings";
    fs::write(local.settings_path(), original).unwrap();
    let report = local.prepare_app_settings().unwrap();
    assert!(report.reset);
    assert_eq!(fs::read(report.backup_path.unwrap()).unwrap(), original);
    assert_eq!(local.get_app_settings().unwrap(), AppSettings::default());
    assert!(local.prepare_app_settings().unwrap().backup_path.is_none());
}

#[test]
fn bom_is_normalized_without_resetting_healthy_preferences() {
    let temporary = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temporary.path().to_path_buf());
    let mut settings = AppSettings::default();
    settings.interface_scale = 125;
    let original = format!("\u{feff}{}", serde_json::to_string(&settings).unwrap());
    fs::write(local.settings_path(), &original).unwrap();
    let report = local.prepare_app_settings().unwrap();
    assert!(!report.reset);
    assert_eq!(local.get_app_settings().unwrap(), settings);
    assert_eq!(fs::read_to_string(report.backup_path.unwrap()).unwrap(), original);
}

#[test]
fn incomplete_contract_and_unreadable_target_are_never_overwritten() {
    let temporary = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temporary.path().to_path_buf());
    fs::write(local.settings_path(), b"{\"future\":true}").unwrap();
    assert!(local.prepare_app_settings().is_err());
    assert_eq!(fs::read(local.settings_path()).unwrap(), b"{\"future\":true}");
    fs::remove_file(local.settings_path()).unwrap();
    fs::create_dir(local.settings_path()).unwrap();
    assert!(local.prepare_app_settings().is_err());
    assert!(local.settings_path().is_dir());
}

#[test]
fn changed_settings_during_recovery_are_preserved() {
    let temporary = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temporary.path().to_path_buf());
    let record = Recovery { version: 1, id: uuid::Uuid::new_v4().simple().to_string(),
        original: io::digest(b"{broken"), candidate: AppSettings::default(), reset: true };
    fs::write(local.root.join(RECORD), serde_json::to_vec(&record).unwrap()).unwrap();
    fs::write(local.settings_path(), b"external-change").unwrap();
    assert!(local.prepare_app_settings().is_err());
    assert_eq!(fs::read(local.settings_path()).unwrap(), b"external-change");
    assert!(local.root.join(RECORD).exists());
}

#[test]
fn completed_publication_is_recovered_without_a_second_write() {
    let temporary = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temporary.path().to_path_buf());
    let record = Recovery { version: 1, id: uuid::Uuid::new_v4().simple().to_string(),
        original: io::digest(b"{broken"), candidate: AppSettings::default(), reset: true };
    fs::write(local.root.join(RECORD), serde_json::to_vec(&record).unwrap()).unwrap();
    fs::write(local.root.join(format!("settings-backup-{}.json", record.id)), b"{broken").unwrap();
    fs::write(local.settings_path(), serde_json::to_vec_pretty(&record.candidate).unwrap()).unwrap();
    assert!(local.prepare_app_settings().unwrap().backup_path.is_some());
    assert!(!local.root.join(RECORD).exists());
    assert!(local.prepare_app_settings().unwrap().backup_path.is_none());
}
