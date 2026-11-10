use super::error::{blocking, state, CommandError};
use crate::request_usage::{RequestUsageQuery, RequestUsageReport};

#[tauri::command]
pub(crate) async fn read_request_usage(
    app: tauri::AppHandle, query: RequestUsageQuery,
) -> Result<RequestUsageReport, CommandError> {
    let state = state(&app)?;
    blocking(move || crate::request_usage::read(state.root(), query)
        .map_err(|error| CommandError::new("request-usage-unavailable", error))).await
}
