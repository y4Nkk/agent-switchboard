use super::http::{argument, as_json, optional_argument, InvokeRequest};
use crate::commands::{self, error::CommandError};
use crate::local_state::CloudBackupSettings;
use asb_core::contracts::{
    AppKind, ProviderDraft, SettingsValues, UsageHistoryRequest,
};
use serde_json::Value;
use tauri::AppHandle;

mod system;
mod client_software;
mod providers;
mod usage;
mod configuration;

pub(super) fn dispatch(app: &AppHandle, request: InvokeRequest) -> Result<Value, CommandError> {
    crate::application_preparation::authorize(app, &request.command)?;
    match request.command.as_str() {
        "get_application_preparation" => return as_json(crate::application_preparation::get_application_preparation(app.clone())),
        "recheck_application_data" => return as_json(crate::application_preparation::recheck_application_data(app.clone())),
        _ => {}
    }
    tauri::async_runtime::block_on(async {
        if let Some(result) = client_software::dispatch(app, &request).await? { return Ok(result); }
        if let Some(result) = super::settings_dispatch::dispatch(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = super::provider_diagnostics_dispatch::dispatch(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = super::codex_dispatch::dispatch(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = super::claude_dispatch::dispatch(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = super::extensions_dispatch::dispatch(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = super::sessions_dispatch::dispatch(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = system::windows(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = system::gateway(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = system::cloud(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = system::switching(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = system::login(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = providers::profiles(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = providers::imports(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = providers::client_settings(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = providers::prompts(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = providers::requests(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = usage::usage(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = configuration::configuration(app, &request).await? {
            return Ok(result);
        }
        if let Some(result) = configuration::manual(app, &request).await? {
            return Ok(result);
        }
        Err(CommandError::keyed(
            "web-command-unavailable",
            "errors.misc.webCommandUnavailable",
            "浏览器开发环境不支持该原生窗口命令",
        ))
    })
}
