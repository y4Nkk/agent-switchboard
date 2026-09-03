//! Window commands for the integrated (undecorated) title bar and the
//! dev-only inspector toggle.
//!
//! The custom webview buttons only emit intents; the native side performs
//! them through Tauri's portable window APIs, so the same buttons work on
//! every supported platform. Visuals stay in the webview.

use super::error::CommandError;
use crate::local_state::AppSettings;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

/// Applies live desktop preferences before persistence. The caller restores
/// the prior complete setting when this returns an error, so the native window
/// state and login registration never report a value that was not saved.
/// Hardware acceleration is applied before WebView creation on the next app
/// start.
pub(crate) fn apply_desktop_settings(
    app: &AppHandle,
    settings: &AppSettings,
) -> Result<(), CommandError> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| CommandError::new("main-window-unavailable", "主窗口不可用"))?;
    window
        .set_always_on_top(settings.always_on_top)
        .map_err(|error| CommandError::new("always-on-top-failed", error.to_string()))?;

    let autostart = app.autolaunch();
    let registered = autostart
        .is_enabled()
        .map_err(|error| CommandError::new("launch-at-login-status-failed", error.to_string()))?;
    if registered == settings.launch_at_login {
        return Ok(());
    }
    if settings.launch_at_login {
        autostart
            .enable()
            .map_err(|error| CommandError::new("launch-at-login-enable-failed", error.to_string()))
    } else {
        autostart
            .disable()
            .map_err(|error| CommandError::new("launch-at-login-disable-failed", error.to_string()))
    }
}

#[tauri::command]
pub fn window_minimize(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.minimize();
    }
}

#[tauri::command]
pub fn window_toggle_maximize(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_maximized().unwrap_or(false) {
            let _ = window.unmaximize();
        } else {
            let _ = window.maximize();
        }
    }
}

#[tauri::command]
pub fn window_is_maximized(app: tauri::AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|window| window.is_maximized().ok())
        .unwrap_or(false)
}

#[tauri::command]
pub fn window_close(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        // Routes through CloseRequested, so the close-to-tray absorption in
        // the window-event handler keeps working.
        let _ = window.close();
    }
}

/// Restarts the complete desktop process so webview creation-time options,
/// including hardware acceleration, are recreated from the persisted setting.
#[tauri::command]
pub fn restart_application(app: tauri::AppHandle) {
    app.restart();
}

/// Dev-build debug affordance: toggles the WebView inspector (F12). The
/// inspector methods only exist without the `devtools` cargo feature in
/// debug builds, and release builds disable devtools entirely.
#[cfg(debug_assertions)]
#[tauri::command]
pub fn toggle_devtools(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_devtools_open() {
            window.close_devtools();
        } else {
            window.open_devtools();
        }
    }
}
