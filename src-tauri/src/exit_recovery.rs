//! Native exit-blocked feedback and its single frontend recovery destination.
use crate::local_state::{LanguagePreference, LocalState};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Destination { Radar, Backups, ClientManagement, Sessions }

pub(crate) fn active_client_destination() -> Option<Destination> {
    if crate::client_software::gate::history_transferring() { Some(Destination::Sessions) }
    else if crate::client_software::blocks_exit() { Some(Destination::ClientManagement) }
    else { None }
}

pub(crate) fn show(app: &AppHandle, destination: Destination, error: &str) {
    let language = LocalState::from_app(app).and_then(|state| state.get_app_settings())
        .map(|settings| settings.language).unwrap_or(LanguagePreference::System);
    let chinese = match language {
        LanguagePreference::ZhCn => true,
        LanguagePreference::EnUs => false,
        LanguagePreference::System => system_chinese(),
    };
    let copy = match (destination, chinese) {
        (Destination::Sessions, true) => "会话操作正在执行，已取消退出。请在会话记录中等待操作完成。",
        (Destination::Sessions, false) => "A session operation is still running. Wait for it to finish in Sessions before quitting.",
        (Destination::ClientManagement, true) => "客户端正在更新或复验，已取消退出。请在设置的客户端管理中等待操作完成。",
        (Destination::ClientManagement, false) => "A client update or verification is still running. Wait for it to finish in Settings → Client management before quitting.",
        (Destination::Radar, true) => "检测未能安全结束，已取消退出。请在降智雷达中重试保存后退出。",
        (Destination::Radar, false) => "Exit cancelled because the probe could not finish safely. Retry saving in Intelligence radar before quitting.",
        (Destination::Backups, true) => "Claude 接管恢复失败，已取消退出。请在设置的本地备份中处理恢复后再退出。",
        (Destination::Backups, false) => "Exit cancelled because Claude takeover recovery failed. Resolve recovery in Settings → Local backups before quitting.",
    };
    if let Err(open_error) = crate::tray::tray_open_main(app.clone()) {
        log::error!("Cannot show exit recovery workspace: {open_error}");
    }
    if let Some(window) = app.get_webview_window("main") {
        if let Err(event_error) = window.emit("exit-recovery", destination) {
            log::error!("Cannot navigate to exit recovery: {event_error}");
        }
    }
    app.dialog().message(format!("{copy}\n{error}"))
        .title("Agent Switchboard")
        .kind(tauri_plugin_dialog::MessageDialogKind::Error).show(|_| {});
}

#[cfg(windows)]
fn system_chinese() -> bool {
    // The low ten bits identify the primary Windows UI language (Chinese = 4).
    unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() & 0x3ff == 4 }
}

#[cfg(not(windows))]
fn system_chinese() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|key| std::env::var(key).ok()
        .filter(|value| !value.is_empty())).is_some_and(|locale| locale.to_ascii_lowercase().starts_with("zh"))
}
