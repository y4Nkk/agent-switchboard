use super::error::{blocking, state, CommandError};
use crate::{runtime_log::{self, RuntimeLogAction}, session_manager::range::{Action, Operations, Preview, Progress, Request}};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn preview_session_range(app: AppHandle, request: Request, action: Action) -> Result<Preview, CommandError> {
    let local = state(&app)?;
    let operations = app.state::<Arc<Operations>>().inner().clone();
    blocking(move || operations.preview(local.root(), request, action).map_err(error)).await
}

#[tauri::command]
pub async fn discard_session_range(app: AppHandle, id: String) -> Result<(), CommandError> {
    app.state::<Arc<Operations>>().discard(&id).map_err(error)
}

#[tauri::command]
pub async fn read_session_range_progress(app: AppHandle, id: String) -> Result<Option<Progress>, CommandError> {
    let progress = app.state::<Arc<Operations>>().progress().map_err(error)?;
    Ok((progress.id.as_deref() == Some(id.as_str())).then_some(progress))
}

#[tauri::command]
pub async fn execute_session_range(app: AppHandle, id: String, confirmed: bool) -> Result<Progress, CommandError> {
    let local = state(&app)?;
    let operations = app.state::<Arc<Operations>>().inner().clone();
    blocking(move || {
        let result = operations.execute(local.root(), &id, confirmed);
        match &result {
            Ok(progress) if progress.warning.is_none() && progress.outcomes.iter().all(|outcome| outcome.completed && outcome.error.is_none()) =>
                runtime_log::record_success(RuntimeLogAction::SessionRangeProcessed),
            Ok(_) => runtime_log::record_failure(RuntimeLogAction::SessionRangeProcessed, "session-range-partial-failure"),
            Err(_) => runtime_log::record_failure(RuntimeLogAction::SessionRangeProcessed, "session-range-failed"),
        }
        result.map_err(error)
    }).await
}

fn error(message: String) -> CommandError {
    CommandError::localized("session-range-failed", "sessions.range.failed", message.clone(), serde_json::json!({"detail":message}))
}
