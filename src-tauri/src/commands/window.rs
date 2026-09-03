//! Window commands for the integrated (undecorated) title bar and the
//! dev-only inspector toggle.
//!
//! The custom webview buttons only emit intents; the native side performs
//! them through Win32 system commands — the same channel the system caption
//! buttons use — so minimize/restore animations, snap-aware maximize, and
//! the close path behave exactly like a decorated window. Visuals stay in
//! the webview.

use super::error::CommandError;
use crate::local_state::AppSettings;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use windows_sys::Win32::Foundation::WPARAM;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    IsZoomed, SendMessageW, SC_CLOSE, SC_MAXIMIZE, SC_MINIMIZE, SC_RESTORE, WM_SYSCOMMAND,
};

/// Applies live desktop preferences before persistence. The caller restores
/// the prior complete setting when this returns an error, so the native window
/// state and Windows login registration never report a value that was not
/// saved. Hardware acceleration is applied before WebView creation on the
/// next app start.
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

fn main_hwnd(app: &AppHandle) -> Option<windows_sys::Win32::Foundation::HWND> {
    app.get_webview_window("main")
        .and_then(|window| window.hwnd().ok())
        .map(|hwnd| hwnd.0)
}

#[tauri::command]
pub fn window_minimize(app: tauri::AppHandle) {
    if let Some(hwnd) = main_hwnd(&app) {
        unsafe {
            SendMessageW(hwnd, WM_SYSCOMMAND, SC_MINIMIZE as WPARAM, 0);
        }
    }
}

#[tauri::command]
pub fn window_toggle_maximize(app: tauri::AppHandle) {
    if let Some(hwnd) = main_hwnd(&app) {
        let command = if unsafe { IsZoomed(hwnd) } != 0 {
            SC_RESTORE
        } else {
            SC_MAXIMIZE
        };
        unsafe {
            SendMessageW(hwnd, WM_SYSCOMMAND, command as WPARAM, 0);
        }
    }
}

#[tauri::command]
pub fn window_is_maximized(app: tauri::AppHandle) -> bool {
    main_hwnd(&app)
        .map(|hwnd| unsafe { IsZoomed(hwnd) } != 0)
        .unwrap_or(false)
}

#[tauri::command]
pub fn window_close(app: tauri::AppHandle) {
    if let Some(hwnd) = main_hwnd(&app) {
        unsafe {
            SendMessageW(hwnd, WM_SYSCOMMAND, SC_CLOSE as WPARAM, 0);
        }
    }
}

/// Restarts the complete desktop process so WebView2 creation-time options,
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
