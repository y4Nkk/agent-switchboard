mod app_paths;
mod application_data_io;
mod application_preparation;
mod startup;
mod app_update;
mod ccswitch_source;
mod changelog;
mod client_config_watcher;
mod client_software;
mod claude_auth;
mod claude_integration;
mod cloud_backup;
mod codex_auth;
mod codex_common;
mod codex_metering;
mod codex_official_quota;
mod codex_probe;
mod codex_project_plans;
mod codex_prompts;
mod codex_reset;
mod command_registry;
mod commands;
mod config_store;
#[cfg(debug_assertions)]
mod dev_api;
mod distribution;
mod desktop_shortcut;
mod extensions;
mod exit_recovery;
mod fonts;
mod gateway;
mod local_state;
mod model_usage;
mod model_usage_cache;
mod native_telemetry;
mod request_usage;
mod official_login;
mod outbound_proxy;
mod probe;
mod process_control;
mod provider_diagnostics;
mod provider_request;
mod provider_transfer;
mod resource_monitor;
mod runtime_log;
mod session_manager;
mod storage;
#[cfg(test)]
mod test_client_paths;
mod tray;
mod update_temp_sweep;
mod upstream_overrides;
mod usage_cache;
mod usage_history;
mod usage_query;
mod workspace_visibility;
#[cfg(windows)]
mod webview_memory;
mod xai_auth;

pub use commands::local_config_paths;
use tauri::Manager;

/// Hardware acceleration is a WebView2 (Windows) creation-time concept; other
/// platforms keep their webview engine defaults.
#[cfg(windows)]
const WRY_DEFAULT_WEBVIEW2_BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

#[cfg(debug_assertions)]
pub(crate) fn web_development_enabled(value: Option<&std::ffi::OsStr>) -> bool {
    value == Some(std::ffi::OsStr::new("1"))
}

#[cfg(windows)]
fn apply_hardware_acceleration(
    windows: &mut [tauri::utils::config::WindowConfig],
    hardware_acceleration: bool,
) {
    if hardware_acceleration {
        return;
    }

    for window in windows {
        // Supplying an explicit argument string replaces Wry's defaults, so
        // seed this exact default before adding the GPU switch.
        let arguments = window
            .additional_browser_args
            .get_or_insert_with(|| WRY_DEFAULT_WEBVIEW2_BROWSER_ARGS.to_string());
        if !arguments
            .split_ascii_whitespace()
            .any(|argument| argument == "--disable-gpu")
        {
            if !arguments.is_empty() {
                arguments.push(' ');
            }
            arguments.push_str("--disable-gpu");
        }
    }
}

#[cfg(windows)]
struct StartupHardwareAcceleration(bool);

#[cfg(windows)]
pub(crate) fn apply_startup_hardware_acceleration(
    app: &tauri::AppHandle,
    windows: &mut [tauri::utils::config::WindowConfig],
) {
    use tauri::Manager;
    apply_hardware_acceleration(windows, app.state::<StartupHardwareAcceleration>().inner().0);
}

#[cfg(windows)]
fn initialize_window_preferences(app: &tauri::AppHandle, local: &local_state::LocalState) {
    use tauri::Manager;
    // This creation-time preference requires a restart. Recreated tray windows
    // must use the same WebView2 environment arguments as the main window.
    let enabled = local.get_app_settings()
        .map(|settings| settings.hardware_acceleration)
        .unwrap_or(true);
    app.manage(StartupHardwareAcceleration(enabled));
}

/// The main window is created hidden so startup owns its visibility in one
/// place. It stays hidden only for an explicit start-minimized preference; a
/// settings read failure also reveals it, keeping the recovery shell reachable.
pub(crate) fn reveal_main_window(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        workspace_visibility::publish(app);
    }
}

/// Runs the Agent Switchboard desktop shell.
pub fn run() {
    let mut context = tauri::generate_context!();
    let log_directory =
        app_paths::log_directory(&context.config().identifier).expect("无法定位应用日志目录");
    #[cfg(windows)]
    let startup_windows = app_paths::prepare_windows(&mut context);

    let command_handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = crate::command_registry::handler!();
    tauri::Builder::default()
        // Registered first so a duplicate launch exits during plugin init,
        // before the gateway, tray, or any window state is created. The
        // callback runs in the surviving instance and restores its window.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Err(error) = tray::tray_open_main(app.clone()) {
                log::warn!("重复启动已拦截，恢复主窗口失败: {error}");
            }
        }))
        .plugin(runtime_log::plugin(log_directory))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(desktop_shortcut::DesktopSettingsState::default())
        .manage(app_update::UpdateManager::default())
        .manage(client_software::SoftwareManager::default())
        .manage(std::sync::Arc::new(session_manager::range::Operations::default()))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(move |app| {
            #[cfg(windows)]
            return start_application(app, &startup_windows);
            #[cfg(not(windows))]
            start_application(app)
        })
        .on_window_event(handle_window_event)
        .invoke_handler(move |invoke| {
            if let Err(error) = application_preparation::authorize(invoke.message.webview_ref().app_handle(), invoke.message.command()) {
                invoke.resolver.reject(error); return true;
            }
            command_handler(invoke)
        })
        .build(context)
        .expect("Agent Switchboard 启动失败")
        .run(handle_run_event);
}

fn start_application(app: &mut tauri::App,
    #[cfg(windows)] windows: &[tauri::utils::config::WindowConfig]) -> Result<(), Box<dyn std::error::Error>> {
    let local = local_state::LocalState::from_app(app.handle())
        .map_err(std::io::Error::other)?;
    let preferences = local.prepare_app_settings();
    let initial_settings = preferences.as_ref().ok().map(|prepared| prepared.settings.clone());
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    use tauri::Manager;
    app.manage(local.clone());
    app.manage(commands::ConfigWriteGate::default());
    app.manage(application_preparation::Preparation::new(initial_settings.clone(), sender.clone()));
    #[cfg(windows)]
    initialize_window_preferences(app.handle(), &local);
    #[cfg(windows)]
    app_paths::build_windows(app.handle(), windows).map_err(std::io::Error::other)?;
    if initial_settings.as_ref().is_none_or(|settings| !settings.start_minimized) {
        reveal_main_window(app.handle());
    }
    #[cfg(debug_assertions)]
    start_development_api(app.handle()).map_err(std::io::Error::other)?;
    application_preparation::start(app.handle().clone(), local, preferences, sender, receiver);
    Ok(())
}

#[cfg(debug_assertions)]
fn start_development_api(app: &tauri::AppHandle) -> Result<(), String> {
    if !web_development_enabled(std::env::var_os("ASB_WEB_DEVELOPMENT").as_deref()) { return Ok(()); }
    let origin = app.config().build.dev_url.as_ref().map(|url| url.origin().ascii_serialization())
        .ok_or("缺少浏览器开发地址")?;
    dev_api::start(app.clone(), origin)?;
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") { let _ = window.hide(); }
    workspace_visibility::publish(app);
    Ok(())
}

fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    use tauri::Manager;
    tray::popup::window_event(window, event);
    if window.label() != "main" {
        return;
    }
    workspace_visibility::publish(window.app_handle());
    if let tauri::WindowEvent::Focused(false) = event {
        window.app_handle().state::<desktop_shortcut::DesktopSettingsState>().set_recording(false);
    }
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        if tray::should_absorb(window.app_handle()) {
            api.prevent_close();
            // A tray has already been built successfully, so hide is
            // recoverable. Do not let a hide failure destroy the app.
            let _ = window.hide();
            workspace_visibility::publish(window.app_handle());
        } else {
            // The persistent hidden tray WebView is still a window;
            // closing only main would otherwise leave the process alive.
            api.prevent_close();
            tray::request_explicit_exit();
            window.app_handle().exit(0);
        }
    }
}

fn handle_run_event(app: &tauri::AppHandle, event: tauri::RunEvent) {
    if matches!(&event, tauri::RunEvent::Exit) {
        if app.try_state::<application_preparation::Preparation>().is_some_and(|state| state.require_ready().is_ok()) {
            session_manager::history::flush_on_exit(app);
        }
        return;
    }
    if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
        if app.try_state::<application_preparation::Preparation>().is_some_and(|state| state.defer_exit(code)) {
            api.prevent_exit(); return;
        }
        if exit_blocked_by_operations(app) { api.prevent_exit(); resume_preparation(app); return; }
        if code == Some(tauri::RESTART_EXIT_CODE) {
            if defer_final_preparation_exit(app, code) || !stop_probe_for_exit(app) { api.prevent_exit(); }
            return;
        }
        if !tray::take_explicit_exit() && tray::should_absorb(app) {
            api.prevent_exit();
            if let Some(window) = app.get_webview_window("main") { let _ = window.hide(); workspace_visibility::publish(app); }
        } else {
            if defer_final_preparation_exit(app, code) || !stop_probe_for_exit(app) { api.prevent_exit(); return; }
            if let Some(gateway) = app.try_state::<gateway::GatewayController>() {
                if let Err(error) = commands::switching::claude_gateway::restore_on_exit(app) {
                    api.prevent_exit(); resume_preparation(app);
                    app.state::<codex_probe::ProbeRegistry>().resume_after_blocked_exit();
                    log::error!("Claude 接管恢复失败，已取消退出：{error}");
                    exit_recovery::show(app, exit_recovery::Destination::Backups, &error.to_string());
                } else { gateway.shutdown(); }
            }
        }
    }
}

fn exit_blocked_by_operations(app: &tauri::AppHandle) -> bool {
    match app.state::<std::sync::Arc<session_manager::range::Operations>>().progress() {
        Ok(progress) if progress.running => { exit_recovery::show(app, exit_recovery::Destination::Sessions, ""); return true; }
        Err(error) => { exit_recovery::show(app, exit_recovery::Destination::Sessions, &error); return true; }
        _ => {}
    }
    if let Some(destination) = exit_recovery::active_client_destination() {
        exit_recovery::show(app, destination, ""); return true;
    }
    false
}

fn stop_probe_for_exit(app: &tauri::AppHandle) -> bool {
    if app.try_state::<codex_probe::ProbeRegistry>().is_none() { return true; }
    if let Err(error) = codex_probe::shutdown_on_exit(app) {
        resume_preparation(app);
        log::error!("检测未能安全结束，已取消退出：{error}");
        exit_recovery::show(app, exit_recovery::Destination::Radar, &error.to_string());
        return false;
    }
    true
}

fn defer_final_preparation_exit(app: &tauri::AppHandle, code: Option<i32>) -> bool {
    let pending = app.try_state::<application_preparation::Preparation>().is_some_and(|state| state.prepare_exit(code));
    if pending { tray::request_explicit_exit(); }
    pending
}

fn resume_preparation(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<application_preparation::Preparation>() { state.resume_after_blocked_exit(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn disabling_hardware_acceleration_keeps_wry_defaults_and_adds_the_gpu_flag() {
        let mut windows = vec![tauri::utils::config::WindowConfig::default()];
        apply_hardware_acceleration(&mut windows, false);

        assert_eq!(
            windows[0].additional_browser_args.as_deref(),
            Some("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu")
        );
    }

    #[cfg(windows)]
    #[test]
    fn enabled_hardware_acceleration_leaves_existing_browser_arguments_unchanged() {
        let mut windows = vec![tauri::utils::config::WindowConfig::default()];
        windows[0].additional_browser_args =
            Some("--autoplay-policy=no-user-gesture-required".to_string());

        apply_hardware_acceleration(&mut windows, true);

        assert_eq!(
            windows[0].additional_browser_args.as_deref(),
            Some("--autoplay-policy=no-user-gesture-required")
        );
    }

    #[test]
    fn web_development_requires_the_exact_enabled_value() {
        assert!(web_development_enabled(Some(std::ffi::OsStr::new("1"))));
        assert!(!web_development_enabled(None));
        assert!(!web_development_enabled(Some(std::ffi::OsStr::new(""))));
        assert!(!web_development_enabled(Some(std::ffi::OsStr::new("0"))));
        assert!(!web_development_enabled(Some(std::ffi::OsStr::new("true"))));
    }
}
