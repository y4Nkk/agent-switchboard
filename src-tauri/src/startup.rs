use crate::{client_config_watcher, client_software, codex_probe, commands, desktop_shortcut,
    gateway, local_state::LocalState, model_usage_cache, native_telemetry, provider_request,
    resource_monitor, runtime_log, session_manager, storage, tray, update_temp_sweep, usage_query};
use tauri::Manager;

pub(crate) fn initialize_services(app: &tauri::AppHandle, local: &LocalState,
    ) -> Result<Vec<commands::error::CommandError>, String> {
    local.initialize_extension_schema()?;
    let mut issues = Vec::new();
    if let Err(error) = local.initialize_session_storage() {
        issues.push(commands::error::CommandError::new("session-storage-unavailable", error));
    }
    if let Err(error) = app.state::<client_software::SoftwareManager>().initialize(local) {
        issues.push(commands::error::CommandError::new("software-state-unavailable", error));
    }
    let write_gate = app.state::<commands::ConfigWriteGate>().inner().clone();
    {
        let _guard = write_gate.lock()?;
        app.manage(gateway::GatewayController::start_with_write_lock(local, write_gate.shared()));
        app.manage(gateway::PortChangePreparations::default());
        app.manage(commands::switching::ProfileSavePreparations::default());
        app.manage(commands::switching::CodexProfileSavePreparations::default());
        app.manage(commands::switching::CodexPolicyPreparations::default());
        app.manage(provider_request::ProviderRequests::default());
        app.manage(commands::provider_diagnostics::ProviderRepairPreparations::default());
        app.manage(codex_probe::ProbeRegistry::new());
        app.manage(resource_monitor::ResourceMonitor::default());
        app.manage(model_usage_cache::ModelUsageManager::default());
        app.manage(native_telemetry::NativeTelemetry::start(local.root().to_path_buf()));
        if let Err(error) = codex_probe::recover_interrupted(local) {
            issues.push(commands::error::CommandError::new("probe-recovery-failed", error));
        }

    }
    if let Err(error) = app.plugin(desktop_shortcut::plugin()) {
        issues.push(commands::error::CommandError::new("shortcut-service-unavailable", error.to_string()));
    }
    if let Err(error) = start_background_services(app, local) {
        issues.push(commands::error::CommandError::new("background-service-unavailable", error));
    }
    Ok(issues)
}

fn start_background_services(app: &tauri::AppHandle, local: &LocalState) -> Result<(), String> {
    use tauri::Manager;
    let watcher = client_config_watcher::ClientConfigWatcher::start(
        app.clone(),
        [
            local.target(asb_core::AppKind::Codex)?,
            local.target(asb_core::AppKind::Claude)?,
        ],
    ).map_err(|error| error.to_string())?;
    app.manage(watcher);
    if let Err(error) = tray::setup(app) {
        tray::recover_main(app, &error);
    }
    // The tray panel is a persistent surface; its usage data must
    // refresh even while the main window is hidden, closed to the
    // tray, or on another page. The scheduler thread owns that
    // cadence, so no renderer lifecycle can stop it.
    usage_query::scheduler::spawn(app.clone());
    storage::spawn(app.clone());
    session_manager::history::spawn(app, local.root().to_path_buf());
    update_temp_sweep::spawn(app.clone());
    runtime_log::record_started();
    Ok(())
}

pub(crate) fn recover_configuration(app: &tauri::AppHandle, local: &LocalState)
    -> Result<Vec<commands::error::CommandError>, String> {
    let gate = app.state::<commands::ConfigWriteGate>();
    let _guard = gate.lock()?;
    let mut errors = Vec::new();
    if let Err(error) = commands::switching::recover_pending_profile_save(app) {
        errors.push(commands::error::CommandError::new("profile-save-recovery-failed", error));
    }
    if let Err(error) = commands::switching::codex_policy::recover_on_startup(local,
        app.state::<gateway::GatewayController>().inner()) {
        errors.push(commands::error::CommandError::new("codex-policy-recovery-failed", error));
    }
    Ok(errors)
}
