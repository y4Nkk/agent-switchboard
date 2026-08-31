//! The typed command error and the shared command guards.
//!
//! Every command maps its failures onto [`CommandError`] so the UI receives
//! a stable code plus a scrubbed, user-readable message.

use crate::local_state::{LocalState, ProfileStoreError};
use asb_core::adapter;
use serde::Serialize;
use tauri::AppHandle;

/// Structured command error surfaced to the UI as a typed object.
#[derive(Debug, Clone, Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl CommandError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: adapter::scrub_message(message.into()),
        }
    }
}

pub(crate) fn store_error(error: ProfileStoreError) -> CommandError {
    let code = match error {
        ProfileStoreError::Unreadable => "store-unreadable",
        ProfileStoreError::Unsupported => "profile-store-unsupported",
    };
    CommandError::new(code, error.to_string())
}

impl From<asb_switch::SwitchError> for CommandError {
    fn from(error: asb_switch::SwitchError) -> Self {
        let code = match &error {
            asb_switch::SwitchError::ReadCurrent { .. } => "read-current",
            asb_switch::SwitchError::PlanRejected { .. } => "plan-rejected",
            asb_switch::SwitchError::BlockedByLock { .. } => "blocked-by-lock",
            asb_switch::SwitchError::ExternalChange { .. } => "external-change",
            asb_switch::SwitchError::PlanChanged => "preview-stale",
            asb_switch::SwitchError::CommitFailed { .. } => "commit-failed",
        };
        CommandError::new(code, error.to_string())
    }
}

pub(crate) fn state(app: &AppHandle) -> Result<LocalState, CommandError> {
    LocalState::from_app(app).map_err(|error| CommandError::new("app-state-unavailable", error))
}

/// Runs one blocking unit of command work on the dedicated blocking pool.
/// Every command that touches files, the CC Switch database, or the network
/// goes through here: Tauri runs plain synchronous commands on the main
/// thread, so inline I/O would freeze the window for the whole operation.
/// The JoinError branch only triggers when the task panicked.
pub(crate) async fn blocking<T, F>(task: F) -> Result<T, CommandError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, CommandError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|_| CommandError::new("task-interrupted", "后台任务已中断".to_string()))?
}

pub(crate) fn require_write_confirmation(
    confirm_write: bool,
    operation: &str,
) -> Result<(), CommandError> {
    if confirm_write {
        return Ok(());
    }
    Err(CommandError::new(
        "write-not-confirmed",
        format!("{operation}前必须进行显式确认"),
    ))
}

const AUDIT_LOG_WARNING: &str = "配置已写入，但本地审计记录保存失败；本次操作无法从应用中撤回";

pub(crate) fn record_audit_or_warn(result: Result<(), String>, warnings: &mut Vec<String>) {
    if result.is_err() {
        warnings.push(AUDIT_LOG_WARNING.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_store_errors_have_stable_codes() {
        assert_eq!(
            store_error(ProfileStoreError::Unsupported).code,
            "profile-store-unsupported"
        );
        assert_eq!(
            store_error(ProfileStoreError::Unreadable).code,
            "store-unreadable"
        );
    }

    #[test]
    fn writes_require_explicit_confirmation() {
        let error = require_write_confirmation(false, "写入配置").expect_err("must reject");
        assert_eq!(error.code, "write-not-confirmed");
        assert!(error.message.contains("显式确认"));
        assert!(require_write_confirmation(true, "写入配置").is_ok());
        assert!(require_write_confirmation(false, "重置供应商数据").is_err());
    }

    #[test]
    fn successful_configuration_write_reports_audit_store_failure_as_a_warning() {
        let mut warnings = vec![];
        record_audit_or_warn(Err("store unavailable".to_string()), &mut warnings);
        assert_eq!(warnings, vec![AUDIT_LOG_WARNING.to_string()]);

        record_audit_or_warn(Ok(()), &mut warnings);
        assert_eq!(warnings.len(), 1);
    }
}
