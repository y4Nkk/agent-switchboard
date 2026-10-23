use super::error::{state, CommandError};
use crate::session_manager::{search_tasks, SessionSearchRequest};
use tauri::AppHandle;

fn search_error(message: String) -> CommandError {
    CommandError::localized("session-search-failed", "errors.sessions.searchFailed", message.clone(),
        serde_json::json!({ "detail": message }))
}

#[tauri::command]
pub async fn start_session_search(app: AppHandle, request: SessionSearchRequest) -> Result<String, CommandError> {
    let local = state(&app)?;
    search_tasks::start(local.root().into(), request).map_err(search_error)
}

#[tauri::command]
pub async fn read_session_search(app: AppHandle, id: String) -> Result<search_tasks::SearchTaskState, CommandError> {
    let local = state(&app)?;
    search_tasks::read(local.root(), &id).map_err(search_error)
}

#[tauri::command]
pub async fn cancel_session_search(app: AppHandle, id: String) -> Result<(), CommandError> {
    let local = state(&app)?;
    search_tasks::cancel(local.root(), &id).map_err(search_error)
}
