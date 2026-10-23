use crate::commands::error::CommandError;
use std::sync::{atomic::{AtomicBool, AtomicU32, Ordering}, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcut, Modifiers, Shortcut, ShortcutState};

#[derive(Default)]
struct ShortcutBindings {
    active: Option<Shortcut>,
    cleanup: Option<Shortcut>,
}

#[derive(Default)]
pub(crate) struct DesktopSettingsState {
    pub gate: tokio::sync::Mutex<()>,
    bindings: Mutex<ShortcutBindings>,
    active_id: AtomicU32,
    error: Mutex<Option<CommandError>>,
    recording: AtomicBool,
}

impl DesktopSettingsState {
    pub fn error(&self) -> Result<Option<CommandError>, CommandError> {
        self.error.lock().map(|error| error.clone())
            .map_err(|_| CommandError::keyed("desktop-settings-state", "errors.desktopSettingsState", "无法读取桌面偏好运行状态"))
    }

    pub fn set_error(&self, error: Option<CommandError>) {
        match self.error.lock() {
            Ok(mut current) => *current = error,
            Err(_) => log::error!("无法更新桌面偏好运行状态"),
        }
    }

    pub fn set_recording(&self, recording: bool) {
        self.recording.store(recording, Ordering::Release);
    }
}

/// One canonical chord: ordered modifiers plus a physical letter, digit, Space or F1–F12.
pub(crate) fn parse_shortcut(
    value: &str,
) -> Result<Option<Shortcut>, crate::local_state::DesktopSettingsError> {
    use crate::local_state::DesktopSettingsError;
    if value.is_empty() { return Ok(None); }
    if value.len() > 64 { return Err(DesktopSettingsError::ShortcutTooLong); }
    let shortcut = value
        .parse::<Shortcut>()
        .map_err(|error| DesktopSettingsError::ShortcutParse(error.to_string()))?;
    let key = shortcut.key.to_string();
    let allowed = key == "Space"
        || key.strip_prefix("Key").is_some_and(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase())
        || key.strip_prefix("Digit").is_some_and(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_digit())
        || key.strip_prefix('F').and_then(|s| s.parse::<u8>().ok()).is_some_and(|n| (1..=12).contains(&n));
    if !allowed || !shortcut.mods.intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER) {
        return Err(DesktopSettingsError::ShortcutNotAllowed);
    }
    if shortcut.to_string() != value {
        return Err(DesktopSettingsError::ShortcutNonCanonical);
    }
    Ok(Some(shortcut))
}

/// Register the replacement before releasing the current chord, preserving it on conflict.
pub(crate) fn apply(app: &AppHandle, value: &str) -> Result<(), CommandError> {
    let next = parse_shortcut(value).map_err(|error| {
        let (key, params) = error.parts();
        CommandError::localized("shortcut-invalid", key, error.to_string(), params)
    })?;
    let state = app.state::<DesktopSettingsState>();
    let mut bindings = bindings_lock(&state)
        .ok_or_else(|| CommandError::keyed("shortcut-state", "errors.shortcut.state", "快捷键状态不可用"))?;
    if bindings.active == next && bindings.cleanup.is_none() { return Ok(()); }
    let manager = app.try_state::<GlobalShortcut<tauri::Wry>>().ok_or_else(|| CommandError::keyed(
        "shortcut-service-unavailable", "errors.shortcut.serviceUnavailable",
        "系统全局快捷键服务不可用，请检查桌面环境后重启应用",
    ))?;
    if let Some(shortcut) = bindings.cleanup {
        manager.unregister(shortcut).map_err(|error| CommandError::localized(
            "shortcut-cleanup-failed", "errors.shortcut.cleanupFailed",
            format!("无法清理上次失败的快捷键，请重启应用：{error}"),
            serde_json::json!({ "detail": error.to_string() }),
        ))?;
        bindings.cleanup = None;
    }
    if bindings.active == next { return Ok(()); }
    if let Some(shortcut) = next {
        manager.register(shortcut).map_err(|error| CommandError::localized(
            "shortcut-register-failed", "errors.shortcut.registerFailed",
            format!("无法注册快捷键，可能已被系统或其他应用占用：{error}"),
            serde_json::json!({ "detail": error.to_string() }),
        ))?;
    }
    if let Some(previous) = bindings.active {
        if let Err(error) = manager.unregister(previous) {
            // The cleanup failure is folded into the diagnostic detail; the
            // renderer template carries the primary explanation.
            let mut detail = error.to_string();
            let mut message = format!("无法释放原快捷键：{error}");
            if let Some(shortcut) = next {
                if let Err(cleanup) = manager.unregister(shortcut) {
                    bindings.cleanup = Some(shortcut);
                    let cleanup_text = format!("；清理新快捷键也失败，请重启应用：{cleanup}");
                    message.push_str(&cleanup_text);
                    detail.push_str(&cleanup_text);
                }
            }
            return Err(CommandError::localized(
                "shortcut-unregister-failed", "errors.shortcut.unregisterFailed",
                message, serde_json::json!({ "detail": detail }),
            ));
        }
    }
    bindings.active = next;
    state.active_id.store(next.map(|shortcut| shortcut.id()).unwrap_or(0), Ordering::Release);
    Ok(())
}

fn bindings_lock(state: &DesktopSettingsState) -> Option<std::sync::MutexGuard<'_, ShortcutBindings>> {
    state.bindings.lock().ok()
}

pub(crate) fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new().with_handler(|app, shortcut, event| {
        if event.state() != ShortcutState::Pressed { return; }
        let state = app.state::<DesktopSettingsState>();
        if state.recording.load(Ordering::Acquire) || state.active_id.load(Ordering::Acquire) != shortcut.id() { return; }
        if let Err(error) = toggle_main(app) {
            log::error!("全局快捷键执行失败：{}", error.message);
            state.set_error(Some(error.clone()));
            if let Err(emit_error) = app.emit("desktop-settings-error", &error) {
                log::error!("无法报告快捷键错误：{emit_error}");
            }
        }
    }).build()
}

fn toggle_main(app: &AppHandle) -> Result<(), CommandError> {
    let window = app.get_webview_window("main")
        .ok_or_else(|| CommandError::keyed("main-window-unavailable", "errors.mainWindowUnavailable", "主窗口不可用"))?;
    let error = |cause: tauri::Error| CommandError::new("shortcut-window-failed", cause.to_string());
    if window.is_visible().map_err(error)? && !window.is_minimized().map_err(error)?
        && window.is_focused().map_err(error)? && crate::tray::is_ready() {
        window.hide().map_err(error)?;
        crate::workspace_visibility::publish(app);
        Ok(())
    } else {
        crate::tray::tray_open_main(app.clone())
            .map_err(|cause| CommandError::new("shortcut-window-failed", cause.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_canonical_modifier_and_physical_key_chords() {
        assert!(parse_shortcut("").unwrap().is_none());
        for chord in ["control+KeyK", "shift+control+Space", "alt+Digit7", "super+F12"] {
            assert!(parse_shortcut(chord).unwrap().is_some(), "{chord}");
        }
        for chord in ["KeyK", "shift+KeyK", "Ctrl+K", "control+shift+KeyK", "control+Tab", "control+F13", " control+KeyK"] {
            assert!(parse_shortcut(chord).is_err(), "{chord}");
        }
    }
}
