//! Codex settings IPC forwarding; business logic stays in the typed command owner.
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
        "get_codex_subagent_settings" => {
            command!(commands::subagent_settings::get_codex_subagent_settings(
                app.clone(),
            ))
        }
        "query_codex_official_quota" => command!(commands::query_codex_official_quota(
            app.clone(),
            argument(&request.args, "profileId")?,
        )),
        "read_codex_official_quota" => command!(commands::read_codex_official_quota(
            app.clone(),
            argument(&request.args, "profileId")?,
        )),
        "get_cached_codex_official_reset" => {
            command!(commands::get_cached_codex_official_reset(app.clone()))
        }
        "refresh_codex_official_reset" => {
            command!(commands::refresh_codex_official_reset(app.clone()))
        }
        "get_cached_codex_reset_status" => {
            command!(commands::get_cached_codex_reset_status(app.clone()))
        }
        "check_codex_reset_status" => command!(commands::check_codex_reset_status(app.clone())),
        "get_cached_codex_reset_hub_status" => {
            command!(commands::get_cached_codex_reset_hub_status(app.clone()))
        }
        "check_codex_reset_hub_status" => command!(commands::check_codex_reset_hub_status(app.clone())),
        "codex_login_blocker" => {
            command!(commands::official_login::codex_login_blocker(app.clone()))
        }
        _ => Ok(None),
    }
}
