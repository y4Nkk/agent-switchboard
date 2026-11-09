use super::error::{blocking, observe, state, CommandError};
use crate::{runtime_log::RuntimeLogAction, session_manager::provider_transfer::{self, Error, Operation, Preview}};
use tauri::AppHandle;

#[tauri::command]
pub async fn preview_codex_session_provider_transfer(app: AppHandle) -> Result<Preview, CommandError> {
    let local = state(&app)?;
    blocking(move || provider_transfer::preview(local.root()).map_err(|error| transfer_error(error, false))).await
}

#[tauri::command]
pub async fn read_codex_session_provider_transfer(app: AppHandle) -> Result<Option<Operation>, CommandError> {
    let local = state(&app)?;
    blocking(move || provider_transfer::latest(local.root()).map_err(|error| transfer_error(error, false))).await
}

#[tauri::command]
pub async fn apply_codex_session_provider_transfer(app: AppHandle, id: String, confirm_write: bool) -> Result<Operation, CommandError> {
    confirmed(confirm_write)?;
    let local = state(&app)?;
    observe(RuntimeLogAction::CodexSessionProvidersTransferred, blocking(move ||
        provider_transfer::execute(local.root(), &id).map_err(|error| transfer_error(error, false)))).await
}

#[tauri::command]
pub async fn restore_codex_session_provider_transfer(app: AppHandle, id: String, confirm_write: bool) -> Result<Operation, CommandError> {
    confirmed(confirm_write)?;
    let local = state(&app)?;
    observe(RuntimeLogAction::CodexSessionProvidersRestored, blocking(move ||
        provider_transfer::restore(local.root(), &id).map_err(|error| transfer_error(error, true)))).await
}

fn confirmed(value: bool) -> Result<(), CommandError> {
    if value { return Ok(()); }
    Err(CommandError::keyed("codex-history-not-confirmed", "sessions.transfer.confirmRequired", "Confirm the session provider operation first"))
}

pub(super) fn transfer_error(error: Error, restoring: bool) -> CommandError {
    let code = "codex-session-provider-transfer";
    match &error {
        Error::Busy(Some(pid)) => CommandError::localized(code, "sessions.transfer.closeProcess", error.to_string(), serde_json::json!({"pid":pid})),
        Error::Busy(None) => CommandError::keyed(code, "sessions.transfer.closeCodex", error.to_string()),
        Error::Changed => CommandError::keyed(code, if restoring { "sessions.transfer.restoreConflict" } else { "sessions.transfer.changed" }, error.to_string()),
        Error::Conflict(detail) => CommandError::localized(code, if restoring { "sessions.transfer.restoreConflictDetail" } else { "sessions.transfer.failed" }, error.to_string(), serde_json::json!({"detail":detail})),
        Error::Recovery(detail) => CommandError::localized(code, "sessions.transfer.recoveryFailed", error.to_string(), serde_json::json!({"detail":detail})),
        Error::Failure(detail) => CommandError::localized(code, "sessions.transfer.failed", error.to_string(), serde_json::json!({"detail":detail})),
    }
}
