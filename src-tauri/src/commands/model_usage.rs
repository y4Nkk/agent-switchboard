//! Read-only local model-token usage command.

use super::error::state;
use super::error::{blocking, CommandError};
use asb_core::contracts::{ModelUsageRead, ModelUsageRefresh};
use tauri::Manager;

/// Returns the last verified reports without waiting for session discovery.
#[tauri::command]
pub async fn read_model_usage_snapshot(
    app: tauri::AppHandle,
) -> Result<Option<ModelUsageRead>, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        crate::model_usage_cache::read_cached(&state)
            .map_err(|error| CommandError::new("model-usage-index-unavailable", error))
    })
    .await
}

/// All calendar ranges share one refresh; overlapping requests reuse its result.
#[tauri::command]
pub async fn refresh_model_usage(app: tauri::AppHandle) -> Result<ModelUsageRefresh, CommandError> {
    let state = state(&app)?;
    let manager = app.state::<crate::model_usage_cache::ModelUsageManager>().inner().clone();
    blocking(move || {
        manager.start(state.root().to_path_buf())
            .map_err(|error| CommandError::new("model-usage-index-unavailable", error))
    }).await
}

#[tauri::command]
pub async fn read_model_usage_refresh(app: tauri::AppHandle, id: String) -> Result<ModelUsageRefresh, CommandError> {
    let state = state(&app)?;
    app.state::<crate::model_usage_cache::ModelUsageManager>().read(state.root(), &id)
        .map_err(|error| CommandError::new("model-usage-index-unavailable", error))
}
