use super::{Report, Stage};
use crate::{commands::error::CommandError, local_state::AppSettings};
use serde::Serialize;
use std::sync::{atomic::{AtomicBool, Ordering}, mpsc::SyncSender, Mutex};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Status { Running, Ready, Degraded, Blocked }

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    pub session_id: String,
    pub run_id: u64,
    pub revision: u64,
    pub status: Status,
    pub stage: Stage,
    pub elapsed_ms: u64,
    #[serde(flatten)]
    pub report: Report,
}

pub(crate) struct Preparation {
    state: Mutex<(Snapshot, Instant)>,
    wake: SyncSender<()>,
    pub(super) force: AtomicBool,
    pub(super) initialized: AtomicBool,
    lifecycle: Mutex<Lifecycle>,
}

#[derive(Default)]
struct Lifecycle { checking: bool, stopping: bool, exit_code: Option<i32> }

impl Preparation {
    pub(crate) fn new(settings: Option<AppSettings>, wake: SyncSender<()>) -> Self {
        Self { state: Mutex::new((Snapshot { session_id: uuid::Uuid::new_v4().to_string(), run_id: 0, revision: 0, status: Status::Running,
            stage: Stage::Preferences, elapsed_ms: 0, report: Report { settings, ..Default::default() } }, Instant::now())),
            wake, force: AtomicBool::new(false), initialized: AtomicBool::new(false), lifecycle: Mutex::new(Lifecycle::default()) }
    }

    pub(crate) fn snapshot(&self) -> Result<Snapshot, CommandError> {
        let state = self.state.lock().map_err(|_| unavailable())?;
        let mut snapshot = state.0.clone();
        if snapshot.status == Status::Running { snapshot.elapsed_ms = state.1.elapsed().as_millis() as u64; }
        Ok(snapshot)
    }

    pub(super) fn publish(&self, stage: Stage, status: Status, mut report: Report, new_run: bool) -> Result<Snapshot, CommandError> {
        let mut state = self.state.lock().map_err(|_| unavailable())?;
        if new_run { state.0.run_id += 1; state.1 = Instant::now(); }
        if report.settings.is_none() && status == Status::Running { report.settings = state.0.report.settings.clone(); }
        state.0.revision += 1;
        state.0.stage = stage; state.0.status = status; state.0.report = report;
        state.0.elapsed_ms = state.1.elapsed().as_millis() as u64;
        Ok(state.0.clone())
    }

    pub(crate) fn require_ready(&self) -> Result<(), CommandError> {
        if !self.initialized.load(Ordering::Acquire) || matches!(self.snapshot()?.status, Status::Running | Status::Blocked) {
            return Err(CommandError::keyed("application-preparing", "errors.preparation.running", "应用数据正在准备，请稍后重试"));
        }
        Ok(())
    }

    pub(crate) fn request_check(&self) -> Result<(), CommandError> {
        self.force.store(true, Ordering::Release);
        match self.wake.try_send(()) {
            Ok(()) | Err(std::sync::mpsc::TrySendError::Full(_)) => Ok(()),
            Err(_) => Err(unavailable()),
        }
    }

    pub(super) fn claim(&self, first: bool) -> Result<bool, String> {
        let mut lifecycle = self.lifecycle.lock().map_err(|_| "准备流程生命周期锁不可用")?;
        if lifecycle.stopping && !first { return Ok(false); }
        lifecycle.checking = true;
        Ok(true)
    }

    pub(super) fn complete_check(&self) -> Option<i32> {
        let mut lifecycle = self.lifecycle.lock().ok()?;
        lifecycle.checking = false;
        lifecycle.exit_code.take()
    }

    pub(crate) fn defer_exit(&self, code: Option<i32>) -> bool {
        let Ok(mut lifecycle) = self.lifecycle.lock() else { return true; };
        let running = lifecycle.checking || self.snapshot().is_ok_and(|state| state.status == Status::Running);
        if running { lifecycle.stopping = true; lifecycle.exit_code = Some(code.unwrap_or(0)); }
        running
    }

    pub(crate) fn prepare_exit(&self, code: Option<i32>) -> bool {
        let Ok(mut lifecycle) = self.lifecycle.lock() else { return true; };
        lifecycle.stopping = true;
        let running = lifecycle.checking || self.snapshot().is_ok_and(|state| state.status == Status::Running);
        if running { lifecycle.exit_code = Some(code.unwrap_or(0)); }
        running
    }

    pub(crate) fn resume_after_blocked_exit(&self) {
        if let Ok(mut lifecycle) = self.lifecycle.lock() {
            lifecycle.stopping = false; lifecycle.exit_code = None;
        }
        let _ = self.request_check();
    }

}

fn unavailable() -> CommandError {
    CommandError::keyed("application-preparation-unavailable", "errors.preparation.unavailable", "应用准备状态不可用，请查看诊断日志")
}
