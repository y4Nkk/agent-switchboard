//! Renderer-safe historical usage commands.

use super::error::{blocking, state, CommandError};
use asb_core::contracts::{UsageHistoryRequest, UsageHistorySeries};

/// Returns one current provider's query-matched history or the independent
/// Codex official-account trend. The renderer cannot submit an endpoint,
/// account marker, query digest, or history file path.
#[tauri::command]
pub async fn get_usage_history(
    app: tauri::AppHandle,
    request: UsageHistoryRequest,
) -> Result<Vec<UsageHistorySeries>, CommandError> {
    let state = state(&app)?;
    blocking(move || match request {
        UsageHistoryRequest::Provider { profile_id } => {
            let profile = state
                .configuration()
                .find_provider(&profile_id)
                .map_err(|error| CommandError::new("profile-not-found", error))?;
            crate::usage_history::provider_series(&state, &profile)
                .map_err(|error| CommandError::new("usage-history-unavailable", error))
        }
        UsageHistoryRequest::Official => crate::usage_history::official_series(&state)
            .map_err(|error| CommandError::new("usage-history-unavailable", error)),
    })
    .await
}
