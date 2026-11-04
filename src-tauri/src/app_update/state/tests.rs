use super::*;

fn fixture(phase: DownloadPhase) -> (UpdateManager, watch::Receiver<bool>) {
    let (cancel, signal) = watch::channel(false);
    let state = DownloadState { id: "task".into(), revision: 0, version: "9.0.0".into(), phase,
        downloaded_bytes: 0, total_bytes: None, error: None };
    let manager = UpdateManager(Arc::new(Mutex::new(Registry {
        checking: false, candidate: None, job: Some(Job { state, cancel }),
    })));
    (manager, signal)
}

#[test]
fn cancellation_during_verification_prevents_install_and_only_finishes_after_cleanup() {
    let (manager, signal) = fixture(DownloadPhase::Downloading);
    manager.progress("task", 12, Some(20));
    manager.verifying("task");
    let before = manager.read("task").unwrap();
    let acknowledged = manager.cancel("task").unwrap();
    assert_eq!(acknowledged.phase, DownloadPhase::Cancelling);
    assert!(acknowledged.revision > before.revision);
    assert!(*signal.borrow());
    assert!(!manager.begin_install("task").unwrap());
    assert!(manager.begin_check().is_err());
    manager.finish("task", Ok(false));
    let cancelled = manager.read("task").unwrap();
    assert_eq!(cancelled.phase, DownloadPhase::Cancelled);
    assert!(cancelled.revision > acknowledged.revision);
    assert!(cancelled.error.is_none());
    assert!(manager.begin_check().is_ok());
}

#[test]
fn committed_install_cannot_be_cancelled_or_started_again_before_restart() {
    let (manager, signal) = fixture(DownloadPhase::Verifying);
    assert!(manager.begin_install("task").unwrap());
    assert_eq!(manager.cancel("task").unwrap().phase, DownloadPhase::Installing);
    assert!(!*signal.borrow());
    manager.finish("task", Ok(true));
    assert_eq!(manager.read("task").unwrap().phase, DownloadPhase::Installed);
    assert_eq!(manager.begin_check().err().unwrap().code, "updater-busy");
    assert_eq!(manager.start("candidate").err().unwrap().code, "updater-busy");
}

#[test]
fn progress_and_verification_cannot_revive_a_cancelled_or_obsolete_task() {
    let (manager, _signal) = fixture(DownloadPhase::Downloading);
    manager.progress("obsolete", 500, Some(1000));
    manager.progress("task", 3, Some(0));
    let before = manager.cancel("task").unwrap();
    assert_eq!(before.total_bytes, None);
    manager.progress("task", 900, Some(1000));
    manager.verifying("task");
    let after = manager.read("task").unwrap();
    assert_eq!(after.downloaded_bytes, 3);
    assert_eq!(after.phase, DownloadPhase::Cancelling);
    assert_eq!(after.revision, before.revision);
    assert_eq!(manager.read("obsolete").err().unwrap().code, "updater-task-missing");
}

#[test]
fn failed_tasks_keep_diagnostics_and_release_the_gate_for_a_new_check() {
    let (manager, _signal) = fixture(DownloadPhase::Downloading);
    manager.finish("task", Err(CommandError::new("fixture-network-failed", "connection interrupted")));
    let failed = manager.read("task").unwrap();
    assert_eq!(failed.phase, DownloadPhase::Failed);
    assert_eq!(failed.error.unwrap().code, "fixture-network-failed");
    let check = manager.begin_check().unwrap();
    assert_eq!(manager.start("old").err().unwrap().code, "updater-busy");
    assert!(manager.begin_check().is_err());
    drop(check);
    assert_eq!(manager.start("old").err().unwrap().code, "updater-candidate-stale");
    assert!(manager.begin_check().is_ok());
}

#[test]
fn accepted_cancellation_wins_over_a_simultaneous_network_failure() {
    let (manager, _signal) = fixture(DownloadPhase::Downloading);
    manager.cancel("task").unwrap();
    manager.finish("task", Err(CommandError::new("fixture-network-failed", "connection interrupted")));
    let state = manager.read("task").unwrap();
    assert_eq!(state.phase, DownloadPhase::Cancelled);
    assert!(state.error.is_none());
}
