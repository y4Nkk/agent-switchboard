pub(crate) mod archive;
pub(crate) mod backup_cleanup;
pub(crate) mod publish;
pub(crate) mod sqlite;
mod cache;
mod sizes;
pub(crate) use archive::{ArchiveDigest, ArchiveWriter, HistoryArchivePlan};
pub(crate) use backup_cleanup::{execute as execute_backup_cleanup, preview as preview_backup_cleanup_plan, BackupCleanupPlan};
pub(crate) use cache::{clear_cache, spawn};
pub(crate) use sizes::{usage, StorageUsage};
use serde::Serialize;
use asb_core::contracts::{AppKind, LocalizedMessage};

#[derive(Default, Debug, Serialize)]
pub(crate) struct ReclaimOutcome { pub removed: u64, pub warning: Option<String> }

#[derive(Debug)]
pub(crate) enum ReclaimError { Changed, Failed(String) }
impl From<String> for ReclaimError { fn from(error: String) -> Self { Self::Failed(error) } }
pub(crate) enum ArchiveScope { Configuration(AppKind), Backups }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveOutcome {
    pub path: String,
    pub removed: u64,
    pub warnings: Vec<LocalizedMessage>,
}

impl ArchiveOutcome {
    fn add(&mut self, scope: ArchiveScope, result: Result<ReclaimOutcome, ReclaimError>) {
        match result {
            Ok(result) => { self.removed += result.removed; if let Some(warning) = result.warning { self.warn(scope, false, warning); } }
            Err(ReclaimError::Changed) => self.warn(scope, true, String::new()),
            Err(ReclaimError::Failed(error)) => self.warn(scope, false, error),
        }
    }
    fn warn(&mut self, scope: ArchiveScope, changed: bool, detail: String) {
        let (key, app) = match (scope, changed) {
            (ArchiveScope::Configuration(app), true) => ("storage.archive.configChanged", app.label()),
            (ArchiveScope::Configuration(app), false) => ("storage.archive.configFailed", app.label()),
            (ArchiveScope::Backups, true) => ("backup.cleanup.changed", ""),
            (ArchiveScope::Backups, false) => ("backup.cleanup.failed", ""),
        };
        let text = if changed { "已完整归档；历史变化，未清理变化后的记录".into() }
            else { format!("已完整归档；部分回收未完成：{detail}") };
        self.warnings.push(LocalizedMessage::new(key, serde_json::json!({"app":app,"detail":detail}), text));
    }
}
