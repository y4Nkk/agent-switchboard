use crate::commands::error::CommandError;
use tauri::{AppHandle, Emitter, Manager};

pub fn visible(app: &AppHandle) -> Result<bool, String> {
    #[cfg(debug_assertions)]
    if crate::web_development_enabled(std::env::var_os("ASB_WEB_DEVELOPMENT").as_deref()) { return Ok(true); }
    let window = app.get_webview_window("main").ok_or("主窗口尚未准备好")?;
    Ok(window.is_visible().map_err(|error| error.to_string())?
        && !window.is_minimized().map_err(|error| error.to_string())?)
}

#[tauri::command]
pub fn workspace_visible(app: AppHandle) -> Result<bool, CommandError> {
    visible(&app).map_err(|detail| CommandError::localized("workspace-visibility-failed",
        "resources.error.visibility", "无法读取窗口可见状态", serde_json::json!({ "detail": detail })))
}

pub fn publish(app: &AppHandle) {
    match visible(app) {
        Ok(visible) => {
            if let Err(error) = app.emit("workspace-visibility-changed", visible) {
                log::warn!("无法发布工作区可见状态：{error}");
            }
        }
        Err(error) => log::debug!("工作区可见状态尚不可用：{error}"),
    }
}
