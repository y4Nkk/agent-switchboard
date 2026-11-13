use super::error::{state, CommandError};
use asb_core::provider_conversion::{ProviderConversion, ProviderConversionRequest};
use tauri::Manager;

#[tauri::command]
pub async fn prepare_provider_conversion(app: tauri::AppHandle, request: ProviderConversionRequest) -> Result<ProviderConversion, CommandError> {
    let local = state(&app)?;
    let gate = app.state::<super::ConfigWriteGate>().inner().clone();
    gate.run(move || crate::provider_conversion::prepare(&local, request)).await
}
