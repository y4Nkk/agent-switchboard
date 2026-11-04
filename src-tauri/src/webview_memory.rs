use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
    COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
};
use windows::core::Interface;
use crate::runtime_log::{record_failure, record_success, RuntimeLogAction};

struct MemoryState {
    applied_visible: Option<bool>,
    last_error: Option<(bool, String)>,
}

impl MemoryState {
    fn update(&mut self, visible: bool, set: impl FnOnce() -> Result<(), String>) -> Option<Result<(), String>> {
        if self.applied_visible == Some(visible) { return None; }
        match set() {
            Ok(()) => { self.applied_visible = Some(visible); self.last_error = None; Some(Ok(())) }
            Err(code) => {
                self.applied_visible = None;
                let failed = (visible, code.clone());
                if self.last_error.as_ref() == Some(&failed) { return None; }
                self.last_error = Some(failed);
                Some(Err(code))
            }
        }
    }
}

static STATE: Mutex<MemoryState> = Mutex::new(MemoryState { applied_visible: None, last_error: None });

fn set_target(view: &tauri::webview::PlatformWebview, visible: bool) -> Result<(), String> {
    let level = if visible { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL }
        else { COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW };
    let code = |stage: &str, error: windows::core::Error| format!("webview-memory-{stage}-{:08x}", error.code().0 as u32);
    let core = unsafe { view.controller().CoreWebView2()
            .and_then(|core| core.cast::<ICoreWebView2_19>())
    }.map_err(|error| code("interface", error))?;
    unsafe { core.SetMemoryUsageTargetLevel(level) }.map_err(|error| code("set", error))?;
    let mut actual = COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL;
    unsafe { core.MemoryUsageTargetLevel(&mut actual) }.map_err(|error| code("read", error))?;
    if actual != level { return Err("webview-memory-readback-mismatch".into()); }
    Ok(())
}

fn apply(view: &tauri::webview::PlatformWebview, visible: bool) {
    let action = if visible { RuntimeLogAction::WebviewMemoryNormal } else { RuntimeLogAction::WebviewMemoryLow };
    let Ok(mut state) = STATE.lock() else {
        record_failure(action, "webview-memory-state-unavailable");
        return;
    };
    match state.update(visible, || set_target(view, visible)) {
        Some(Ok(())) => record_success(action),
        Some(Err(code)) => record_failure(action, &code),
        None => {}
    }
}

pub(crate) fn synchronize(app: &AppHandle, observed: bool) {
    #[cfg(debug_assertions)]
    if crate::web_development_enabled(std::env::var_os("ASB_WEB_DEVELOPMENT").as_deref()) {
        crate::workspace_visibility::emit(app, observed);
        return;
    }
    let handle = app.clone();
    if app.run_on_main_thread(move || synchronize_on_main(&handle, observed)).is_err() {
        record_failure(if observed { RuntimeLogAction::WebviewMemoryNormal } else { RuntimeLogAction::WebviewMemoryLow },
            "webview-memory-dispatch-failed");
        crate::workspace_visibility::emit(app, observed);
    }
}

fn synchronize_on_main(app: &AppHandle, observed: bool) {
    let Some(window) = app.get_webview_window("main") else { return; };
    // Re-read within this serialized main-thread turn so an older hide cannot
    // override a restore. Wry executes the native callback inline on this thread.
    let visible = match crate::workspace_visibility::visible(app) {
        Ok(visible) => visible,
        Err(_) => {
            record_failure(if observed { RuntimeLogAction::WebviewMemoryNormal } else { RuntimeLogAction::WebviewMemoryLow },
                "webview-memory-visibility-failed");
            return;
        }
    };
    if window.with_webview(move |view| apply(&view, visible)).is_err() {
        record_failure(if visible { RuntimeLogAction::WebviewMemoryNormal } else { RuntimeLogAction::WebviewMemoryLow },
            "webview-memory-dispatch-failed");
    }
    // Emitting inside with_webview re-enters Wry's locked WebView dispatcher.
    // Deliver visibility only after leaving it, with Normal already restored.
    crate::workspace_visibility::emit(app, visible);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_targets_are_deduplicated_and_restore_requires_a_real_set() {
        let mut state = MemoryState { applied_visible: None, last_error: None };
        assert_eq!(state.update(false, || Ok(())), Some(Ok(())));
        assert_eq!(state.update(false, || panic!("duplicate native call")), None);
        assert_eq!(state.update(true, || Ok(())), Some(Ok(())));
        assert_eq!(state.applied_visible, Some(true));
    }

    #[test]
    fn retries_failed_targets_without_repeating_the_same_diagnostic() {
        let mut state = MemoryState { applied_visible: None, last_error: None };
        assert_eq!(state.update(false, || Err("native-failure".into())), Some(Err("native-failure".into())));
        assert_eq!(state.applied_visible, None);
        assert_eq!(state.update(false, || Err("native-failure".into())), None);
        assert_eq!(state.update(true, || Err("native-failure".into())), Some(Err("native-failure".into())));
        assert_eq!(state.update(false, || Ok(())), Some(Ok(())));
        assert_eq!(state.applied_visible, Some(false));
    }

    #[test]
    fn failed_readback_invalidates_the_old_target_so_restore_reapplies_normal() {
        let mut state = MemoryState { applied_visible: Some(true), last_error: None };
        state.update(false, || Err("webview-memory-read-80004005".into()));
        assert_eq!(state.applied_visible, None);
        assert_eq!(state.update(true, || Ok(())), Some(Ok(())));
        assert_eq!(state.applied_visible, Some(true));
    }
}
