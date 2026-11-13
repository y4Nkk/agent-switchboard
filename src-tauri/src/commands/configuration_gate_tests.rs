use super::ConfigWriteGate;
use crate::config_store::{snapshot, ConfigStore};
use asb_core::AppKind;
use std::{fs, sync::mpsc, time::Duration};

#[test]
fn queued_configuration_read_waits_for_directory_replacement() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("state");
    let store = ConfigStore::new(root.clone());
    let settings = store.client_settings_path(AppKind::Codex);
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    fs::write(&settings, serde_json::to_vec(&asb_core::ownership::default_client_settings(AppKind::Codex)).unwrap()).unwrap();
    let original = fs::read(&settings).unwrap();
    let gate = ConfigWriteGate::default();
    let writer = gate.lock().unwrap();
    let live = store.configuration_dir();
    let retired = root.join("retired");
    fs::rename(&live, &retired).unwrap();
    let reader_gate = gate.clone();
    let (started_tx, started_rx) = mpsc::channel();
    let (entered_tx, entered_rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        tauri::async_runtime::block_on(reader_gate.run(move || {
            entered_tx.send(()).unwrap();
            snapshot::read_configuration_snapshot(&store).map_err(super::error::store_error)
        }))
    });
    started_rx.recv().unwrap();
    assert!(matches!(entered_rx.recv_timeout(Duration::from_millis(50)), Err(mpsc::RecvTimeoutError::Timeout)));
    fs::rename(&retired, &live).unwrap();
    drop(writer);
    let snapshot = reader.join().unwrap().unwrap();
    snapshot::validate_snapshot(&snapshot).unwrap();
    assert_eq!(fs::read(&settings).unwrap(), original);
}
