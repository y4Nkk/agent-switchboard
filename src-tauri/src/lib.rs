mod commands;
mod local_state;
mod probe;

pub use commands::local_config_paths;

/// Runs the Agent Switchboard desktop shell.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::config_status,
            commands::list_profiles,
            commands::create_profile,
            commands::update_profile,
            commands::delete_profile,
            commands::import_discovered_profile,
            commands::get_common,
            commands::set_common,
            commands::preview_switch,
            commands::execute_switch,
            commands::list_backups,
            commands::restore_backup,
            commands::undo_last_switch,
            commands::backup_diff,
            commands::probe_endpoint,
            commands::lock_status,
            commands::recover_stale_lock,
            commands::discover_local,
        ])
        .run(tauri::generate_context!())
        .expect("Agent Switchboard 启动失败");
}
