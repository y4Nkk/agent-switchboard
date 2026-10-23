//! Owns the question catalog, the persisted batch lifecycle, and the single
//! active probe's execution. Durable status lives in the ledger; the registry
//! keeps the control handle, live clock and results awaiting a save retry.
mod ledger;
mod output;
mod questions;
mod runner;
mod process;
mod snapshot;
mod config_watch;
mod progress;
#[cfg(test)]
mod tests;

use ledger::ProbeRunStatus;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
pub(crate) use ledger::{
    ProbeBatchStatus, ProbeHistoryPage, ProbeHistoryProfile, ProbeHistoryQuery,
    ProbeLedger, ProbeProfileFilter,
};
pub(crate) use questions::{resolve_question, ProbeGrading, ResolvedQuestion, QUESTIONS};
use runner::{cli_version, run_once, ProbeAbort};
#[cfg(test)]
pub(crate) use runner::codex_executable;
pub(crate) const MAX_RUN_COUNT: u32 = 10;

/// Automatic grading outcome. Visual answers, execution failures,
/// cancellation and interruption never grade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeRunOutcome {
    Passed,
    Failed,
    Undetermined,
}

/// One finished CLI call, before it is stored.
#[derive(Clone, Debug)]
pub(crate) struct ProbeRunResult {
    pub outcome: ProbeRunOutcome,
    pub final_answer: Option<String>,
    pub session_id: Option<String>,
    pub reasoning_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub model: Option<String>,
    pub usage_error: Option<String>,
    pub duration_ms: u64,
    /// Execution or protocol failure, with scrubbed diagnostic details.
    pub error: Option<String>,
}

/// The serialized batch view served to the UI: the ledger record, its runs,
/// the in-memory "results not yet saved" flag and transient run progress.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProbeBatchView {
    #[serde(flatten)]
    pub batch: ledger::ProbeBatchRecord,
    pub completed_runs: u32,
    pub runs: Vec<ledger::ProbeRunRecord>,
    pub persist_pending: bool,
    pub persist_error: Option<String>,
    pub progress: Option<progress::ProbeProgressView>,
}

/// Results that could not be written to the ledger, kept in memory until the
/// retry entry flushes them. Never shown as archived.
#[derive(Clone, Debug)]
pub(crate) struct UnsavedProbe {
    pub batch_id: String,
    pub pending_runs: Vec<ledger::ProbeRunRecord>,
    pub terminal: (ledger::ProbeBatchStatus, Option<String>),
    pub save_error: String,
}

mod registry;
pub(crate) use registry::ProbeRegistry;
mod execution;
pub(crate) use execution::spawn_probe;

/// Events invalidate the shared read model; the registry and ledger own all data.
pub(crate) fn notify_changed(app: &tauri::AppHandle, batch_id: &str) {
    use tauri::Emitter;
    if let Err(error) = app.emit("codex-probe-changed", batch_id) {
        log::warn!("无法通知检测状态变化：{error}");
    }
}

// -------------------------------------------------------------- read paths

/// The live-or-latest batch view the radar panel reconnects to after a
/// refresh or restart.
pub(crate) fn current_view(
    local: &crate::local_state::LocalState,
    registry: &ProbeRegistry,
) -> Result<Option<ProbeBatchView>, String> {
    let ledger = ProbeLedger::new(local.root());
    registry.read_view(&ledger, None)
}

/// One historical batch by id.
pub(crate) fn batch_view(
    local: &crate::local_state::LocalState,
    registry: &ProbeRegistry,
    batch_id: &str,
) -> Result<Option<ProbeBatchView>, String> {
    let ledger = ProbeLedger::new(local.root());
    registry.read_view(&ledger, Some(batch_id))
}

fn assemble_view(
    mut batch: ledger::ProbeBatchRecord,
    mut runs: Vec<ledger::ProbeRunRecord>,
    pending: Option<UnsavedProbe>,
    progress: Option<progress::ProbeProgressView>,
) -> ProbeBatchView {
    let persist_pending = pending.is_some();
    let persist_error = pending.as_ref().map(|pending| format!("检测结果尚未保存：{}", pending.save_error));
    if let Some(pending) = pending {
        for record in pending.pending_runs {
            if let Some(stored) = runs.iter_mut().find(|run| run.seq == record.seq) { *stored = record; }
            else { runs.push(record); }
        }
        runs.sort_by_key(|run| run.seq);
        batch.status = pending.terminal.0;
        batch.status_error = pending.terminal.1;
    }
    let completed_runs = runs
        .iter()
        .filter(|run| run.status != ProbeRunStatus::Running)
        .count() as u32;
    ProbeBatchView {
        batch,
        completed_runs,
        runs,
        persist_pending,
        persist_error,
        progress,
    }
}

/// Replays pending results without releasing their slot until commit succeeds.
pub(crate) fn retry_save(
    local: &crate::local_state::LocalState,
    registry: &ProbeRegistry,
) -> Result<(), String> {
    registry.retry_save(&ProbeLedger::new(local.root()))
}

// ------------------------------------------------------ startup and exit

/// Marks batches left running by an abrupt exit as interrupted: finished
/// results stay, saved session ids get their usage backfilled, and no run is
/// graded after the fact.
pub(crate) fn recover_interrupted(local: &crate::local_state::LocalState) -> Result<u32, String> {
    ProbeLedger::new(local.root()).recover_interrupted()
}

/// Exit cleanup: stop the in-flight call (terminating the child process so
/// no probe keeps spending quota), wait for the worker, and record the
/// cancellation.
pub(crate) fn shutdown_on_exit(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    let Some(registry) = app.try_state::<ProbeRegistry>() else { return Ok(()); };
    let result = (|| {
        registry.request_shutdown(Duration::from_secs(10))?;
        let local = crate::local_state::LocalState::from_app(app)?;
        retry_save(&local, &registry)
    })();
    if result.is_err() { registry.resume_after_blocked_exit(); }
    result
}
