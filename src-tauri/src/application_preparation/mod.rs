//! One native owner of application-data preparation and its observable result.
mod runtime;
pub(crate) use runtime::start;
mod state;
pub(crate) use state::{Preparation, Snapshot, Status};
use crate::{commands::error::CommandError, local_state::{AppSettings, LocalState, SettingsPreparation}};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Stage { Preferences, Configuration, OutboundProxy, Services, Complete }

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Report {
    pub settings: Option<AppSettings>,
    pub configuration_ready: bool,
    pub preferences_reset: bool,
    pub repaired_files: Vec<String>,
    pub backup_paths: Vec<String>,
    pub issues: Vec<CommandError>,
    #[serde(skip)]
    pub observed_revision: String,
}

pub(crate) fn check_data(local: &LocalState, preferences: Option<Result<SettingsPreparation, String>>,
    mut progress: impl FnMut(Stage, &Report)) -> Report {
    let mut observed = input_revision(local);
    let mut report = check_domains(local, preferences, &mut progress);
    if !report.repaired_files.is_empty() {
        observed = input_revision(local);
        let mut verified = check_domains(local, None, |_, _| {});
        verified.repaired_files.extend(report.repaired_files);
        verified.repaired_files.sort(); verified.repaired_files.dedup();
        verified.backup_paths.extend(report.backup_paths);
        verified.preferences_reset |= report.preferences_reset;
        report = verified;
    }
    if input_revision(local) != observed {
        report.configuration_ready = false;
        report.issues.push(problem("application-data-changed", "检查期间数据发生变化，原件已保留，正在重新检查".into()));
    }
    report.observed_revision = observed;
    report
}

fn check_domains(local: &LocalState, preferences: Option<Result<SettingsPreparation, String>>,
    mut progress: impl FnMut(Stage, &Report)) -> Report {
    let mut report = Report::default();
    progress(Stage::Preferences, &report);
    match preferences.unwrap_or_else(|| local.prepare_app_settings()) {
        Ok(prepared) => {
            report.settings = Some(prepared.settings);
            if let Some(path) = prepared.backup_path {
                report.repaired_files.push("settings.json".into());
                report.backup_paths.push(path);
            }
            report.preferences_reset = prepared.reset;
            if prepared.reset { log::warn!("损坏的界面偏好已备份并恢复默认值"); }
        }
        Err(error) => report.issues.push(problem("app-settings-unavailable", error)),
    }
    progress(Stage::Configuration, &report);
    match prepare_configuration(local) {
        Ok((files, backup)) => {
            report.configuration_ready = true;
            report.repaired_files.extend(files.into_iter().map(|path| format!("configuration/{path}")));
            report.backup_paths.extend(backup);
        }
        Err(error) => report.issues.push(problem("profile-store-repair-failed", error)),
    }
    progress(Stage::OutboundProxy, &report);
    if let Err(error) = crate::outbound_proxy::load(local.root()) {
        report.issues.push(crate::commands::outbound_proxy_settings::settings_error(error));
    }
    report
}

fn prepare_configuration(local: &LocalState) -> Result<(Vec<String>, Option<String>), String> {
    let store = local.configuration();
    let recovered = store.recover_pending_repair()?;
    local.initialize_configuration_store()?;
    if let Some(repaired) = recovered { return Ok((repaired.repaired_files, repaired.backup_path)); }
    if let Ok(snapshot) = crate::config_store::snapshot::read_configuration_snapshot(&store) {
        if crate::config_store::snapshot::validate_snapshot(&snapshot).is_ok() { return Ok((Vec::new(), None)); }
    }
    let repaired = store.repair()?;
    Ok((repaired.repaired_files, repaired.backup_path))
}

fn problem(code: &'static str, detail: String) -> CommandError {
    CommandError::localized(code, "errors.preparation.data", detail.clone(), serde_json::json!({ "detail": detail }))
}

pub(crate) fn input_revision(local: &LocalState) -> String {
    let mut parts = vec![local.configuration().data_revision().unwrap_or_else(|error| format!("error:{error}"))];
    for name in ["settings.json", "settings-recovery.json", "outbound-proxy.json", "profiles.json", "configuration-upgrade.json"] {
        let part = match crate::application_data_io::read(&local.root().join(name)) {
            Ok(Some(bytes)) => crate::application_data_io::digest(&bytes),
            Ok(None) => "missing".into(),
            Err(error) => format!("error:{error}"),
        };
        parts.push(part);
    }
    parts.join(":")
}

pub(super) fn needs_recovery(local: &LocalState) -> bool {
    if local.get_app_settings().is_err() { return true; }
    let store = local.configuration();
    match crate::config_store::snapshot::read_configuration_snapshot(&store) {
        Ok(snapshot) => crate::config_store::snapshot::validate_snapshot(&snapshot).is_err(),
        Err(_) => true,
    }
}

#[tauri::command]
pub(crate) fn get_application_preparation(app: tauri::AppHandle) -> Result<Snapshot, CommandError> {
    use tauri::Manager;
    app.state::<Preparation>().snapshot()
}

#[tauri::command]
pub(crate) fn recheck_application_data(app: tauri::AppHandle) -> Result<(), CommandError> {
    use tauri::Manager;
    app.state::<Preparation>().request_check()
}

#[cfg(test)]
pub(crate) mod tests;

pub(crate) fn authorize(app: &tauri::AppHandle, command: &str) -> Result<(), CommandError> {
    use tauri::Manager;
    if matches!(command, "get_application_preparation" | "recheck_application_data"
        | "window_minimize" | "window_toggle_maximize" | "window_is_maximized" | "window_close"
        | "tray_ready" | "tray_resize" | "tray_hide" | "tray_quit"
        | "open_app_data_dir" | "open_runtime_log_dir" | "list_runtime_logs" | "toggle_devtools") {
        return Ok(());
    }
    app.try_state::<Preparation>().ok_or_else(|| CommandError::keyed(
        "application-preparing", "errors.preparation.running", "应用数据正在准备，请稍后重试"))?.require_ready()
}
