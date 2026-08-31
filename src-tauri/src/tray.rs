//! Native Windows system-tray integration.
//!
//! This module owns tray IDs, menu construction, and window restoration. It
//! only reports current routing; it never writes client configuration or
//! bypasses the preview/confirmation transaction.

use crate::commands::config_status_report;
use crate::local_state::{AppSettings, CloseBehavior, LocalState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Wry,
};

const TRAY_ID: &str = "agent-switchboard-tray";
const MENU_SHOW: &str = "tray-show-window";
const MENU_QUIT: &str = "tray-quit";
const MENU_CODEX_STATUS: &str = "tray-codex-status";
const MENU_CLAUDE_STATUS: &str = "tray-claude-status";

static STATUS_ITEMS: OnceLock<(MenuItem<Wry>, MenuItem<Wry>)> = OnceLock::new();
static TRAY_READY: AtomicBool = AtomicBool::new(false);
static EXPLICIT_EXIT: AtomicBool = AtomicBool::new(false);

/// Whether the tray icon actually exists right now. Hiding a window is only
/// safe while the tray can bring it back; without it the app would be lost.
pub fn is_ready() -> bool {
    TRAY_READY.load(Ordering::Relaxed)
}

/// The single keep-alive decision shared by window close and process exit:
/// absorb the request only while the tray exists. An unreadable settings file
/// fails toward hiding (the tray can still restore the window), never toward
/// exit — an error must never take the whole app down.
pub(crate) fn keep_alive(tray_ready: bool, stored: Result<AppSettings, String>) -> bool {
    if !tray_ready {
        return false;
    }
    stored
        .map(|settings| settings.close_behavior == CloseBehavior::HideToTray)
        .unwrap_or(true)
}

/// Marks the menu's "退出" as an intentional process exit. Tauri emits an
/// ExitRequested event for app.exit, so the lifecycle guard consumes this
/// marker and lets exactly that request through.
pub fn request_explicit_exit() {
    EXPLICIT_EXIT.store(true, Ordering::Relaxed);
}

/// Consumes the intentional-exit marker once.
pub fn take_explicit_exit() -> bool {
    EXPLICIT_EXIT.swap(false, Ordering::Relaxed)
}

/// Reads settings and applies the shared decision. Used by both event hooks.
pub(crate) fn should_absorb(app: &AppHandle<Wry>) -> bool {
    let stored = LocalState::from_app(app).and_then(|state| state.get_app_settings());
    keep_alive(is_ready(), stored)
}

fn route_label(status: &crate::commands::ConfigFileStatus) -> String {
    let client = match status.app {
        asb_core::AppKind::Codex => "Codex",
        asb_core::AppKind::Claude => "Claude",
    };
    let route = status
        .route
        .as_ref()
        .and_then(|route| route.provider_name.as_deref())
        .unwrap_or_else(|| {
            if status.read_error.is_some() {
                "配置不可读"
            } else if !status.exists {
                "未加载"
            } else {
                "官方登录"
            }
        });
    format!("{client} · {route}")
}

fn show_main_window(app: &AppHandle<Wry>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn status_texts(app: &AppHandle<Wry>) -> (String, String) {
    let statuses = crate::local_state::LocalState::from_app(app)
        .ok()
        .and_then(|state| config_status_report(&state).ok());
    match statuses {
        Some(statuses) => {
            let codex = statuses
                .iter()
                .find(|status| status.app == asb_core::AppKind::Codex)
                .map(route_label)
                .unwrap_or_else(|| "Codex · 未加载".to_string());
            let claude = statuses
                .iter()
                .find(|status| status.app == asb_core::AppKind::Claude)
                .map(route_label)
                .unwrap_or_else(|| "Claude · 未加载".to_string());
            (claude, codex)
        }
        None => ("Claude · 未加载".to_string(), "Codex · 未加载".to_string()),
    }
}

/// Rebuilds the status labels from the current live configuration. The two
/// items retain stable handles created by `setup`.
pub fn refresh(app: &AppHandle<Wry>) {
    let Some((claude_item, codex_item)) = STATUS_ITEMS.get() else {
        return;
    };
    let (claude, codex) = status_texts(app);
    let _ = claude_item.set_text(claude);
    let _ = codex_item.set_text(codex);
}

/// Creates the always-visible tray icon. Left click restores the main window;
/// right click opens the native menu: one window affordance, two disabled
/// route facts, and one explicit process-exit command. Menu state must come
/// from the existing real configuration state; it is never a switching entry.
pub fn setup(app: &AppHandle<Wry>) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, MENU_SHOW, "打开主界面", true, None::<&str>)?;
    let claude = MenuItem::with_id(
        app,
        MENU_CLAUDE_STATUS,
        "Claude · 未加载",
        false,
        None::<&str>,
    )?;
    let codex = MenuItem::with_id(
        app,
        MENU_CODEX_STATUS,
        "Codex · 未加载",
        false,
        None::<&str>,
    )?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &separator_one,
            &claude,
            &codex,
            &separator_two,
            &quit,
        ],
    )?;
    let _ = STATUS_ITEMS.set((claude.clone(), codex.clone()));
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("默认应用图标".to_string()))?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("Agent Switchboard")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_SHOW => show_main_window(app),
            MENU_QUIT => {
                request_explicit_exit();
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    TRAY_READY.store(true, Ordering::Relaxed);
    refresh(app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_state::{MotionPreference, ThemePreference};

    #[test]
    fn settings_error_keeps_the_app_recoverable_when_tray_exists() {
        assert!(keep_alive(true, Err("应用设置不可读".to_string())));
    }

    #[test]
    fn exit_setting_allows_close_even_when_tray_exists() {
        assert!(!keep_alive(
            true,
            Ok(AppSettings {
                close_behavior: CloseBehavior::Exit,
                theme: ThemePreference::System,
                motion: MotionPreference::System,
                always_on_top: false,
                hardware_acceleration: true,
            })
        ));
    }

    #[test]
    fn hide_requires_a_real_tray_icon() {
        assert!(!keep_alive(false, Ok(AppSettings::default())));
    }

    #[test]
    fn explicit_exit_marker_is_consumed_once() {
        EXPLICIT_EXIT.store(false, Ordering::Relaxed);
        request_explicit_exit();
        assert!(take_explicit_exit());
        assert!(!take_explicit_exit());
    }
}
