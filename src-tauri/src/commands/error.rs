//! The typed command error and the shared command guards.
//!
//! Every command maps its failures onto [`CommandError`] so the UI receives
//! a stable code plus a scrubbed, user-readable message. App-owned
//! explanations additionally carry a `message_key` (+ structured `params`)
//! so the renderer can present them in the current interface language;
//! `message` remains the scrubbed raw diagnostic and the fallback rendering.

use crate::config_store::{ProfileStoreError, StoreOperationError};
use crate::local_state::LocalState;
use crate::runtime_log::{self, RuntimeLogAction};
use asb_core::adapter;
use serde::Serialize;
use std::future::Future;
use tauri::AppHandle;

/// Structured command error surfaced to the UI as a typed object.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
    /// App-owned explanation key; external diagnostics without a catalog
    /// entry retain their scrubbed `message`. Keys may come from core
    /// `LocalizedMessage` values, so this is a runtime string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_key: Option<String>,
    /// Interpolation values for `message_key`. Parameters whose name ends in
    /// `Key` carry catalog keys themselves and are resolved recursively by
    /// the renderer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl CommandError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: adapter::scrub_message(message.into()),
            message_key: None,
            params: None,
        }
    }

    /// A structured, translatable failure: `key` names the renderer-side
    /// template, `message` stays the scrubbed diagnostic detail/fallback.
    /// `params` may carry user values and nested catalog keys (`*Key`).
    /// Every string parameter is scrubbed individually before it crosses
    /// the boundary.
    pub(crate) fn localized(
        code: &'static str,
        key: &'static str,
        message: impl Into<String>,
        params: serde_json::Value,
    ) -> Self {
        Self {
            code,
            message: adapter::scrub_message(message.into()),
            message_key: Some(key.to_string()),
            params: scrub_params(params),
        }
    }

    /// Convenience for `localized` without parameters.
    pub(crate) fn keyed(code: &'static str, key: &'static str, message: impl Into<String>) -> Self {
        Self::localized(code, key, message, serde_json::Value::Null)
    }

    /// Maps one core-owned localized message, keeping its runtime key and
    /// params; the text stays the scrubbed diagnostic detail.
    pub(crate) fn from_localized(
        code: &'static str,
        message: asb_core::contracts::LocalizedMessage,
    ) -> Self {
        Self {
            code,
            message: adapter::scrub_message(message.text),
            message_key: Some(message.key),
            params: scrub_params(message.params.unwrap_or(serde_json::Value::Null)),
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Scrubs every string leaf of a flat parameters object; non-object params
/// pass through untouched (callers only build flat objects here).
fn scrub_params(params: serde_json::Value) -> Option<serde_json::Value> {
    match params {
        serde_json::Value::Null => None,
        serde_json::Value::Object(map) => {
            let scrubbed = map
                .into_iter()
                .map(|(name, value)| {
                    let value = match value {
                        serde_json::Value::String(text) => serde_json::Value::String(adapter::scrub_message(text)),
                        other => other,
                    };
                    (name, value)
                })
                .collect::<serde_json::Map<String, serde_json::Value>>();
            Some(serde_json::Value::Object(scrubbed))
        }
        other => Some(other),
    }
}

pub(crate) fn store_error(error: ProfileStoreError) -> CommandError {
    let code = match error {
        ProfileStoreError::UnreadableFile { .. } => "store-unreadable",
        ProfileStoreError::InvalidFile { .. } => "profile-store-unsupported",
        ProfileStoreError::Recovering { .. } => "application-preparing",
        ProfileStoreError::NotFound { .. } => "codex-profile-not-found",
    };
    if code != "application-preparing" {
        let location = match &error {
            ProfileStoreError::InvalidFile { path, .. } | ProfileStoreError::UnreadableFile { path, .. } =>
                runtime_log::configuration_location(path),
            ProfileStoreError::NotFound { id } => runtime_log::configuration_location(
                &std::path::PathBuf::from(format!("configuration/providers/codex/{id}.json"))),
            ProfileStoreError::Recovering { .. } => None,
        };
        runtime_log::record_configuration_failure(code, location);
    }
    CommandError::new(code, error.to_string())
}

/// Maps one structured validation failure onto its renderer translation
/// coordinates; the `Display` text stays the diagnostic detail.
pub(crate) fn validation_error(code: &'static str, error: asb_core::validate::ValidationError) -> CommandError {
    let (key, params) = error.message_parts();
    CommandError::localized(code, key, error.to_string(), params)
}

/// Maps one store operation failure: store-level state errors keep the
/// stable `store_error` code and file diagnostic; every
/// validation or write failure uses the command's own code. No command may
/// stringify a store error instead of going through here.
pub(crate) fn operation_error(code: &'static str, error: StoreOperationError) -> CommandError {
    match error {
        StoreOperationError::Store(store) => store_error(store),
        StoreOperationError::Invalid(message) => CommandError::new(code, message),
    }
}

impl From<asb_switch::SwitchError> for CommandError {
    fn from(error: asb_switch::SwitchError) -> Self {
        use asb_core::lock::LockStatus;
        use asb_switch::executor::{RecoveryOutcome, SwitchError};
        let command = |code, key, params: serde_json::Value| {
            CommandError::localized(code, key, error.to_string(), params)
        };
        match &error {
            SwitchError::ReadCurrent { message } => command(
                "read-current",
                "errors.switch.readCurrent",
                serde_json::json!({ "detail": message }),
            ),
            SwitchError::PlanRejected { message, line } => command(
                "plan-rejected",
                "errors.switch.planRejected",
                serde_json::json!({ "detail": message, "line": line.map(|l| l as u64) }),
            ),
            SwitchError::BlockedByLock { status } => match status {
                LockStatus::Free => command("blocked-by-lock", "errors.switch.blockedFree", serde_json::json!({})),
                LockStatus::Held(holder) => command(
                    "blocked-by-lock",
                    "errors.switch.blockedHeld",
                    serde_json::json!({
                        "pid": holder.pid.map(|pid| pid as u64),
                        "process": holder.process_name,
                    }),
                ),
                LockStatus::Stale(holder) => command(
                    "blocked-by-lock",
                    "errors.switch.blockedStale",
                    serde_json::json!({
                        "pid": holder.pid.map(|pid| pid as u64),
                        "process": holder.process_name,
                    }),
                ),
                LockStatus::Indeterminate { reason } => command(
                    "blocked-by-lock",
                    "errors.switch.blockedIndeterminate",
                    serde_json::json!({ "detail": reason }),
                ),
            },
            SwitchError::ExternalChange { .. } => {
                command("external-change", "errors.switch.externalChange", serde_json::json!({}))
            }
            SwitchError::PlanChanged => {
                command("preview-stale", "errors.switch.planChanged", serde_json::json!({}))
            }
            SwitchError::CommitFailed { stage, message: _, recovery } => {
                let recovery_key = match recovery {
                    RecoveryOutcome::NotNeeded => "errors.switch.recoveryNotNeeded",
                    RecoveryOutcome::Restored { .. } => "errors.switch.recoveryRestored",
                    RecoveryOutcome::RestoreFailed { .. } => "errors.switch.recoveryRestoreFailed",
                };
                command(
                    "commit-failed",
                    "errors.switch.commitFailed",
                    serde_json::json!({ "stageKey": format!("errors.switch.stage.{stage}"), "recoveryKey": recovery_key }),
                )
            }
            SwitchError::LockReleaseFailed { prior, .. } => command(
                "lock-release-failed",
                "errors.switch.lockReleaseFailed",
                serde_json::json!({ "detail": prior.to_string() }),
            ),
        }
    }
}

pub(crate) fn state(app: &AppHandle) -> Result<LocalState, CommandError> {
    LocalState::from_app(app).map_err(|error| {
        CommandError::localized(
            "app-state-unavailable",
            "errors.appStateUnavailable",
            error.clone(),
            serde_json::json!({ "detail": error }),
        )
    })
}

/// Runs one blocking unit of command work on the dedicated blocking pool.
/// Every command that touches files, an external database, or the network
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
        .map_err(|_| CommandError::keyed("task-interrupted", "errors.taskInterrupted", "后台任务已中断"))?
}

/// Records the result of one explicit application action without allowing a
/// diagnostic-write failure to affect that action's result. Read-only polling
/// commands deliberately do not use this wrapper.
pub(crate) async fn observe<T, F>(action: RuntimeLogAction, operation: F) -> Result<T, CommandError>
where
    F: Future<Output = Result<T, CommandError>>,
{
    let result = operation.await;
    match &result {
        Ok(_) => runtime_log::record_success(action),
        Err(error) => runtime_log::record_failure(action, error.code),
    }
    result
}

pub(crate) fn require_write_confirmation(
    confirm_write: bool,
    operation: &str,
) -> Result<(), CommandError> {
    if confirm_write {
        return Ok(());
    }
    Err(CommandError::localized(
        "write-not-confirmed",
        "errors.writeNotConfirmed",
        format!("{operation}前必须进行显式确认"),
        serde_json::json!({ "operation": operation }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_store_errors_have_stable_codes() {
        assert_eq!(
            store_error(ProfileStoreError::invalid(std::path::Path::new("broken.json"), "invalid JSON")).code,
            "profile-store-unsupported"
        );
        assert_eq!(
            store_error(ProfileStoreError::unreadable(std::path::Path::new("unreadable.json"),
                &std::io::Error::from(std::io::ErrorKind::PermissionDenied))).code,
            "store-unreadable"
        );
    }

    #[test]
    fn recovery_and_missing_profiles_are_not_reported_as_corruption() {
        let pending = store_error(ProfileStoreError::Recovering { path: "configuration-repair.json".into() });
        assert_eq!(pending.code, "application-preparing");
        let missing = store_error(ProfileStoreError::NotFound { id: "missing-profile".into() });
        assert_eq!(missing.code, "codex-profile-not-found");
        assert!(missing.message.contains("missing-profile"));
        let invalid = store_error(ProfileStoreError::invalid(std::path::Path::new("providers/broken.json"), "供应商排序位置无效"));
        assert_eq!(invalid.code, "profile-store-unsupported");
        assert!(invalid.message.contains("broken.json"));
        assert!(invalid.message.contains("供应商排序位置无效"));
    }

    #[test]
    fn operation_errors_keep_the_store_code_and_own_the_command_code() {
        assert_eq!(
            operation_error(
                "codex-profile-create-failed",
                StoreOperationError::Store(ProfileStoreError::invalid(std::path::Path::new("broken.json"), "invalid JSON")),
            )
            .code,
            "profile-store-unsupported"
        );
        let invalid = operation_error(
            "codex-profile-create-failed",
            StoreOperationError::Invalid("该客户端已有官方登录入口".to_string()),
        );
        assert_eq!(invalid.code, "codex-profile-create-failed");
        assert_eq!(invalid.message, "该客户端已有官方登录入口");
        assert!(invalid.message_key.is_none());
    }

    #[test]
    fn writes_require_explicit_confirmation() {
        let error = require_write_confirmation(false, "写入配置").expect_err("must reject");
        assert_eq!(error.code, "write-not-confirmed");
        assert!(error.message.contains("显式确认"));
        assert_eq!(error.message_key.as_deref(), Some("errors.writeNotConfirmed"));
        assert!(require_write_confirmation(true, "写入配置").is_ok());
        assert!(require_write_confirmation(false, "写入客户端配置").is_err());
    }

    #[test]
    fn localized_errors_carry_their_key_and_scrubbed_params() {
        let error = CommandError::localized(
            "read-current",
            "errors.switch.readCurrent",
            "无法读取当前配置",
            serde_json::json!({ "detail": "boom" }),
        );
        assert_eq!(error.message_key.as_deref(), Some("errors.switch.readCurrent"));
        assert_eq!(error.params.as_ref().unwrap()["detail"], "boom");
        // A secret-looking value never survives into params.
        let redacted = CommandError::localized(
            "plan-rejected",
            "errors.switch.planRejected",
            "bad plan",
            serde_json::json!({ "detail": "sk-secret-value-abc123" }),
        );
        let rendered = redacted.params.unwrap().to_string();
        assert!(!rendered.contains("sk-secret-value-abc123"));
    }
}
