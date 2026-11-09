//! Explicit Codex history metadata writes; ordinary session reads remain read-only.
// Note: .agents/notes/implemented/feature/2026-11-09-codex-session-provider-transfer.md
mod database;
mod inventory;
mod journal;
mod transcript;
mod transaction;
#[cfg(test)] mod tests;

use serde::{Deserialize, Serialize};
use std::{path::{Path, PathBuf}, sync::Mutex};

const TARGET: &str = asb_core::ownership::CODEX_PROVIDER_ID;
static GATE: Mutex<()> = Mutex::new(());
type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error { Busy(Option<u32>), Changed, Conflict(String), Recovery(String), Failure(String) }
impl From<String> for Error { fn from(value: String) -> Self { Self::Failure(value) } }
impl From<&str> for Error { fn from(value: &str) -> Self { Self::Failure(value.into()) } }
impl From<std::io::Error> for Error { fn from(value: std::io::Error) -> Self { Self::Failure(value.to_string()) } }
impl From<rusqlite::Error> for Error { fn from(value: rusqlite::Error) -> Self { Self::Failure(value.to_string()) } }
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy(pid) => write!(f, "Codex must be closed before changing history ({pid:?})"),
            Self::Changed => f.write_str("History changed; create a new preview"),
            Self::Conflict(detail) | Self::Recovery(detail) | Self::Failure(detail) => f.write_str(detail),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Scope { home: PathBuf, sqlite: PathBuf }
impl Scope {
    fn resolve() -> Result<Self> {
        let home = super::codex_root()?;
        let sqlite = crate::local_state::codex_paths::sqlite_root(&home)?;
        Ok(Self { home: inventory::absolute(&home)?, sqlite: inventory::absolute(&sqlite)? })
    }
    fn database(&self) -> PathBuf { self.sqlite.join("state_5.sqlite") }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderGroup { pub provider: Option<String>, pub database_rows: usize, pub files: usize }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preview {
    pub id: String, pub target_provider: String, pub sessions: usize,
    pub database_rows: usize, pub files: usize, pub missing_originals: usize,
    pub history_root: String, pub database_path: Option<String>, pub external_files: Vec<String>,
    pub providers: Vec<ProviderGroup>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase { Applying, Applied, Restoring, Restored }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation { pub preview: Preview, pub phase: Phase, pub backup_directory: String }

fn idle() -> Result<()> {
    use crate::resource_monitor::client_processes::{codex_history_occupant, HistoryProcessCheck};
    match codex_history_occupant()? {
        HistoryProcessCheck::Idle => Ok(()),
        HistoryProcessCheck::Running(pid) => Err(Error::Busy(Some(pid))),
        HistoryProcessCheck::Uncertain => Err(Error::Busy(None)),
    }
}

pub fn preview(state: &Path) -> Result<Preview> {
    let _gate = GATE.lock().map_err(|_| "Session transfer gate unavailable")?;
    preview_at(state, &Scope::resolve()?, &idle)
}

fn preview_at(state: &Path, scope: &Scope, check: &dyn Fn() -> Result<()>) -> Result<Preview> {
    check()?;
    journal::require_settled(state)?;
    let snapshot = inventory::scan(scope)?;
    let id = uuid::Uuid::new_v4().to_string();
    let view = inventory::summary(&id, scope, &snapshot);
    let plan = journal::Plan { version: 1, scope: scope.clone(), snapshot, view: view.clone(),
        created_at: chrono::Utc::now().timestamp() };
    journal::save(&journal::plan_path(state), &plan)?;
    Ok(view)
}

pub fn execute(state: &Path, id: &str) -> Result<Operation> {
    let _gate = GATE.lock().map_err(|_| "Session transfer gate unavailable")?;
    let _lease = crate::client_software::gate::history_transfer()?;
    transaction::execute(state, &Scope::resolve()?, id, &idle)
}

pub fn restore(state: &Path, id: &str) -> Result<Operation> {
    let _gate = GATE.lock().map_err(|_| "Session transfer gate unavailable")?;
    let _lease = crate::client_software::gate::history_transfer()?;
    transaction::restore(state, &Scope::resolve()?, id, &idle)
}

pub fn latest(state: &Path) -> Result<Option<Operation>> {
    let _gate = GATE.lock().map_err(|_| "Session transfer gate unavailable")?;
    journal::latest(state)?.map(|record| journal::view(state, &record)).transpose()
}

pub fn ensure_settled(state: &Path) -> Result<()> {
    let _gate = GATE.lock().map_err(|_| "Session transfer gate unavailable")?;
    journal::require_settled(state)
}
