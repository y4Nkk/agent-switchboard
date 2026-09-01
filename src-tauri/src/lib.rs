mod ccswitch_source;
mod cloud_backup;
mod codex_reset;
mod commands;
mod config_store;
#[cfg(debug_assertions)]
mod dev_api;
mod fonts;
mod local_state;
mod probe;
mod runtime_log;
mod session_manager;
mod tray;
mod update;
mod usage_cache;
mod usage_query;

pub use commands::local_config_paths;

const WRY_DEFAULT_WEBVIEW2_BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

#[cfg(debug_assertions)]
fn web_development_enabled(value: Option<&std::ffi::OsStr>) -> bool {
    value == Some(std::ffi::OsStr::new("1"))
}

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

fn configure_hardware_acceleration<R: tauri::Runtime>(context: &mut tauri::Context<R>) {
    let identifier = context.config().identifier.clone();
    let hardware_acceleration = local_state::LocalState::from_startup_identifier(&identifier)
        .and_then(|state| state.get_app_settings())
        // A missing or malformed app setting must not stop the recovery shell;
        // retain WebView2's current GPU-enabled default in that case.
        .map(|settings| settings.hardware_acceleration)
        .unwrap_or(true);

    apply_hardware_acceleration(&mut context.config_mut().app.windows, hardware_acceleration);
}

/// Runs the Agent Switchboard desktop shell.
pub fn run() {
    use tauri::Manager;

    let mut context = tauri::generate_context!();
    configure_hardware_acceleration(&mut context);

    tauri::Builder::default()
        .plugin(runtime_log::plugin())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            // A malformed settings file is rejected by the typed settings
            // surface, but must never prevent the tray/window recovery shell
            // from starting. Default native window behavior remains usable.
            if let Ok(settings) = local_state::LocalState::from_app(app.handle())
                .and_then(|state| state.get_app_settings())
            {
                runtime_log::set_level(settings.runtime_log_level);
                let _ = commands::apply_desktop_settings(app.handle(), &settings);
            }
            tray::setup(app.handle())?;
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
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if tray::should_absorb(window.app_handle()) {
                    api.prevent_close();
                    // A tray has already been built successfully, so hide is
                    // recoverable. Do not let a hide failure destroy the app.
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::status::config_status,
            commands::list_profiles,
            commands::reset_profile_store,
            commands::create_profile,
            commands::update_profile,
            commands::delete_profile,
            commands::reorder_profiles,
            commands::import_discovered_profile,
            commands::common_settings::get_common_settings_editor,
            commands::common_settings::save_common_settings,
            commands::common_settings::preview_common_settings,
            commands::prompt_management::get_global_prompt_document,
            commands::prompt_management::save_global_prompt_document,
            commands::switching::preview_switch,
            commands::switching::execute_switch,
            commands::switching::list_backups,
            commands::runtime_log::list_runtime_logs,
            commands::runtime_log::open_runtime_log_dir,
            commands::switching::restore_backup,
            commands::switching::undo_last_switch,
            commands::switching::backup_diff,
            commands::switching::open_backup_dir,
            commands::cloud_backup::get_cloud_backup_settings,
            commands::cloud_backup::set_cloud_backup_settings,
            commands::cloud_backup::cloud_backup_setup_sql,
            commands::cloud_backup::upload_cloud_backup,
            commands::cloud_backup::restore_cloud_backup,
            commands::probe_endpoint,
            commands::test_usage_query,
            commands::query_profile_usage,
            commands::fetch_provider_models,
            commands::check_update,
            commands::get_cached_codex_reset_status,
            commands::check_codex_reset_status,
            commands::status::lock_status,
            commands::status::recover_stale_lock,
            commands::discover_local,
            commands::list_sessions,
            commands::get_session_messages,
            commands::resume_session,
            commands::scan_ccswitch,
            commands::import_ccswitch_profiles,
            commands::window::window_minimize,
            commands::window::window_toggle_maximize,
            commands::window::window_is_maximized,
            commands::window::window_close,
            commands::window::restart_application,
            commands::get_app_settings,
            commands::set_app_settings,
            commands::list_system_fonts,
            commands::window::toggle_devtools,
        ])
        .build(context)
        .expect("Agent Switchboard 启动失败")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
                // A user-invoked desktop restart must never enter the
                // close-to-tray path. Tauri uses this dedicated code when it
                // relaunches the executable.
                if code == Some(tauri::RESTART_EXIT_CODE) {
                    return;
                }
                // Tray menu "退出" is the one explicit request allowed to end
                // the process. Every other exit request stays recoverable.
                if !tray::take_explicit_exit() && tray::should_absorb(app) {
                    api.prevent_exit();
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabling_hardware_acceleration_keeps_wry_defaults_and_adds_the_gpu_flag() {
        let mut windows = vec![tauri::utils::config::WindowConfig::default()];
        apply_hardware_acceleration(&mut windows, false);

        assert_eq!(
            windows[0].additional_browser_args.as_deref(),
            Some("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --disable-gpu")
        );
    }

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
