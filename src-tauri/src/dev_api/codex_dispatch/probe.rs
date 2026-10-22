//! Both transports call the same probe commands and share one registry.
use super::super::http::{argument, as_json, InvokeRequest};
use crate::commands::{codex_probe, error::CommandError};
use serde_json::Value;
use tauri::AppHandle;

pub(super) async fn dispatch(
    app: &AppHandle,
    request: &InvokeRequest,
) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
        "get_codex_probe_catalog" => as_json(codex_probe::get_codex_probe_catalog().await),
        "start_codex_probe" => as_json(codex_probe::start_codex_probe(
            app.clone(), argument(&request.args, "request")?,
        ).await),
        "get_current_codex_probe" => as_json(
            codex_probe::get_current_codex_probe(app.clone()).await,
        ),
        "cancel_codex_probe" => as_json(codex_probe::cancel_codex_probe(app.clone()).await),
        "retry_codex_probe_save" => as_json(
            codex_probe::retry_codex_probe_save(app.clone()).await,
        ),
        "list_codex_probe_history" => as_json(codex_probe::list_codex_probe_history(
            app.clone(), argument(&request.args, "request")?,
        ).await),
        "list_codex_probe_history_profiles" => as_json(
            codex_probe::list_codex_probe_history_profiles(app.clone()).await,
        ),
        "get_codex_probe_batch" => as_json(codex_probe::get_codex_probe_batch(
            app.clone(), argument(&request.args, "batchId")?,
        ).await),
        "delete_codex_probe_batches" => as_json(codex_probe::delete_codex_probe_batches(
            app.clone(), argument(&request.args, "request")?,
        ).await),
        _ => return Ok(None),
    };
    result.map(Some)
}
