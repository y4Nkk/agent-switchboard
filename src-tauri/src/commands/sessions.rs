use super::error::{blocking, observe, state, CommandError};
use crate::runtime_log::RuntimeLogAction;
use crate::session_manager;
use asb_core::contracts::AppKind;
use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn claude_session_sources(app: AppHandle) -> Result<Vec<session_manager::history::HistoryRoot>, CommandError> {
    let local = state(&app)?;
    blocking(move || session_manager::history::roots(local.root()).map_err(history_error)).await
}

#[tauri::command]
pub async fn pick_claude_session_source(app: AppHandle) -> Result<Option<Vec<session_manager::history::HistoryRoot>>, CommandError> {
    let Some(path) = super::window::pick_directory(app.clone()).await? else { return Ok(None); };
    let local = state(&app)?;
    blocking(move || session_manager::history::add_root(local.root(), Path::new(&path)).map(Some).map_err(history_error)).await
}

#[tauri::command]
pub async fn forget_claude_session_source(app: AppHandle, path: String) -> Result<Vec<session_manager::history::HistoryRoot>, CommandError> {
    let local = state(&app)?;
    blocking(move || session_manager::history::forget_root(local.root(), &path).map_err(history_error)).await
}

fn history_error(message: String) -> CommandError {
    CommandError::localized("claude-session-sources-failed", "sessions.sources.failed", message.clone(), serde_json::json!({"detail":message}))
}
#[tauri::command]
pub async fn convert_session_sources(
    app_handle: AppHandle,
    ids: Vec<String>,
) -> Result<session_manager::SessionConversionOutcome, CommandError> {
    let local = state(&app_handle)?;
    blocking(move || session_manager::convert_session_sources(local.root(), &ids)
        .map_err(|message| CommandError::localized("session-conversion-failed", "errors.sessions.conversionFailed",
            message.clone(), serde_json::json!({ "detail": message })))).await
}

/// Resolves one exact session identity independently of search pagination.
#[tauri::command]
pub async fn get_session_metadata(
    app_handle: AppHandle,
    app: AppKind,
    session_id: String,
) -> Result<session_manager::SessionMeta, CommandError> {
    let local = state(&app_handle)?;
    blocking(move || {
        session_manager::load_metadata(local.root(), app, &session_id)
            .map_err(|message| CommandError::localized(
                "session-unavailable",
                "errors.sessions.unavailable",
                message.clone(),
                serde_json::json!({ "detail": message }),
            ))
    })
    .await
}

/// The renderer selects a session, never a filesystem source or export path.
#[tauri::command]
pub async fn export_session_markdown(
    app_handle: AppHandle,
    app: AppKind,
    session_id: String,
) -> Result<Option<String>, CommandError> {
    let local = state(&app_handle)?;
    blocking(move || {
        let filename_id: String = session_id
            .chars()
            .filter(|character| character.is_ascii_alphanumeric() || matches!(*character, '-' | '_'))
            .take(64)
            .collect();
        let Some(destination) = app_handle
            .dialog()
            .file()
            .add_filter("Markdown", &["md"])
            .set_file_name(format!("session-{filename_id}.md"))
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let path = destination.into_path().map_err(|error| {
            CommandError::localized(
                "session-export-failed",
                "errors.sessions.exportPathInvalid",
                format!("无法使用所选导出路径：{error}"),
                serde_json::json!({ "detail": error.to_string() }),
            )
        })?;
        session_manager::export_markdown(local.root(), app, &session_id, &path).map_err(|error| {
            CommandError::localized(
                "session-export-failed",
                "errors.sessions.exportWriteFailed",
                format!("无法写入会话 Markdown：{error}"),
                serde_json::json!({ "detail": error.to_string() }),
            )
        })?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
}

#[tauri::command]
pub async fn search_session_bookmarks(
    app_handle: AppHandle,
    request: session_manager::BookmarkSearchRequest,
) -> Result<session_manager::BookmarkPage, CommandError> {
    let local = state(&app_handle)?;
    blocking(move || {
        session_manager::search_bookmarks(local.root(), request)
            .map_err(|message| CommandError::localized(
                "session-bookmarks-unavailable",
                "errors.sessions.bookmarksUnavailable",
                message.clone(),
                serde_json::json!({ "detail": message }),
            ))
    })
    .await
}

#[tauri::command]
pub async fn save_session_bookmark(
    app_handle: AppHandle,
    app: AppKind,
    session_id: String,
    message_id: String,
) -> Result<session_manager::BookmarkSummary, CommandError> {
    let local = state(&app_handle)?;
    blocking(move || {
        session_manager::save_bookmark(local.root(), app, &session_id, &message_id)
            .map_err(|message| CommandError::localized(
                "session-bookmark-save-failed",
                "errors.sessions.bookmarkSaveFailed",
                message.clone(),
                serde_json::json!({ "detail": message }),
            ))
    })
    .await
}

#[tauri::command]
pub async fn delete_session_bookmark(
    app_handle: AppHandle,
    id: String,
) -> Result<crate::storage::ReclaimOutcome, CommandError> {
    let local = state(&app_handle)?;
    blocking(move || {
        session_manager::delete_bookmark(local.root(), &id)
            .map_err(|message| CommandError::localized(
                "session-bookmark-delete-failed",
                "errors.sessions.bookmarkDeleteFailed",
                message.clone(),
                serde_json::json!({ "detail": message }),
            ))
    })
    .await
}

/// Loads one transcript after resolving its source from the approved session
/// roots. No session file is created, modified, or deleted by this command.
#[tauri::command]
pub async fn get_session_message_page(
    app_handle: AppHandle,
    request: session_manager::MessagePageRequest,
) -> Result<session_manager::MessagePage, CommandError> {
    let local = state(&app_handle)?;
    blocking(move || {
        session_manager::load_page(local.root(), request)
            .map_err(|message| CommandError::localized(
                "session-unavailable",
                "errors.sessions.unavailable",
                message.clone(),
                serde_json::json!({ "detail": message }),
            ))
    })
    .await
}

#[tauri::command]
pub async fn get_session_message_content(
    app_handle: AppHandle,
    request: session_manager::MessageContentRequest,
) -> Result<session_manager::ContentPage, CommandError> {
    let local = state(&app_handle)?;
    blocking(move || session_manager::load_content(local.root(), request).map_err(session_content_error)).await
}

#[tauri::command]
pub async fn get_session_bookmark_content(
    app_handle: AppHandle,
    id: String,
    offset: usize,
) -> Result<session_manager::ContentPage, CommandError> {
    let local = state(&app_handle)?;
    blocking(move || session_manager::load_bookmark_content(local.root(), &id, offset)
        .map_err(session_content_error)).await
}

fn session_content_error(message: String) -> CommandError {
    CommandError::localized("session-unavailable", "errors.sessions.unavailable", message.clone(),
        serde_json::json!({ "detail": message }))
}

#[tauri::command]
pub async fn get_session_copy_text(app_handle: AppHandle, source: session_manager::SessionCopySource) -> Result<String, CommandError> {
    let local = state(&app_handle)?;
    blocking(move || session_manager::load_copy_text(local.root(), source).map_err(session_content_error)).await
}

/// Starts the selected session in the platform terminal. The session source
/// is resolved by the backend; the renderer never submits a path or
/// executable command.
#[tauri::command]
pub async fn resume_session(
    app_handle: AppHandle,
    app: AppKind,
    session_id: String,
) -> Result<crate::session_manager::SessionResume, CommandError> {
    let local = state(&app_handle)?;
    observe(RuntimeLogAction::SessionResumed, async move {
        blocking(move || {
            crate::session_manager::resume_session(local.root(), app, &session_id)
                .map_err(|message| CommandError::localized(
                    "session-resume-failed",
                    "errors.sessions.resumeFailed",
                    message.clone(),
                    serde_json::json!({ "detail": message }),
                ))
        })
        .await
    })
    .await
}

/// Permanently removes the one local session record the backend resolves from
/// the approved roots. Irreversible by contract; the confirmation sheet in
/// the renderer is the only gate.
#[tauri::command]
pub async fn delete_session(app_handle: AppHandle, app: AppKind, session_id: String) -> Result<(), CommandError> {
    let local = state(&app_handle)?;
    observe(RuntimeLogAction::SessionDeleted, async move {
        blocking(move || {
            crate::session_manager::delete_session(local.root(), app, &session_id)
                .map_err(|message| CommandError::localized(
                    "session-delete-failed",
                    "errors.sessions.deleteFailed",
                    message.clone(),
                    serde_json::json!({ "detail": message }),
                ))
        })
        .await
    })
    .await
}

/// Batch counterpart of `delete_session`: one scan resolves every requested
/// record and each item reports its own outcome, so an unresolved or
/// undeletable id never blocks the rest. Only an unreadable root set fails
/// the whole call.
#[tauri::command]
pub async fn delete_sessions(
    app_handle: AppHandle,
    requests: Vec<crate::session_manager::SessionDeleteRequest>,
) -> Result<Vec<crate::session_manager::SessionDeleteOutcome>, CommandError> {
    let local = state(&app_handle)?;
    observe(RuntimeLogAction::SessionDeleted, async move {
        blocking(move || {
            crate::session_manager::delete_sessions(local.root(), &requests)
                .map_err(|message| CommandError::localized(
                    "session-delete-failed",
                    "errors.sessions.deleteFailed",
                    message.clone(),
                    serde_json::json!({ "detail": message }),
                ))
        })
        .await
    })
    .await
}

#[tauri::command]
pub async fn update_session_organization(
    app_handle: AppHandle,
    requests: Vec<session_manager::SessionDeleteRequest>,
    change: session_manager::SessionOrganizationChange,
) -> Result<Vec<session_manager::SessionMeta>, CommandError> {
    let local = state(&app_handle)?;
    blocking(move || {
        session_manager::update_session_organization(local.root(), &requests, &change)
            .map_err(|message| CommandError::localized(
                "session-organization-failed",
                "errors.sessions.organizationFailed",
                message.clone(),
                serde_json::json!({ "detail": message }),
            ))
    }).await
}
