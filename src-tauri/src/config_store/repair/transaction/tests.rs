use super::*;
use asb_core::{contracts::AppKind, ownership::default_client_settings};

fn prepared() -> (tempfile::TempDir, ConfigStore, Transaction) {
    let temporary = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(temporary.path().to_path_buf());
    store.initialize_current_layout().unwrap();
    let path = store.client_settings_path(AppKind::Codex);
    let mut settings = default_client_settings(AppKind::Codex);
    settings.settings.remove(settings.settings.keys().next().cloned().as_ref().unwrap());
    fs::write(&path, serde_json::to_vec(&settings).unwrap()).unwrap();
    let mut transaction = Transaction::begin(&store).unwrap();
    super::super::files::copy_tree(&store.configuration_dir(), &transaction.staged()).unwrap();
    let repaired = super::super::repair_stage(&transaction.staged()).unwrap();
    transaction.prepare(repaired).unwrap();
    (temporary, store, transaction)
}

#[test]
fn every_directory_exchange_boundary_recovers_without_an_empty_store() {
    for boundary in [0, 1, 2] {
        let (_temporary, store, transaction) = prepared();
        if boundary >= 1 { fs::rename(transaction.live(), transaction.backup()).unwrap(); }
        if boundary >= 2 { fs::rename(transaction.staged(), transaction.live()).unwrap(); }
        assert!(store.initialize_current_layout().is_err());
        let report = store.recover_pending_repair().unwrap().unwrap();
        assert_eq!(report.repaired_files.len(), 1);
        assert_eq!(revision(&store.configuration_dir()).unwrap(), transaction.journal.candidate.unwrap());
        assert!(!store.state_root.join(JOURNAL).exists());
        assert!(store.recover_pending_repair().unwrap().is_none());
    }
}

#[test]
fn interrupted_build_is_discarded_only_with_an_unchanged_original() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(temporary.path().to_path_buf());
    store.initialize_current_layout().unwrap();
    let transaction = Transaction::begin(&store).unwrap();
    fs::create_dir_all(transaction.staged()).unwrap();
    fs::write(transaction.staged().join("partial"), b"generated").unwrap();
    assert!(store.recover_pending_repair().unwrap().is_none());
    assert!(!transaction.stage_root().exists());
    assert!(store.configuration_dir().is_dir());
}

#[test]
fn changed_original_or_candidate_preserves_the_journal_and_all_material() {
    for original_changed in [true, false] {
        let (_temporary, store, transaction) = prepared();
        let target = if original_changed { transaction.live() } else { transaction.staged() };
        fs::write(target.join("external-change"), b"retain").unwrap();
        assert!(store.recover_pending_repair().is_err());
        assert!(transaction.record().is_file());
        assert!(target.join("external-change").exists());
        assert!(transaction.live().is_dir());
    }
}

#[test]
fn unknown_recovery_records_are_rejected_without_rewriting() {
    let temporary = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(temporary.path().to_path_buf());
    let path = store.state_root.join(JOURNAL);
    for original in [b"{broken".as_slice(), br#"{"version":99,"id":"../../elsewhere"}"#] {
        fs::write(&path, original).unwrap();
        assert!(store.recover_pending_repair().is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(!store.configuration_dir().exists());
    }
}

#[test]
fn backup_conflict_never_overwrites_the_original() {
    let (_temporary, store, transaction) = prepared();
    fs::create_dir(transaction.backup()).unwrap();
    fs::write(transaction.backup().join("foreign"), b"keep").unwrap();
    let original = revision(&transaction.live()).unwrap();
    assert!(store.recover_pending_repair().is_err());
    assert_eq!(revision(&transaction.live()).unwrap(), original);
    assert_eq!(fs::read(transaction.backup().join("foreign")).unwrap(), b"keep");
}
