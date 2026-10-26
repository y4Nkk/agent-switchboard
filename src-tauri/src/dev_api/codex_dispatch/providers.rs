//! Codex providers IPC forwarding; business logic stays in the typed command owner.
use super::super::http::{argument, as_json, InvokeRequest};
use crate::commands::{self, error::CommandError};
use serde_json::Value;
use tauri::AppHandle;

macro_rules! command {
    ($future:expr) => {
        as_json($future.await).map(Some)
    };
}

pub(super) async fn dispatch(
    app: &AppHandle,
    request: &InvokeRequest,
) -> Result<Option<Value>, CommandError> {
    match request.command.as_str() {
        "list_codex_profiles" => command!(commands::list_codex_profiles(app.clone())),
        "create_codex_profile" => command!(commands::create_codex_profile(
            app.clone(),
            argument(&request.args, "draft")?,
        )),
        "delete_codex_profile" => command!(commands::delete_codex_profile(
            app.clone(),
            argument(&request.args, "profileId")?,
            argument(&request.args, "expectedFileHash")?,
        )),
        "reorder_codex_profiles" => command!(commands::reorder_codex_profiles(
            app.clone(),
            argument(&request.args, "orderedIds")?,
            argument(&request.args, "expectedFileHashes")?,
        )),
        "prepare_codex_profile_save" => command!(commands::switching::prepare_codex_profile_save(
            app.clone(),
            argument(&request.args, "profileId")?,
            argument(&request.args, "draft")?,
            argument(&request.args, "expectedFileHash")?,
            argument(&request.args, "intent")?,
        )),
        "commit_codex_profile_save" => command!(commands::switching::commit_codex_profile_save(
            app.clone(),
            argument(&request.args, "preparationId")?,
            argument(&request.args, "confirmWrite")?,
        )),
        "import_discovered_codex_profile" => command!(commands::import_discovered_codex_profile(app.clone())),
        _ => Ok(None),
    }
}
