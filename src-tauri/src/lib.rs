mod app_paths;
mod ccswitch_source;
mod changelog;
mod client_config_watcher;
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
mod official_login;
mod outbound_proxy;
mod probe;
mod process_control;
mod provider_diagnostics;
mod provider_request;
mod provider_transfer;
mod runtime_log;
mod session_manager;
mod storage;
#[cfg(test)]
mod test_client_paths;
mod tray;
mod upstream_overrides;
mod usage_cache;
mod usage_history;
mod usage_query;
mod xai_auth;

pub use commands::local_config_paths;

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
fn reveal_main_window(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
    }
}

/// Runs the Agent Switchboard desktop shell.
pub fn run() {
    let mut context = tauri::generate_context!();
    let log_directory =
        app_paths::log_directory(&context.config().identifier).expect("无法定位应用日志目录");
    #[cfg(windows)]
    let startup_windows = app_paths::prepare_windows(&mut context);

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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(move |app| {
            let local = local_state::LocalState::from_app(app.handle())
                .map_err(std::io::Error::other)?;
            // Upgrade after the single-instance guard, before creating Windows WebViews
            // or reading any saved startup preference.
            if let Err(error) = local.upgrade_app_settings() {
                log::error!("应用设置升级失败，已保留原始文件：{error}");
            }
            #[cfg(windows)]
            initialize_window_preferences(app.handle(), &local);
            #[cfg(windows)]
            app_paths::build_windows(app.handle(), &startup_windows)
                .map_err(std::io::Error::other)?;
            let local = initialize_local_services(app, local)?;
            apply_startup_settings(app);
            start_background_services(app, &local)
        })
        .on_window_event(handle_window_event)
        .invoke_handler(crate::command_registry::handler!())
        .build(context)
        .expect("Agent Switchboard 启动失败")
        .run(handle_run_event);
}

fn initialize_local_services(
    app: &mut tauri::App,
    local: local_state::LocalState,
) -> Result<local_state::LocalState, Box<dyn std::error::Error>> {
    use tauri::Manager;
    // Outbound proxy settings gate every gateway-side HTTP client;
    // load them before any client is built. A malformed file defaults
    // to direct egress instead of blocking startup.
    if let Err(error) = outbound_proxy::load(local.root()) {
        log::warn!("出站代理设置不可用，按直连处理：{error}");
    }
    app.manage(commands::ConfigWriteGate::default());
    let write_gate = app.state::<commands::ConfigWriteGate>().inner().clone();
    // Startup can replay a pending port change before the controller
    // is published. Hold the same gate as every later client-config
    // transaction so that recovery has one write boundary.
    let _write_guard = write_gate.lock().map_err(std::io::Error::other)?;
    // Invalid application configuration leaves the shell alive so the
    // repair entry can explain the failure without touching client files.
    let configuration_ready = match local.initialize_configuration_store() {
        Ok(()) => true,
        Err(error) => {
            log::error!("供应商存储不可用，已进入修复状态: {error}");
            false
        }
    };
    local
        .initialize_extension_schema()
        .map_err(std::io::Error::other)?;
    // The gateway controller always exists; a failed port bind or an
    // unusable state file becomes a visible runtime state instead of
    // refusing the window.
    app.manage(gateway::GatewayController::start_with_write_lock(
        &local,
        write_gate.shared(),
    ));
    app.manage(gateway::PortChangePreparations::default());
    app.manage(commands::switching::ProfileSavePreparations::default());
    app.manage(commands::switching::CodexProfileSavePreparations::default());
    app.manage(commands::switching::CodexPolicyPreparations::default());
    app.manage(provider_request::ProviderRequests::default());
    app.manage(commands::provider_diagnostics::ProviderRepairPreparations::default());
    app.manage(codex_probe::ProbeRegistry::new());
    // A batch left running by an abrupt exit is closed as interrupted before
    // any command can read it; finished results and saved session usage stay.
    if let Err(error) = codex_probe::recover_interrupted(&local) {
        log::warn!("降智检测历史恢复失败：{error}");
    }
    if configuration_ready {
        if let Err(error) = commands::switching::recover_pending_profile_save(app.handle()) {
            log::error!("配置事务需要恢复，已保留事务和备份：{error}");
        }
    }
    if let Err(error) = commands::switching::codex_policy::recover_on_startup(
        &local, app.state::<gateway::GatewayController>().inner()) {
        log::error!("Codex 网关策略需要恢复，已保留原始事务：{error}");
    }
    Ok(local)
}

fn apply_startup_settings(app: &tauri::App) {
    use tauri::Manager;
    if let Err(error) = app.handle().plugin(desktop_shortcut::plugin()) {
        log::error!("全局快捷键服务初始化失败：{error}");
        app.state::<desktop_shortcut::DesktopSettingsState>()
            .set_error(Some(commands::error::CommandError::localized(
                "shortcut-service-unavailable", "errors.shortcut.serviceUnavailable",
                error.to_string(), serde_json::Value::Null,
            )));
    }
    // A malformed settings file is rejected by the typed settings
    // surface, but must never prevent the tray/window recovery shell
    // from starting. Default native window behavior remains usable.
    match local_state::LocalState::from_app(app.handle())
        .and_then(|state| state.get_app_settings())
    {
        Ok(settings) => {
            runtime_log::set_level(settings.runtime_log_level);
            let applied = commands::apply_desktop_settings(app.handle(), &settings);
            if let Err(error) = &applied {
                log::error!("桌面偏好应用失败：{}", error.message);
                app.state::<desktop_shortcut::DesktopSettingsState>().set_error(Some(error.clone()));
            }
            if !settings.start_minimized || applied.is_err() {
                reveal_main_window(app.handle());
            }
        }
        Err(_) => reveal_main_window(app.handle()),
    }
}

fn start_background_services(app: &mut tauri::App, local: &local_state::LocalState) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::Manager;
    let watcher = client_config_watcher::ClientConfigWatcher::start(
        app.handle().clone(),
        [
            local.target(asb_core::AppKind::Codex).map_err(std::io::Error::other)?,
            local.target(asb_core::AppKind::Claude).map_err(std::io::Error::other)?,
        ],
    )
    .map_err(std::io::Error::other)?;
    app.manage(watcher);
    if let Err(error) = tray::setup(app.handle()) {
        tray::recover_main(app.handle(), &error);
    }
    // The tray panel is a persistent surface; its usage data must
    // refresh even while the main window is hidden, closed to the
    // tray, or on another page. The scheduler thread owns that
    // cadence, so no renderer lifecycle can stop it.
    usage_query::scheduler::spawn(app.handle().clone());
    storage::spawn(app.handle().clone());
    session_manager::history::spawn(app.handle(), local.root().to_path_buf());
    runtime_log::record_started();
    #[cfg(debug_assertions)]
    {
        let web_development = std::env::var_os("ASB_WEB_DEVELOPMENT");
        if web_development_enabled(web_development.as_deref()) {
            let development_origin = app
                .config()
                .build
                .dev_url
                .as_ref()
                .map(|url| url.origin().ascii_serialization())
                .ok_or_else(|| std::io::Error::other("缺少浏览器开发地址"))?;
            dev_api::start(app.handle().clone(), development_origin)
                .map_err(std::io::Error::other)?;
            // The persistent Vite process owns the one-shot browser launch;
            // Tauri restarts this process for every backend hot reload.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        }
    }
    Ok(())
}

fn handle_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    use tauri::Manager;
    tray::popup::window_event(window, event);
    if window.label() != "main" {
        return;
    }
    if let tauri::WindowEvent::Focused(false) = event {
        window.app_handle().state::<desktop_shortcut::DesktopSettingsState>().set_recording(false);
    }
    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
        if tray::should_absorb(window.app_handle()) {
            api.prevent_close();
            // A tray has already been built successfully, so hide is
            // recoverable. Do not let a hide failure destroy the app.
            let _ = window.hide();
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
    use tauri::Manager;
    if matches!(&event, tauri::RunEvent::Exit) {
        session_manager::history::flush_on_exit(app);
        return;
    }
    if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
        // A user-invoked desktop restart must never enter the
        // close-to-tray path. Tauri uses this dedicated code when it
        // relaunches the executable.
        if code == Some(tauri::RESTART_EXIT_CODE) {
            // Restart is a normal exit: stop the in-flight probe call and
            // record the cancellation before the process is replaced.
            if let Err(error) = codex_probe::shutdown_on_exit(app) {
                api.prevent_exit();
                log::error!("检测未能安全结束，已取消退出：{error}");
                exit_recovery::show(app, exit_recovery::Destination::Radar, &error.to_string());
                return;
            }
            return;
        }
        // Explicit quit and the configured close-to-exit action end
        // the process; implicit exits remain recoverable via the tray.
        if !tray::take_explicit_exit() && tray::should_absorb(app) {
            api.prevent_exit();
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        } else {
            // Terminate any probe subprocess first so no detection call
            // outlives the shell and keeps spending quota.
            if let Err(error) = codex_probe::shutdown_on_exit(app) {
                api.prevent_exit();
                log::error!("检测未能安全结束，已取消退出：{error}");
                exit_recovery::show(app, exit_recovery::Destination::Radar, &error.to_string());
                return;
            }
            if let Some(gateway) = app.try_state::<gateway::GatewayController>() {
                if let Err(error) = commands::switching::claude_gateway::restore_on_exit(app) {
                    api.prevent_exit();
                    app.state::<codex_probe::ProbeRegistry>().resume_after_blocked_exit();
                    log::error!("Claude 接管恢复失败，已取消退出：{error}");
                    exit_recovery::show(app, exit_recovery::Destination::Backups, &error.to_string());
                } else {
                    gateway.shutdown();
                }
            }
        }
    }
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
