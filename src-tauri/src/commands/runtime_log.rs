//! Typed IPC for the application-owned runtime event log.

use super::error::{blocking, CommandError};
use crate::runtime_log::{self, RuntimeLogEntry};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub async fn list_runtime_logs(app: AppHandle) -> Result<Vec<RuntimeLogEntry>, CommandError> {
    blocking(move || {
        runtime_log::list(&app).map_err(|error| CommandError::new("runtime-log-unavailable", error))
    })
    .await
}

/// Opens only the app-owned runtime-log directory in the system file manager.
/// No path is accepted from or disclosed to the renderer.
#[tauri::command]
pub async fn open_runtime_log_dir(app: AppHandle) -> Result<(), CommandError> {
    blocking(move || {
        let directory = runtime_log::log_directory(&app)
            .map_err(|error| CommandError::new("runtime-log-directory-unavailable", error))?;
        std::fs::create_dir_all(&directory).map_err(|_| {
            CommandError::new(
                "runtime-log-directory-create-failed",
                "无法创建应用日志目录",
            )
        })?;
        app.opener()
            .open_path(directory.to_string_lossy().into_owned(), None::<&str>)
            .map_err(|_| {
                CommandError::new(
                    "runtime-log-directory-open-failed",
                    "无法打开应用日志文件夹",
                )
            })
    })
    .await
}
