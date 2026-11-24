use std::collections::{btree_map::Entry, BTreeMap};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use serde::Serialize;
use uuid::Uuid;

use super::SkillRepository;
use crate::extensions::sources::{GitHubRequest, HttpTransfer};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogScanProgress {
    elapsed_ms: u64,
    repositories: Vec<RepositoryProgress>,
}

#[derive(Clone, Serialize)]
struct RepositoryProgress {
    repository: SkillRepository,
    #[serde(flatten)]
    state: RepositoryState,
    requests: Vec<RequestProgress>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub(super) enum RepositoryState {
    Queued,
    Running,
    Complete { candidate_count: usize },
    Failed { message: String },
}

#[derive(Clone, Serialize)]
struct RequestProgress {
    step: GitHubRequest,
    state: RequestState,
    #[serde(flatten)]
    transfer: HttpTransfer,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum RequestState {
    Waiting,
    Reading,
    Received,
    Failed,
}

struct ActiveScan {
    started_at: Instant,
    repositories: Vec<RepositoryProgress>,
}

static ACTIVE_SCANS: Mutex<BTreeMap<Uuid, ActiveScan>> = Mutex::new(BTreeMap::new());

fn active_scans() -> MutexGuard<'static, BTreeMap<Uuid, ActiveScan>> {
    ACTIVE_SCANS.lock().unwrap_or_else(|error| error.into_inner())
}

/// The in-flight command owns the live snapshot. The response carries the final reading.
pub(crate) struct CatalogScanMonitor(Uuid);

impl CatalogScanMonitor {
    pub(crate) fn begin(id: Uuid, started_at: Instant, repos: &[SkillRepository]) -> Option<Self> {
        let mut scans = active_scans();
        let Entry::Vacant(slot) = scans.entry(id) else { return None };
        let repositories = repos.iter().map(|repo| RepositoryProgress {
            repository: repo.clone(),
            state: RepositoryState::Queued,
            requests: Vec::new(),
        }).collect();
        slot.insert(ActiveScan { started_at, repositories });
        Some(Self(id))
    }

    pub(crate) fn read(id: Uuid) -> Option<CatalogScanProgress> {
        let scans = active_scans();
        let scan = scans.get(&id)?;
        Some(CatalogScanProgress {
            elapsed_ms: scan.started_at.elapsed().as_millis() as u64,
            repositories: scan.repositories.clone(),
        })
    }

    pub(crate) fn snapshot(&self) -> CatalogScanProgress {
        Self::read(self.0).expect("live progress owned by this scan")
    }

    pub(super) fn set_state(&self, index: usize, state: RepositoryState) {
        self.update(index, |repo| repo.state = state);
    }

    pub(super) fn update_request(&self, index: usize, step: GitHubRequest, transfer: HttpTransfer) {
        let state = if transfer.http_status.is_some() { RequestState::Reading } else { RequestState::Waiting };
        self.update(index, |repo| {
            let next = RequestProgress { step, state, transfer };
            if let Some(request) = repo.requests.last_mut().filter(|request| request.step == step) {
                *request = next;
            } else {
                repo.requests.push(next);
            }
        });
    }

    pub(super) fn finish_request(&self, index: usize, step: GitHubRequest, received: bool) {
        self.update(index, |repo| {
            if let Some(request) = repo.requests.last_mut().filter(|request| request.step == step) {
                request.state = if received { RequestState::Received } else { RequestState::Failed };
            }
        });
    }

    fn update(&self, index: usize, update: impl FnOnce(&mut RepositoryProgress)) {
        let mut scans = active_scans();
        let scan = scans.get_mut(&self.0).expect("live progress owned by this scan");
        update(&mut scan.repositories[index]);
    }
}

impl Drop for CatalogScanMonitor {
    fn drop(&mut self) {
        active_scans().remove(&self.0);
    }
}
