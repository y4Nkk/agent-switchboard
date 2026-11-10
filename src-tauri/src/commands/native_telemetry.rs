use super::{client_configuration_apply::{self, ClientConfigurationApplyPreview},
    error::{blocking, require_write_confirmation, state, store_error, CommandError}, ConfigWriteGate};
use crate::native_telemetry::{configuration, NativeTelemetry, ReceiverStatus};
use asb_core::contracts::{AppKind, SettingsValues};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TelemetrySetup {
    receiver: ReceiverStatus,
    clients: Vec<configuration::ClientExportStatus>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TelemetryCommit {
    target: AppKind,
    enabled: bool,
    expected_hash: String,
    expected_rendered_hash: String,
    expected_settings_hash: String,
    expected_target_existed: bool,
}

struct Candidate {
    current: String,
    existed: bool,
    rendered: String,
    settings: SettingsValues,
    settings_hash: String,
}

fn candidate(state: &crate::local_state::LocalState, app: AppKind, enabled: bool)
    -> Result<Candidate, CommandError>
{
    let (current, existed) = configuration::read(state, app).map_err(unavailable)?;
    let stored = state.configuration().get_client_settings(app).map_err(store_error)?;
    let mut settings = stored.settings;
    let rendered = configuration::render(app, &current, enabled, &mut settings).map_err(unavailable)?;
    Ok(Candidate { current, existed, rendered, settings, settings_hash: stored.settings_hash })
}

fn unavailable(error: impl std::fmt::Display) -> CommandError {
    CommandError::new("native-telemetry-unavailable", error.to_string())
}

fn require_listener(app: &AppHandle, enabled: bool) -> Result<(), CommandError> {
    if enabled && !app.state::<NativeTelemetry>().status().map_err(unavailable)?.listening {
        return Err(unavailable("本机性能接收器未监听，请先处理监听错误并重试"));
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn read_native_telemetry(app: AppHandle) -> Result<TelemetrySetup, CommandError> {
    let state = state(&app)?;
    let receiver = app.state::<NativeTelemetry>().status().map_err(unavailable)?;
    blocking(move || Ok(TelemetrySetup { receiver, clients: [AppKind::Codex, AppKind::Claude]
        .into_iter().map(|target| configuration::inspect(&state, target)).collect() })).await
}

#[tauri::command]
pub(crate) async fn retry_native_telemetry(app: AppHandle) -> Result<(), CommandError> {
    blocking(move || app.state::<NativeTelemetry>().retry().map_err(unavailable)).await
}

#[tauri::command]
pub(crate) async fn preview_native_telemetry(
    app: AppHandle, target: AppKind, enabled: bool,
) -> Result<ClientConfigurationApplyPreview, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        require_listener(&app, enabled)?;
        let candidate = candidate(&state, target, enabled)?;
        let path = state.target(target).map_err(unavailable)?;
        let file = asb_switch::preview_rendered(target, &path, &state.backup_dir(),
            &candidate.current, &candidate.rendered, asb_core::adapter::PreviewDiff::Full)
            .map_err(CommandError::from)?;
        Ok(ClientConfigurationApplyPreview { file, settings_hash: candidate.settings_hash,
            target_existed: candidate.existed })
    }).await
}

#[tauri::command]
pub(crate) async fn commit_native_telemetry(
    app: AppHandle, request: TelemetryCommit, confirm_write: bool,
) -> Result<(), CommandError> {
    require_write_confirmation(confirm_write, "应用本机 OpenTelemetry 导出配置")?;
    let state = state(&app)?;
    let gate = app.state::<ConfigWriteGate>().inner().clone();
    blocking(move || {
        let _guard = gate.lock().map_err(unavailable)?;
        super::switching::ensure_profile_save_recovered(&app)?;
        require_listener(&app, request.enabled)?;
        let candidate = candidate(&state, request.target, request.enabled)?;
        client_configuration_apply::commit_rendered_client_configuration(
            &state, app.state::<crate::gateway::GatewayController>().inner(), request.target,
            &candidate.current, candidate.existed, candidate.settings, &candidate.rendered,
            &request.expected_hash, &request.expected_rendered_hash,
            &request.expected_settings_hash, request.expected_target_existed,
            "native-opentelemetry-configuration",
        )
    }).await
}
