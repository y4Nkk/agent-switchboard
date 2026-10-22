use super::*;
use crate::codex_probe::progress::{ProbeProgressView, ProbeRunPhase, ProbeRunProgressView};

fn fixture(id: &str, planned: u32) -> (tempfile::TempDir, ProbeLedger, ProbeRegistry) {
    let directory = tempfile::tempdir().unwrap();
    let ledger = ProbeLedger::new(directory.path());
    let registry = ProbeRegistry::new();
    registry.begin(id).unwrap();
    ledger.insert_batch(&ledger::NewProbeBatch {
        id: id.into(), started_at: ledger::now_rfc3339(), planned_runs: planned,
        question: ledger::ProbeQuestionSnapshot {
            id: "custom".into(), label: "Question".into(), text: "Count".into(), expected_answer: "45".into(),
        },
        grading_version: ProbeGrading::Integer, cli_version: None, config: Default::default(),
    }).unwrap();
    (directory, ledger, registry)
}

fn result(seq: u32) -> ledger::ProbeRunRecord {
    ledger::ProbeRunRecord { seq, status: ProbeRunStatus::Passed,
        final_answer: Some("45".into()), ..Default::default() }
}

fn clock_view(view: ProbeBatchView) -> ProbeRunProgressView {
    match view.progress.unwrap() {
        ProbeProgressView::Run { clock } => clock,
        ProbeProgressView::Preparing { .. } => panic!("expected an executing run"),
    }
}

#[test]
fn preparing_cancellation_uses_the_workers_signal_and_retains_the_slot() {
    let (_directory, ledger, registry) = fixture("live", 1);
    assert!(registry.begin("other").is_err());
    let view = registry.read_view(&ledger, None).unwrap().unwrap();
    assert!(matches!(view.progress, Some(ProbeProgressView::Preparing { seq: 1, cancelling: false })));
    assert!(registry.cancel());
    assert!(!registry.cancel());
    let view = registry.read_view(&ledger, None).unwrap().unwrap();
    assert!(matches!(view.progress, Some(ProbeProgressView::Preparing { seq: 1, cancelling: true })));
    registry.finish_batch(&ledger, "live", ProbeBatchStatus::Cancelled, None, vec![]).unwrap();
    assert!(!registry.cancel());
    assert!(registry.begin("other").is_ok());
}

#[test]
fn clocks_cannot_reset_or_skip_runs_and_old_handles_cannot_change_the_next_run() {
    let (_directory, ledger, registry) = fixture("live", 2);
    assert!(registry.start_run(&ledger, "live", 2, 180).is_err());
    let first = registry.start_run(&ledger, "live", 1, 180).unwrap();
    let deadline = first.deadline();
    assert!(registry.start_run(&ledger, "live", 1, 180).is_err());
    assert!(registry.start_run(&ledger, "live", 2, 180).is_err());
    assert!(registry.start_run(&ledger, "other", 1, 900).is_err());
    assert_eq!(first.deadline(), deadline);
    registry.save_run(&ledger, "live", result(1)).unwrap();
    let ready = registry.read_view(&ledger, None).unwrap().unwrap();
    assert_eq!(ready.completed_runs, 1);
    assert!(matches!(ready.progress, Some(ProbeProgressView::Preparing { seq: 2, .. })));
    assert!(registry.start_run(&ledger, "live", 1, 180).is_err());
    assert!(registry.start_run(&ledger, "live", 3, 180).is_err());
    let second = registry.start_run(&ledger, "live", 2, 180).unwrap();
    first.running();
    first.collecting();
    let active = clock_view(registry.read_view(&ledger, None).unwrap().unwrap());
    assert_eq!(active.seq, 2);
    assert_eq!(active.phase, ProbeRunPhase::Starting);
    assert!(registry.cancel());
    second.running();
    assert_eq!(clock_view(registry.read_view(&ledger, None).unwrap().unwrap()).phase, ProbeRunPhase::Cancelling);
}

#[test]
fn both_public_views_reconnect_to_one_clock_and_terminal_commit_removes_it() {
    let (directory, ledger, registry) = fixture("live", 1);
    let local = crate::local_state::LocalState::from_root(directory.path().to_path_buf());
    let clock = registry.start_run(&ledger, "live", 1, 300).unwrap();
    clock.running();
    let deadline = clock.deadline();
    let current = clock_view(current_view(&local, &registry).unwrap().unwrap());
    let detail = clock_view(batch_view(&local, &registry, "live").unwrap().unwrap());
    assert_eq!(current.time_limit_ms, 300_000);
    assert_eq!(detail.seq, current.seq);
    assert_eq!(detail.phase, current.phase);
    assert!(detail.elapsed_ms >= current.elapsed_ms);
    assert_eq!(clock.deadline(), deadline);
    clock.saving();
    assert_eq!(clock_view(current_view(&local, &registry).unwrap().unwrap()).phase, ProbeRunPhase::Saving);
    registry.finish_batch(&ledger, "live", ProbeBatchStatus::Completed, None, vec![result(1)]).unwrap();
    for view in [current_view(&local, &registry).unwrap().unwrap(), batch_view(&local, &registry, "live").unwrap().unwrap()] {
        assert_eq!(view.batch.status, ProbeBatchStatus::Completed);
        assert!(view.progress.is_none());
        assert_eq!(view.completed_runs, 1);
    }
}

#[test]
fn current_view_prefers_the_slot_over_wall_clock_order() {
    let (_directory, ledger, registry) = fixture("live", 1);
    let (mut future, _) = ledger.load_snapshot(Some("live")).unwrap().unwrap();
    future.id = "future".into();
    ledger.insert_batch(&ledger::NewProbeBatch {
        id: future.id, started_at: "2999-01-01T00:00:00.000Z".into(), planned_runs: 1,
        question: future.question, grading_version: future.grading_version,
        cli_version: future.cli_version, config: future.config,
    }).unwrap();
    ledger.finish_batch("future", ProbeBatchStatus::Completed, None, &[]).unwrap();
    assert_eq!(ledger.load_snapshot(None).unwrap().unwrap().0.id, "future");
    assert_eq!(registry.read_view(&ledger, None).unwrap().unwrap().batch.id, "live");
}

#[test]
fn missing_ownership_and_out_of_band_writes_are_errors_not_empty_progress() {
    let (_directory, ledger, registry) = fixture("live", 1);
    assert!(ProbeRegistry::new().read_view(&ledger, None).err().unwrap().contains("运行槽"));
    ledger.start_run("live", 1).unwrap();
    assert!(registry.read_view(&ledger, None).err().unwrap().contains("不一致"));
    registry.start_run(&ledger, "live", 1, 180).err().unwrap();
}

#[test]
fn a_direct_worker_cancellation_signal_is_visible_without_a_separate_phase_update() {
    let (_directory, ledger, registry) = fixture("live", 1);
    let clock = registry.start_run(&ledger, "live", 1, 180).unwrap();
    let slot = registry.slot.lock().unwrap();
    let Some(ProbeSlot::Running(live)) = slot.as_ref() else { panic!("running slot"); };
    live.cancel.store(true, Ordering::SeqCst);
    drop(slot);
    clock.collecting();
    clock.running();
    assert_eq!(clock_view(registry.read_view(&ledger, None).unwrap().unwrap()).phase, ProbeRunPhase::Cancelling);
}

#[test]
fn failed_terminal_save_retains_results_in_both_views_and_retry_owns_the_slot() {
    let (directory, ledger, registry) = fixture("pending", 1);
    let local = crate::local_state::LocalState::from_root(directory.path().to_path_buf());
    registry.start_run(&ledger, "pending", 1, 180).unwrap();
    let connection = rusqlite::Connection::open(directory.path().join("codex/probes.sqlite3")).unwrap();
    connection.execute_batch("CREATE TRIGGER block_finish BEFORE UPDATE OF status ON probe_batches
        BEGIN SELECT RAISE(ABORT, 'blocked finish'); END;").unwrap();
    assert!(registry.finish_batch(&ledger, "pending", ProbeBatchStatus::Completed, None, vec![result(1)]).is_err());
    assert!(registry.begin("other").is_err());
    assert!(!registry.cancel());
    assert!(registry.retry_save(&ledger).is_err());
    for view in [current_view(&local, &registry).unwrap().unwrap(), batch_view(&local, &registry, "pending").unwrap().unwrap()] {
        assert!(view.persist_pending);
        assert!(view.persist_error.unwrap().contains("blocked finish"));
        assert_eq!(view.runs[0].final_answer.as_deref(), Some("45"));
        assert_eq!(view.completed_runs, 1);
        assert!(view.progress.is_none());
    }
    connection.execute_batch("DROP TRIGGER block_finish").unwrap();
    registry.retry_save(&ledger).unwrap();
    assert!(!registry.read_view(&ledger, None).unwrap().unwrap().persist_pending);
    assert!(registry.begin("other").is_ok());
}

#[test]
fn intermediate_save_failure_retains_results_and_the_slot() {
    let (directory, ledger, registry) = fixture("pending", 2);
    registry.start_run(&ledger, "pending", 1, 180).unwrap();
    let connection = rusqlite::Connection::open(directory.path().join("codex/probes.sqlite3")).unwrap();
    connection.execute_batch("CREATE TRIGGER block_run BEFORE UPDATE OF status ON probe_runs
        BEGIN SELECT RAISE(ABORT, 'blocked run'); END;").unwrap();
    assert!(registry.save_run(&ledger, "pending", result(1)).is_err());
    let pending = registry.read_view(&ledger, None).unwrap().unwrap();
    assert!(pending.persist_pending);
    assert_eq!(pending.runs[0].status, ProbeRunStatus::Passed);
    assert!(registry.begin("other").is_err());
    connection.execute_batch("DROP TRIGGER block_run").unwrap();
    registry.retry_save(&ledger).unwrap();
    assert!(registry.begin("other").is_ok());
}

#[test]
fn run_creation_failure_retains_the_recorded_batch_until_save_retry() {
    let (_directory, ledger, registry) = fixture("failed-start", 1);
    registry.fail_start("failed-start", "创建失败".into()).unwrap();
    let pending = registry.read_view(&ledger, None).unwrap().unwrap();
    assert!(pending.persist_pending);
    assert_eq!(pending.batch.status, ProbeBatchStatus::Failed);
    assert!(pending.progress.is_none());
    assert!(registry.begin("other").is_err());
    registry.retry_save(&ledger).unwrap();
    assert!(registry.begin("other").is_ok());
}

#[test]
fn progress_wire_format_requires_an_explicit_stage_and_keeps_clock_fields_camel_case() {
    let ready = serde_json::to_value(ProbeProgressView::Preparing { seq: 1, cancelling: false }).unwrap();
    assert_eq!(ready["kind"], "preparing");
    assert!(ready.get("elapsedMs").is_none());
    let clock = progress::RunProgress::new(1, 180);
    let running = serde_json::to_value(ProbeProgressView::Run { clock: clock.view(false) }).unwrap();
    assert_eq!(running["kind"], "run");
    assert_eq!(running["timeLimitMs"], 180_000);
    assert_eq!(running["phase"], "starting");
    assert!(running.get("clock").is_none());
}
