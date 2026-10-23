//! Window commands for the integrated (undecorated) title bar, the dev-only
//! inspector toggle, and the native directory picker.
//!
//! The custom webview buttons only emit intents; the native side performs
//! them through Tauri's portable window APIs, so the same buttons work on
//! every supported platform. Controls stay in the webview; window state and
//! material theme stay native.

use super::error::CommandError;
use crate::local_state::{AppSettings, ThemePreference};
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
        .ok_or_else(|| CommandError::keyed("main-window-unavailable", "errors.mainWindowUnavailable", "主窗口不可用"))?;
    apply_interface_scale(app, &window, settings.interface_scale)?;
    window
        .set_always_on_top(settings.always_on_top)
        .map_err(|error| CommandError::new("always-on-top-failed", error.to_string()))?;

    let autostart = app.autolaunch();
    let registered = autostart
        .is_enabled()
        .map_err(|error| CommandError::new("launch-at-login-status-failed", error.to_string()))?;
    if !registered && settings.launch_at_login {
        autostart
            .enable()
            .map_err(|error| CommandError::new("launch-at-login-enable-failed", error.to_string()))?;
    } else if registered && !settings.launch_at_login {
        autostart
            .disable()
            .map_err(|error| CommandError::new("launch-at-login-disable-failed", error.to_string()))?;
    }
    crate::desktop_shortcut::apply(app, &settings.global_shortcut)?;
    // App-level theme also reaches tray windows created after startup.
    app.set_theme(match settings.theme {
        ThemePreference::System => None,
        ThemePreference::Light => Some(tauri::Theme::Light),
        ThemePreference::Dark => Some(tauri::Theme::Dark),
    });
    Ok(())
}

fn apply_interface_scale(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    percent: u16,
) -> Result<(), CommandError> {
    let scale = f64::from(percent) / 100.0;
    let config = app.config().app.windows.iter().find(|window| window.label == "main")
        .ok_or_else(|| {
            CommandError::keyed(
                "main-window-config",
                "errors.misc.mainWindowConfigMissing",
                "缺少主窗口配置",
            )
        })?;
    let minimum = tauri::LogicalSize::new(
        config
            .min_width
            .ok_or_else(|| {
                CommandError::keyed(
                    "main-window-config",
                    "errors.misc.mainWindowMinWidthMissing",
                    "缺少主窗口最小宽度",
                )
            })? * scale,
        config
            .min_height
            .ok_or_else(|| {
                CommandError::keyed(
                    "main-window-config",
                    "errors.misc.mainWindowMinHeightMissing",
                    "缺少主窗口最小高度",
                )
            })? * scale,
    );
    let error = |cause: tauri::Error| CommandError::new("interface-scale-failed", cause.to_string());
    let monitor = window.current_monitor().map_err(error)?;
    if let Some(monitor) = monitor {
        let available = monitor.work_area().size.to_logical::<f64>(monitor.scale_factor());
        if minimum.width > available.width || minimum.height > available.height {
            return Err(CommandError::localized("interface-scale-unavailable",
                "errors.scale.unavailable",
                format!("当前屏幕可用区域不足以使用 {percent}% 缩放，请选择较小比例"),
                serde_json::json!({ "percent": percent })));
        }
    }
    let size = window.inner_size().map_err(error)?.to_logical::<f64>(window.scale_factor().map_err(error)?);
    window.set_min_size(Some(minimum)).map_err(error)?;
    if size.width < minimum.width || size.height < minimum.height {
        window.set_size(tauri::LogicalSize::new(size.width.max(minimum.width), size.height.max(minimum.height)))
            .map_err(error)?;
    }
    window.set_zoom(scale).map_err(error)
}

#[tauri::command]
pub fn window_minimize(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.minimize();
        crate::workspace_visibility::publish(&app);
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
    app.state::<crate::gateway::GatewayController>().shutdown();
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

/// Opens the native directory picker and returns the picked absolute path;
/// `None` means the user canceled. The dialog lives in a Rust command so the
/// browser-dev backend offers the same native picker as the desktop webview,
/// and no local path reaches the renderer unasked.
#[tauri::command]
pub async fn pick_directory(app: AppHandle) -> Result<Option<String>, CommandError> {
    use tauri_plugin_dialog::DialogExt;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |picked| {
        let _ = sender.send(
            picked
                .and_then(|picked| picked.into_path().ok())
                .map(|picked| picked.display().to_string()),
        );
    });
    receiver
        .await
        .map_err(|_| {
            CommandError::keyed(
                "directory-picker-failed",
                "errors.misc.directoryPickerNoResult",
                "目录选择对话框未返回结果",
            )
        })
}

/// Opens the native file picker limited to `extensions` and returns the picked
/// absolute path; `None` means the user canceled. The dialog lives in a Rust
/// command so the browser-dev backend offers the same native picker as the
/// desktop webview, and no local path reaches the renderer unasked. The picker
/// itself never reads the selected file.
#[tauri::command]
pub async fn pick_file(
    app: AppHandle,
    filter_name: String,
    extensions: Vec<String>,
) -> Result<Option<String>, CommandError> {
    use tauri_plugin_dialog::DialogExt;
    let extension_refs: Vec<&str> = extensions.iter().map(String::as_str).collect();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter(&filter_name, &extension_refs)
        .pick_file(move |picked| {
            let _ = sender.send(
                picked
                    .and_then(|picked| picked.into_path().ok())
                    .map(|picked| picked.display().to_string()),
            );
        });
    receiver
        .await
        .map_err(|_| {
            CommandError::keyed(
                "file-picker-failed",
                "errors.misc.filePickerNoResult",
                "文件选择对话框未返回结果",
            )
        })
}
