use super::error::{blocking, observe, state, CommandError};
use crate::outbound_proxy::{self, OutboundProxyDraft, OutboundProxySettings};
use crate::runtime_log::RuntimeLogAction;

pub(crate) fn settings_error(error: outbound_proxy::OutboundProxyError) -> CommandError {
    let (key, params) = error.parts();
    let code = if error.is_io() { "outbound-proxy-unavailable" } else { "outbound-proxy-invalid" };
    CommandError::localized(code, key, error.to_string(), params)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboundProxyView {
    settings: Option<OutboundProxySettings>,
    revision: String,
    issue: Option<CommandError>,
}

impl From<outbound_proxy::SettingsView> for OutboundProxyView {
    fn from(view: outbound_proxy::SettingsView) -> Self {
        Self { settings: view.settings, revision: view.revision, issue: view.issue.map(settings_error) }
    }
}

#[tauri::command]
pub async fn get_outbound_proxy_settings(app: tauri::AppHandle) -> Result<OutboundProxyView, CommandError> {
    let state = state(&app)?;
    blocking(move || outbound_proxy::view(state.root()).map(Into::into).map_err(settings_error)).await
}

#[tauri::command]
pub async fn set_outbound_proxy_settings(app: tauri::AppHandle, settings: OutboundProxyDraft,
    expected_revision: String) -> Result<OutboundProxyView, CommandError> {
    let local = state(&app)?;
    observe(RuntimeLogAction::OutboundProxySaved, async move {
        blocking(move || outbound_proxy::save(local.root(), settings, &expected_revision)
            .map(Into::into).map_err(settings_error)).await
    }).await
}
