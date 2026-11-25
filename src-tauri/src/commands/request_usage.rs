use super::error::{blocking, state, CommandError};
use crate::request_usage::{RequestUsageRead, RequestUsageReader, RequestUsageResult};
use tauri::Manager;

#[tauri::command]
pub(crate) async fn read_request_usage(
    app: tauri::AppHandle, request: RequestUsageRead,
) -> Result<RequestUsageResult, CommandError> {
    let state = state(&app)?;
    let reader = app.state::<RequestUsageReader>().inner().clone();
    blocking(move || reader.read(state.root(), request)
        .map_err(|error| CommandError::new("request-usage-unavailable", error))).await
}
