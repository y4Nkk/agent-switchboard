use crate::commands::error::CommandError;
use serde::Serialize;
use std::sync::{Arc, Mutex, MutexGuard};
use tauri_plugin_updater::Update;
use tokio::sync::watch;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateCheck {
    pub id: String,
    pub latest_version: String,
    pub release_notes: Option<String>,
    pub checked_at: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DownloadPhase {
    Downloading, Verifying, Cancelling, Installing, Installed, Cancelled, Failed,
}

impl DownloadPhase {
    pub fn active(self) -> bool {
        matches!(self, Self::Downloading | Self::Verifying | Self::Cancelling | Self::Installing)
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadState {
    pub id: String,
    pub revision: u64,
    pub version: String,
    pub phase: DownloadPhase,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub error: Option<CommandError>,
}

struct Candidate { metadata: UpdateCheck, update: Update }
struct Job { state: DownloadState, cancel: watch::Sender<bool> }
#[derive(Default)]
struct Registry { checking: bool, candidate: Option<Candidate>, job: Option<Job> }

#[derive(Clone, Default)]
pub(crate) struct UpdateManager(Arc<Mutex<Registry>>);

pub(super) struct CheckGuard(UpdateManager);
impl Drop for CheckGuard {
    fn drop(&mut self) {
        if let Ok(mut registry) = self.0.lock() { registry.checking = false; }
    }
}

fn busy() -> CommandError {
    CommandError::keyed("updater-busy", "errors.updater.busy", "更新操作正在进行，请等待完成后重试。")
}

impl UpdateManager {
    fn lock(&self) -> Result<MutexGuard<'_, Registry>, CommandError> {
        self.0.lock().map_err(|_| CommandError::keyed("updater-state-unavailable",
            "errors.updater.stateUnavailable", "更新状态不可用，请重启应用后重试。"))
    }

    pub(super) fn begin_check(&self) -> Result<CheckGuard, CommandError> {
        let mut registry = self.lock()?;
        if registry.checking || registry.job.as_ref().is_some_and(|job|
            job.state.phase.active() || job.state.phase == DownloadPhase::Installed) {
            return Err(busy());
        }
        registry.checking = true;
        Ok(CheckGuard(self.clone()))
    }

    pub(super) fn checked(&self, update: Option<Update>) -> Result<Option<UpdateCheck>, CommandError> {
        let candidate = update.map(|update| Candidate { metadata: UpdateCheck {
            id: uuid::Uuid::new_v4().to_string(), latest_version: update.version.clone(),
            release_notes: update.body.as_deref().map(str::trim).filter(|body| !body.is_empty()).map(str::to_owned),
            checked_at: chrono::Utc::now().to_rfc3339(),
        }, update });
        let metadata = candidate.as_ref().map(|candidate| candidate.metadata.clone());
        self.lock()?.candidate = candidate;
        Ok(metadata)
    }

    pub(super) fn start(&self, update_id: &str) -> Result<(Update, DownloadState, watch::Receiver<bool>), CommandError> {
        let mut registry = self.lock()?;
        if registry.checking || registry.job.as_ref().is_some_and(|job|
            job.state.phase.active() || job.state.phase == DownloadPhase::Installed) {
            return Err(busy());
        }
        let candidate = registry.candidate.as_ref().filter(|candidate| candidate.metadata.id == update_id)
            .ok_or_else(|| CommandError::keyed("updater-candidate-stale", "errors.updater.candidateStale",
                "更新信息已变化，请重新检查更新。"))?;
        let update = candidate.update.clone();
        let state = DownloadState { id: uuid::Uuid::new_v4().to_string(), revision: 0,
            version: candidate.metadata.latest_version.clone(), phase: DownloadPhase::Downloading,
            downloaded_bytes: 0, total_bytes: None, error: None };
        let (cancel, signal) = watch::channel(false);
        registry.job = Some(Job { state: state.clone(), cancel });
        Ok((update, state, signal))
    }

    fn job<'a>(registry: &'a mut Registry, id: &str) -> Result<&'a mut Job, CommandError> {
        registry.job.as_mut().filter(|job| job.state.id == id).ok_or_else(||
            CommandError::keyed("updater-task-missing", "errors.updater.taskMissing", "下载任务已不存在，请重新下载。"))
    }

    pub(super) fn read(&self, id: &str) -> Result<DownloadState, CommandError> {
        let mut registry = self.lock()?;
        Ok(Self::job(&mut registry, id)?.state.clone())
    }

    pub(super) fn cancel(&self, id: &str) -> Result<DownloadState, CommandError> {
        let mut registry = self.lock()?;
        let job = Self::job(&mut registry, id)?;
        if matches!(job.state.phase, DownloadPhase::Downloading | DownloadPhase::Verifying) {
            job.state.phase = DownloadPhase::Cancelling;
            job.state.revision += 1;
            let _ = job.cancel.send(true);
        }
        Ok(job.state.clone())
    }

    pub(super) fn progress(&self, id: &str, bytes: usize, total: Option<u64>) {
        if let Ok(mut registry) = self.lock() {
            if let Ok(job) = Self::job(&mut registry, id) {
                if job.state.phase == DownloadPhase::Downloading {
                    job.state.downloaded_bytes = job.state.downloaded_bytes.saturating_add(bytes as u64);
                    job.state.total_bytes = total.filter(|total| *total > 0);
                    job.state.revision += 1;
                }
            }
        }
    }

    pub(super) fn verifying(&self, id: &str) {
        if let Ok(mut registry) = self.lock() {
            if let Ok(job) = Self::job(&mut registry, id) {
                if job.state.phase == DownloadPhase::Downloading {
                    job.state.phase = DownloadPhase::Verifying;
                    job.state.revision += 1;
                }
            }
        }
    }

    /// Cancellation and the irreversible install commit are ordered by this lock.
    pub(super) fn begin_install(&self, id: &str) -> Result<bool, CommandError> {
        let mut registry = self.lock()?;
        let job = Self::job(&mut registry, id)?;
        if job.state.phase == DownloadPhase::Cancelling { return Ok(false); }
        if job.state.phase != DownloadPhase::Verifying { return Err(busy()); }
        job.state.phase = DownloadPhase::Installing;
        job.state.revision += 1;
        Ok(true)
    }

    pub(super) fn finish(&self, id: &str, result: Result<bool, CommandError>) {
        let Ok(mut registry) = self.lock() else { return; };
        let Ok(job) = Self::job(&mut registry, id) else { return; };
        let installed = matches!(result, Ok(true));
        match result {
            _ if job.state.phase == DownloadPhase::Cancelling => job.state.phase = DownloadPhase::Cancelled,
            Ok(false) => job.state.phase = DownloadPhase::Cancelled,
            Ok(true) => job.state.phase = DownloadPhase::Installed,
            Err(error) => { job.state.phase = DownloadPhase::Failed; job.state.error = Some(error); }
        }
        job.state.revision += 1;
        if installed { registry.candidate = None; }
    }
}

#[cfg(test)]
mod tests;
