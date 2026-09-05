//! One custom tray panel, backed by the existing configuration executor.

#[cfg(target_os = "linux")]
mod linux;
pub(crate) mod popup;
mod position;
mod snapshot;

use crate::commands::error::CommandError;
use crate::local_state::{AppSettings, CloseBehavior, LocalState};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

static TRAY_READY: AtomicBool = AtomicBool::new(false);
static EXPLICIT_EXIT: AtomicBool = AtomicBool::new(false);
static SWITCHING: AtomicBool = AtomicBool::new(false);

pub fn is_ready() -> bool {
    TRAY_READY.load(Ordering::Relaxed)
}

pub(super) fn set_ready(ready: bool) {
    TRAY_READY.store(ready, Ordering::Relaxed);
}

pub(crate) fn keep_alive(tray_ready: bool, stored: Result<AppSettings, String>) -> bool {
    tray_ready
        && stored
            .map(|settings| settings.close_behavior == CloseBehavior::HideToTray)
            .unwrap_or(true)
}

pub fn request_explicit_exit() {
    EXPLICIT_EXIT.store(true, Ordering::Relaxed);
}

pub fn take_explicit_exit() -> bool {
    EXPLICIT_EXIT.swap(false, Ordering::Relaxed)
}

pub(crate) fn should_absorb(app: &AppHandle) -> bool {
    keep_alive(
        is_ready(),
        LocalState::from_app(app).and_then(|state| state.get_app_settings()),
    )
}

pub fn refresh(app: &AppHandle) {
    if let Err(error) = app.emit("tray-changed", ()) {
        log::warn!("无法刷新托盘状态: {error}");
    }
}

pub fn setup(app: &AppHandle) -> Result<(), String> {
    popup::setup(app)?;
    #[cfg(target_os = "linux")]
    return linux::setup(app);
    #[cfg(not(target_os = "linux"))]
    {
        use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
        let icon = app
            .default_window_icon()
            .cloned()
            .ok_or("默认应用图标不可用")?;
        TrayIconBuilder::with_id("agent-switchboard-tray")
            .icon(icon)
            .tooltip("Agent Switchboard")
            .show_menu_on_left_click(false)
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left | MouseButton::Right,
                    button_state: MouseButtonState::Up,
                    position,
                    rect,
                    ..
                } = event
                {
                    if let Err(error) = popup::toggle(tray.app_handle(), Some(rect), Some(position))
                    {
                        recover_main(tray.app_handle(), &error);
                    }
                }
            })
            .build(app)
            .map_err(|error| format!("无法创建托盘图标: {error}"))?;
        set_ready(true);
        Ok(())
    }
}

#[tauri::command]
pub async fn tray_snapshot(app: AppHandle) -> Result<snapshot::TraySnapshot, CommandError> {
    let state = LocalState::from_app(&app)
        .map_err(|error| CommandError::new("tray-state-unavailable", error))?;
    crate::commands::error::blocking(move || {
        Ok(snapshot::read(&state, SWITCHING.load(Ordering::Acquire)))
    })
    .await
}

#[tauri::command]
pub fn tray_ready(app: AppHandle) -> Result<(), String> {
    popup::ready(&app)
}

#[tauri::command]
pub fn tray_resize(app: AppHandle, height: f64) -> Result<(), String> {
    popup::resize(&app, height)
}

#[tauri::command]
pub fn tray_hide(app: AppHandle) -> Result<(), String> {
    popup::hide(&app, false)
}

#[tauri::command]
pub fn tray_open_main(app: AppHandle, providers: bool) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or("主窗口不可用")?;
    window.show().map_err(|error| error.to_string())?;
    if window.is_minimized().map_err(|error| error.to_string())? {
        window.unminimize().map_err(|error| error.to_string())?;
    }
    if providers {
        window
            .emit("tray-navigate", ())
            .map_err(|error| error.to_string())?;
    }
    window.set_focus().map_err(|error| error.to_string())?;
    popup::hide(&app, false)
}

pub(crate) fn recover_main(app: &AppHandle, error: &str) {
    log::warn!("托盘浮层不可用: {error}");
    if let Err(open_error) = tray_open_main(app.clone(), false) {
        log::warn!("恢复主窗口失败: {open_error}");
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("tray-error", asb_core::adapter::scrub_message(error));
    }
}

struct SwitchingGuard(AppHandle);

impl Drop for SwitchingGuard {
    fn drop(&mut self) {
        SWITCHING.store(false, Ordering::Release);
        refresh(&self.0);
    }
}

#[tauri::command]
pub async fn tray_switch(app: AppHandle, profile_id: String) -> Result<(), CommandError> {
    if SWITCHING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(CommandError::new(
            "tray-switch-busy",
            "供应商正在切换，请稍候",
        ));
    }
    let _guard = SwitchingGuard(app.clone());
    refresh(&app);
    let preview =
        crate::commands::switching::preview_switch(app.clone(), profile_id.clone()).await?;
    crate::commands::switching::execute_switch(
        app,
        profile_id,
        preview.content_hash,
        preview.rendered_hash,
        true,
    )
    .await?;
    Ok(())
}

#[tauri::command]
pub fn tray_quit(app: AppHandle) {
    request_explicit_exit();
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_remains_recoverable_only_when_a_tray_exists() {
        assert!(keep_alive(true, Err("设置不可读".into())));
        assert!(!keep_alive(false, Ok(AppSettings::default())));
        assert!(!keep_alive(
            true,
            Ok(AppSettings {
                close_behavior: CloseBehavior::Exit,
                ..AppSettings::default()
            })
        ));
    }

    #[test]
    fn explicit_exit_marker_is_consumed_once() {
        EXPLICIT_EXIT.store(false, Ordering::Relaxed);
        request_explicit_exit();
        assert!(take_explicit_exit());
        assert!(!take_explicit_exit());
    }
}
