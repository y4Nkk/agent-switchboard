use super::*;
use asb_core::{contracts::AppKind, ownership::default_client_settings};
use std::{fs, path::Path};

#[test]
fn valid_application_data_has_no_repair_writes_or_backups() {
    let temporary = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temporary.path().to_path_buf());
    local.configuration().initialize_current_layout().unwrap();
    let original = serde_json::to_vec(&AppSettings::default()).unwrap();
    fs::write(local.root().join("settings.json"), &original).unwrap();
    let revision = input_revision(&local);
    let report = check_data(&local, None, |_, _| {});
    assert!(report.configuration_ready);
    assert!(report.repaired_files.is_empty());
    assert!(report.backup_paths.is_empty());
    assert!(report.issues.is_empty());
    assert_eq!(input_revision(&local), revision);
    assert_eq!(fs::read(local.root().join("settings.json")).unwrap(), original);
}

#[test]
fn automatic_repair_reports_exact_backups_and_is_idempotent() {
    let temporary = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temporary.path().to_path_buf());
    let path = broken_configuration(&local);
    let original = fs::read(&path).unwrap();
    let report = check_data(&local, None, |_, _| {});
    assert!(report.configuration_ready);
    assert_eq!(report.repaired_files.len(), 1);
    let relative = path.strip_prefix(local.configuration().configuration_dir()).unwrap();
    assert_eq!(fs::read(Path::new(&report.backup_paths[0]).join(relative)).unwrap(), original);
    assert!(check_data(&local, None, |_, _| {}).repaired_files.is_empty());
}

#[test]
fn unrecoverable_configuration_preserves_original_and_reports_partial_result() {
    let temporary = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temporary.path().to_path_buf());
    let path = broken_configuration(&local);
    fs::write(&path, b"{unrecoverable").unwrap();
    let report = check_data(&local, None, |_, _| {});
    assert!(!report.configuration_ready);
    assert!(report.issues.iter().any(|issue| issue.code == "profile-store-repair-failed"));
    assert_eq!(fs::read(&path).unwrap(), b"{unrecoverable");
    assert!(!local.root().join("configuration-repair.json").exists());
}

#[test]
fn invalid_proxy_blocks_clients_and_can_only_be_changed_by_explicit_settings() {
    let temporary = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temporary.path().to_path_buf());
    local.configuration().initialize_current_layout().unwrap();
    let path = local.root().join("outbound-proxy.json");
    crate::outbound_proxy::load(local.root()).unwrap();
    crate::outbound_proxy::cached_client("preparation-async-fixture", |builder| builder.build().unwrap()).unwrap();
    crate::outbound_proxy::cached_blocking_client("preparation-blocking-fixture", |builder| builder.build().unwrap()).unwrap();
    fs::write(&path, b"{invalid-egress").unwrap();
    let report = check_data(&local, None, |_, _| {});
    assert!(report.issues.iter().any(|issue| issue.code == "outbound-proxy-unavailable"));
    assert_eq!(fs::read(&path).unwrap(), b"{invalid-egress");
    assert!(crate::outbound_proxy::apply(reqwest::Client::builder()).is_err());
    assert!(crate::outbound_proxy::apply_blocking(reqwest::blocking::Client::builder()).is_err());
    assert!(crate::outbound_proxy::cached_client("preparation-async-fixture", |_| panic!("invalid policy must not build a client")).is_err());
    assert!(crate::outbound_proxy::cached_blocking_client("preparation-blocking-fixture", |_| panic!("invalid policy must not build a client")).is_err());
    let before = crate::outbound_proxy::view(local.root()).unwrap();
    assert!(before.settings.is_none());
    let draft = crate::outbound_proxy::OutboundProxyDraft { mode: crate::outbound_proxy::OutboundProxyMode::Manual,
        url: Some("http://127.0.0.1:31991".into()) };
    let saved = crate::outbound_proxy::save(local.root(), draft.clone(), &before.revision).unwrap();
    assert!(saved.settings.is_some());
    assert!(crate::outbound_proxy::save(local.root(), draft, &before.revision).is_err());
    assert!(crate::outbound_proxy::apply(reqwest::Client::builder()).is_ok());
    let backups: Vec<_> = fs::read_dir(local.root()).unwrap().flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("outbound-proxy-backup-")).collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(fs::read(backups[0].path()).unwrap(), b"{invalid-egress");
    fs::remove_file(path).unwrap();
    crate::outbound_proxy::load(local.root()).unwrap();
}

#[test]
fn single_owner_coalesces_requests_and_publishes_monotonic_revisions() {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let preparation = Preparation::new(None, sender);
    for _ in 0..20 { preparation.request_check().unwrap(); }
    assert!(receiver.try_recv().is_ok());
    assert!(receiver.try_recv().is_err());
    assert!(preparation.require_ready().is_err());
    let first = preparation.publish(Stage::Configuration, Status::Running, Report::default(), true).unwrap();
    let second = preparation.publish(Stage::Complete, Status::Ready, Report::default(), false).unwrap();
    assert!(second.revision > first.revision);
    assert_eq!(second.run_id, first.run_id);
    preparation.initialized.store(true, std::sync::atomic::Ordering::Release);
    assert!(preparation.require_ready().is_ok());
    assert_eq!(serde_json::to_value(second).unwrap()["configurationReady"], false);
}

#[test]
fn exit_waits_for_the_active_check_and_a_blocked_exit_can_resume() {
    let (sender, _receiver) = std::sync::mpsc::sync_channel(1);
    let preparation = Preparation::new(None, sender);
    preparation.publish(Stage::Complete, Status::Ready, Report::default(), false).unwrap();
    assert!(preparation.claim(false).unwrap());
    assert!(preparation.defer_exit(Some(42)));
    assert_eq!(preparation.complete_check(), Some(42));
    assert!(!preparation.claim(false).unwrap());
    preparation.resume_after_blocked_exit();
    assert!(preparation.claim(false).unwrap());
    assert_eq!(preparation.complete_check(), None);
    assert!(!preparation.prepare_exit(None));
    assert!(!preparation.claim(false).unwrap());
}

#[test]
fn externally_changed_input_is_not_accepted_as_a_verified_revision() {
    let temporary = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temporary.path().to_path_buf());
    local.configuration().initialize_current_layout().unwrap();
    let path = local.configuration().client_settings_path(AppKind::Codex);
    fs::write(&path, serde_json::to_vec(&default_client_settings(AppKind::Codex)).unwrap()).unwrap();
    let report = check_data(&local, None, |stage, _| {
        if matches!(stage, Stage::OutboundProxy) { fs::write(&path, b"{external-change").unwrap(); }
    });
    assert!(!report.configuration_ready);
    assert!(report.issues.iter().any(|issue| issue.code == "application-data-changed"));
    assert_eq!(fs::read(path).unwrap(), b"{external-change");
    assert_ne!(report.observed_revision, input_revision(&local));
}

fn broken_configuration(local: &LocalState) -> std::path::PathBuf {
    local.configuration().initialize_current_layout().unwrap();
    let mut settings = default_client_settings(AppKind::Codex);
    settings.settings.remove(settings.settings.keys().next().cloned().as_ref().unwrap());
    let path = local.configuration().client_settings_path(AppKind::Codex);
    fs::write(&path, serde_json::to_vec(&settings).unwrap()).unwrap();
    path
}

pub(crate) fn checkpoint(name: &str, root: &Path) {
    if std::env::var("ASB_PREPARATION_FAULT").as_deref() != Ok(name) { return; }
    let sandbox = std::env::var_os("ASB_PREPARATION_SANDBOX").expect("fault injection requires sandbox");
    let sandbox = Path::new(&sandbox).canonicalize().unwrap();
    assert!(root.canonicalize().unwrap().starts_with(&sandbox));
    assert!(sandbox.join(".asb-preparation-sandbox").is_file());
    std::process::exit(73);
}

#[test]
#[ignore = "only launched by the isolated preparation sandbox driver"]
fn sandbox_process_driver() {
    let sandbox = std::env::var_os("ASB_PREPARATION_SANDBOX").expect("isolated sandbox is required");
    let sandbox = Path::new(&sandbox).canonicalize().unwrap();
    assert!(sandbox.join(".asb-preparation-sandbox").is_file());
    let local = LocalState::from_root(sandbox.join("state"));
    if std::env::var("ASB_PREPARATION_MODE").as_deref() == Ok("seed") {
        broken_configuration(&local);
        fs::write(local.root().join("settings.json"), b"{interrupted-preferences").unwrap();
        return;
    }
    let report = check_data(&local, None, |_, _| {});
    fs::write(sandbox.join("result.json"), serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    assert!(report.configuration_ready);
    assert!(report.issues.is_empty());
}
