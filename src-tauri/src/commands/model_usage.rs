//! Read-only local model-token usage command.

use super::error::state;
use super::error::{blocking, CommandError};
use asb_core::contracts::{ModelUsageRead, ModelUsageRequest};

/// Aggregates token consumption from the fixed local Codex and Claude Code
/// session roots. The renderer supplies a typed range plus an explicit cache
/// policy; no profile, provider credential, session path, or raw session
/// record crosses this API.
#[tauri::command]
pub async fn get_model_usage_report(
    app: tauri::AppHandle,
    request: ModelUsageRequest,
) -> Result<ModelUsageRead, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        crate::model_usage_cache::get_or_refresh(&state, request)
            .map_err(|error| CommandError::new("model-usage-cache-unavailable", error))
    })
    .await
}
