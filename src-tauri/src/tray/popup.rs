//! A persistent, initially hidden WebView. Readiness and visibility are separate
//! so the first activation cannot display an unpainted window.

use super::position::{place, Bounds};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, Rect, WebviewWindow, WebviewWindowBuilder,
};

pub const LABEL: &str = "tray";

#[derive(Default)]
struct Lifecycle {
    ready: bool,
    requested: bool,
    visible: bool,
    focus_hidden_at: Option<Instant>,
}

impl Lifecycle {
    fn suppress_click(&mut self, now: Instant) -> bool {
        self.focus_hidden_at
            .take()
            .is_some_and(|at| now.saturating_duration_since(at) < Duration::from_millis(250))
    }
}

struct PopupState {
    lifecycle: Lifecycle,
    anchor: Option<Bounds>,
    height: f64,
    generation: u64,
}

pub fn setup(app: &AppHandle) -> Result<(), String> {
    let height = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == LABEL)
        .ok_or("托盘窗口配置缺失")?
        .height;
    app.manage(Mutex::new(PopupState {
        lifecycle: Lifecycle::default(),
        anchor: None,
        height,
        generation: 0,
    }));
    // WebView creation is supported during setup; doing it synchronously in
    // the Windows tray-event handler can deadlock WebView2 initialization.
    ensure_window(app)?;
    Ok(())
}

fn with_state<T>(app: &AppHandle, action: impl FnOnce(&mut PopupState) -> T) -> Result<T, String> {
    let managed = app
        .try_state::<Mutex<PopupState>>()
        .ok_or("托盘窗口状态尚未初始化")?;
    let mut state = managed.lock().map_err(|_| "托盘窗口状态不可用")?;
    Ok(action(&mut state))
}

fn ensure_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        return Ok(window);
    }
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == LABEL)
        .ok_or("托盘窗口配置缺失")?;
    let window = WebviewWindowBuilder::from_config(app, config)
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| format!("托盘窗口创建失败: {error}"))?;
    #[cfg(target_os = "windows")]
    apply_window_outline(&window)?;
    Ok(window)
}

#[cfg(target_os = "windows")]
fn apply_window_outline(window: &WebviewWindow) -> Result<(), String> {
    use std::ffi::c_void;
    #[link(name = "dwmapi", kind = "raw-dylib")]
    extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: *mut c_void,
            attribute: u32,
            value: *const u32,
            size: u32,
        ) -> i32;
    }
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWA_BORDER_COLOR: u32 = 34;
    const DWMWCP_ROUND: u32 = 2;
    const DWMWA_COLOR_NONE: u32 = 0xfffffffe;
    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    // DWM owns the only outline. Unsupported visual attributes leave the same
    // borderless window rectangular; they do not create a second surface.
    for (attribute, value) in [
        (DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND),
        (DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE),
    ] {
        let result = unsafe { DwmSetWindowAttribute(hwnd.0 as *mut c_void, attribute, &value, 4) };
        if result < 0 {
            log::debug!("托盘窗口外观属性 {attribute} 不可用: {result:#x}");
        }
    }
    Ok(())
}

pub fn toggle(
    app: &AppHandle,
    rect: Option<Rect>,
    point: Option<PhysicalPosition<f64>>,
) -> Result<(), String> {
    let close = with_state(app, |state| {
        let suppress = state.lifecycle.suppress_click(Instant::now());
        state.lifecycle.visible || state.lifecycle.requested || suppress
    })?;
    if close {
        return hide(app, false);
    }
    let point = point.or_else(|| app.cursor_position().ok());
    let scale = point
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .map(|monitor| monitor.scale_factor())
        .unwrap_or(1.0);
    let anchor = rect
        .map(|rect| {
            let position = rect.position.to_physical::<f64>(scale);
            let size = rect.size.to_physical::<f64>(scale);
            Bounds {
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
            }
        })
        .or_else(|| {
            point.map(|p| Bounds {
                x: p.x,
                y: p.y,
                width: 0.0,
                height: 0.0,
            })
        });
    let generation = with_state(app, |state| {
        state.anchor = anchor;
        state.lifecycle.requested = true;
        state.generation = state.generation.wrapping_add(1);
        state.generation
    })?;
    if app.get_webview_window(LABEL).is_none() {
        let _ = hide(app, false);
        return Err("托盘窗口不可用，请重新启动应用".into());
    }
    super::refresh(app);
    if with_state(app, |state| state.lifecycle.ready)? {
        show(app)?;
    } else {
        let app = app.clone();
        std::thread::Builder::new()
            .name("tray-ready-timeout".into())
            .spawn(move || {
                std::thread::sleep(Duration::from_secs(8));
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || {
                    let timed_out = with_state(&handle, |state| {
                        state.generation == generation
                            && state.lifecycle.requested
                            && !state.lifecycle.ready
                    })
                    .unwrap_or(false);
                    if timed_out {
                        let _ = hide(&handle, false);
                        if let Some(window) = handle.get_webview_window(LABEL) {
                            if let Err(error) = window.reload() {
                                log::warn!("托盘界面重新加载失败: {error}");
                            }
                        }
                        super::recover_main(&handle, "托盘界面未能完成加载，请重试");
                    }
                });
            })
            .map_err(|error| format!("无法监测托盘加载: {error}"))?;
    }
    Ok(())
}

fn position(app: &AppHandle, window: &WebviewWindow) -> Result<(), String> {
    let (anchor, height) = with_state(app, |state| (state.anchor, state.height))?;
    let monitor = match anchor {
        Some(anchor) => app
            .monitor_from_point(
                anchor.x + anchor.width / 2.0,
                anchor.y + anchor.height / 2.0,
            )
            .map_err(|error| error.to_string())?,
        None => None,
    }
    .or(app.primary_monitor().map_err(|error| error.to_string())?)
    .ok_or("无法取得托盘所在显示器")?;
    let area = monitor.work_area();
    let work = Bounds {
        x: area.position.x as f64,
        y: area.position.y as f64,
        width: area.size.width as f64,
        height: area.size.height as f64,
    };
    let anchor = anchor.unwrap_or(Bounds {
        x: work.x + work.width / 2.0,
        y: work.y,
        width: 0.0,
        height: 0.0,
    });
    let bounds = place(work, anchor, monitor.scale_factor(), height);
    window
        .set_size(PhysicalSize::new(
            bounds.width.round() as u32,
            bounds.height.round() as u32,
        ))
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "linux")]
    if super::linux::is_wayland(window)? {
        // Wayland has no global toplevel coordinate contract. The compositor
        // positions this same panel; fabricating root coordinates is incorrect.
        return Ok(());
    }
    window
        .set_position(PhysicalPosition::new(
            bounds.x.round() as i32,
            bounds.y.round() as i32,
        ))
        .map_err(|error| error.to_string())
}

fn show(app: &AppHandle) -> Result<(), String> {
    if !with_state(app, |state| state.lifecycle.requested)? {
        return Ok(());
    }
    let window = app.get_webview_window(LABEL).ok_or("托盘窗口不可用")?;
    let result = position(app, &window)
        .and_then(|()| window.show().map_err(|error| error.to_string()))
        .and_then(|()| window.set_focus().map_err(|error| error.to_string()));
    if let Err(error) = result {
        let _ = hide(app, false);
        super::recover_main(app, &error);
        return Err(error);
    }
    with_state(app, |state| {
        state.lifecycle.visible = true;
        state.lifecycle.requested = false;
    })
}

pub fn ready(app: &AppHandle) -> Result<(), String> {
    with_state(app, |state| state.lifecycle.ready = true)?;
    show(app)
}

pub fn resize(app: &AppHandle, height: f64) -> Result<(), String> {
    if !height.is_finite() || height <= 0.0 {
        return Err("托盘内容高度无效".into());
    }
    let visible = with_state(app, |state| {
        state.height = height;
        state.lifecycle.visible
    })?;
    if visible {
        if let Some(window) = app.get_webview_window(LABEL) {
            position(app, &window)?;
        }
    }
    Ok(())
}

pub fn hide(app: &AppHandle, focus_lost: bool) -> Result<(), String> {
    let cursor = focus_lost.then(|| app.cursor_position().ok()).flatten();
    if let Some(window) = app.get_webview_window(LABEL) {
        window.hide().map_err(|error| error.to_string())?;
    }
    with_state(app, |state| {
        // Only a loss of focus on the actual tray anchor can belong to the
        // same icon click. Clicking another window never suppresses reopening.
        if focus_lost && state.lifecycle.visible {
            state.lifecycle.focus_hidden_at = state
                .anchor
                .zip(cursor)
                .filter(|(anchor, p)| {
                    p.x >= anchor.x
                        && p.y >= anchor.y
                        && p.x <= anchor.x + anchor.width
                        && p.y <= anchor.y + anchor.height
                })
                .map(|_| Instant::now());
        }
        state.lifecycle.visible = false;
        state.lifecycle.requested = false;
        state.generation = state.generation.wrapping_add(1);
    })
}

pub fn window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() != LABEL {
        return;
    }
    let app = window.app_handle();
    let result = match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide(app, false)
        }
        tauri::WindowEvent::Focused(false) => {
            if with_state(app, |state| state.lifecycle.visible).unwrap_or(false) {
                hide(app, true)
            } else {
                Ok(())
            }
        }
        tauri::WindowEvent::ScaleFactorChanged { .. } => {
            if let Some(window) = app.get_webview_window(LABEL) {
                position(app, &window)
            } else {
                Ok(())
            }
        }
        tauri::WindowEvent::Destroyed => {
            with_state(app, |state| state.lifecycle = Lifecycle::default())
        }
        _ => Ok(()),
    };
    if let Err(error) = result {
        super::recover_main(app, &error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_click_after_focus_loss_does_not_reopen_same_panel() {
        let now = Instant::now();
        let mut lifecycle = Lifecycle {
            focus_hidden_at: Some(now),
            ..Lifecycle::default()
        };
        assert!(lifecycle.suppress_click(now + Duration::from_millis(40)));
        assert!(!lifecycle.suppress_click(now + Duration::from_millis(60)));
    }
}
