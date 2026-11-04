//! Read-only configuration status: per-client file health, route facts,
//! connection identity, write history, and lock observation.

pub(crate) mod codex;
mod overview;
mod report;
#[cfg(test)]
mod tests;


use super::error::{blocking, observe, state, CommandError};
use crate::runtime_log::RuntimeLogAction;
use asb_core::contracts::{AppKind, ConfigWriteRecord, RouteState, SettingsValues};
use asb_core::LockStatus;
use asb_switch::io::FsIo;
use asb_switch::lockfile::{self, RecoveryEntry};
use overview::{runtime_overview_for, runtime_transport, RuntimeOverview};
pub(crate) use report::config_status_report;
use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFileStatus {
    pub app: AppKind,
    pub path: String,
    pub exists: bool,
    pub syntax_ok: bool,
    pub route: Option<RouteState>,
    /// Internal gateway fact; the public route URL is already redacted.
    #[serde(skip)]
    pub(crate) gateway_port: Option<u16>,
    pub read_error: Option<String>,
    /// Current durable transaction state; diagnostics remain readable while writes are blocked.
    pub recovery_issue: Option<String>,
    /// Read-only client-owned values extracted from the real configuration file.
    pub client_settings: Option<SettingsValues>,
    /// A valid client file can still contain an invalid value for an owned setting.
    pub client_settings_error: Option<String>,
    pub codex_permission_profile: Option<asb_core::adapter::codex::PermissionProfileStatus>,
    /// Connection identity only; model, preferences and write history do not prove file health.
    pub active_profile_id: Option<String>,
    pub last_switch: Option<ConfigWriteRecord>,
}

#[tauri::command]
pub async fn config_status(app: AppHandle) -> Result<Vec<ConfigFileStatus>, CommandError> {
    let state = state(&app)?;
    let gateway = app
        .state::<crate::gateway::GatewayController>()
        .inner()
        .clone();
    blocking(move || config_status_report(&state, &gateway)).await
}

#[tauri::command]
pub async fn runtime_overview(app: AppHandle) -> Result<RuntimeOverview, CommandError> {
    let app_data_dir = crate::app_paths::data_directory(&app.config().identifier)
        .map_err(|_| CommandError::keyed("runtime-path-unavailable", "errors.cfg.runtimeDataDirUnavailable", "无法定位应用数据目录"))?;
    Ok(runtime_overview_for(
        app.package_info().version.to_string(),
        &app_data_dir,
        runtime_transport(),
    ))
}

/// Opens a backend-resolved path in the system file manager: directories open
/// themselves, files are revealed and selected in their folder, and a missing
/// file falls back to its folder. Mirrors `open_runtime_log_dir`: the renderer
/// names what to open, never the path itself.
fn open_in_file_manager(app: &AppHandle, path: &Path) -> Result<(), CommandError> {
    if path.is_dir() {
        return app
            .opener()
            .open_path(path.to_string_lossy().into_owned(), None::<&str>)
            .map_err(|_| CommandError::keyed("path-open-failed", "errors.cfg.openContainingFolderFailed", "无法打开所在文件夹"));
    }
    if path.is_file() {
        return app
            .opener()
            .reveal_item_in_dir(path)
            .map_err(|_| CommandError::keyed("path-open-failed", "errors.cfg.revealFileFailed", "无法在文件管理器中定位该文件"));
    }
    let folder = path
        .parent()
        .filter(|folder| folder.is_dir())
        .ok_or_else(|| CommandError::keyed("path-open-failed", "errors.cfg.fileAndFolderMissing", "文件与所在文件夹均不存在"))?;
    app.opener()
        .open_path(folder.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|_| CommandError::keyed("path-open-failed", "errors.cfg.openContainingFolderFailed", "无法打开所在文件夹"))
}

/// Reveals the client's real configuration file in its folder, reusing the
/// `config_status` path resolution. No path crosses the IPC boundary.
#[tauri::command]
pub async fn open_config_file_location(
    app: AppHandle,
    target: AppKind,
) -> Result<(), CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let path = state
            .target(target)
            .map_err(|error| CommandError::new("config-path-unavailable", error))?;
        open_in_file_manager(&app, &path)
    })
    .await
}

/// Opens the application data directory reported by `runtime_overview`.
#[tauri::command]
pub async fn open_app_data_dir(app: AppHandle) -> Result<(), CommandError> {
    let path = crate::app_paths::data_directory(&app.config().identifier)
        .map_err(|_| CommandError::keyed("runtime-path-unavailable", "errors.cfg.runtimeDataDirUnavailable", "无法定位应用数据目录"))?;
    open_in_file_manager(&app, &path)
}

#[tauri::command]
pub async fn lock_status(app: AppHandle, target: AppKind) -> Result<LockStatus, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let target = state
            .target(target)
            .map_err(|error| CommandError::new("config-path-unavailable", error))?;
        Ok(lockfile::probe_lock(&FsIo, &target))
    })
    .await
}

#[tauri::command]
pub async fn recover_stale_lock(
    app: AppHandle,
    target: AppKind,
) -> Result<RecoveryEntry, CommandError> {
    observe(RuntimeLogAction::StaleLockRecovered, async move {
        let state = state(&app)?;
        blocking(move || {
            let target = state
                .target(target)
                .map_err(|error| CommandError::new("config-path-unavailable", error))?;
            lockfile::recover_stale(&FsIo, &target)
                .map_err(|_| CommandError::keyed("lock-not-stale", "errors.cfg.lockNotStale", "当前锁不是可恢复的遗留状态"))
        })
        .await
    })
    .await
}
