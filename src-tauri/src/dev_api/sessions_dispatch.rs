use super::http::{argument, as_json, InvokeRequest};
use crate::commands::{self, error::CommandError};
use serde_json::Value;
use tauri::AppHandle;

pub(super) async fn dispatch(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    macro_rules! command { ($future:expr) => { as_json($future.await) }; }
    let result = match request.command.as_str() {
        "claude_session_sources" => command!(commands::claude_session_sources(app.clone())),
        "pick_claude_session_source" => command!(commands::pick_claude_session_source(app.clone())),
        "forget_claude_session_source" => command!(commands::forget_claude_session_source(app.clone(), argument(&request.args, "path")?)),
        "convert_session_sources" => command!(commands::convert_session_sources(app.clone(), argument(&request.args, "ids")?)),
        "search_sessions" => command!(commands::search_sessions(app.clone(), argument(&request.args, "request")?)),
        "get_session_message_page" => command!(commands::get_session_message_page(app.clone(), argument(&request.args, "request")?)),
        "get_session_message_content" => command!(commands::get_session_message_content(app.clone(), argument(&request.args, "request")?)),
        "get_session_copy_text" => command!(commands::get_session_copy_text(app.clone(), argument(&request.args, "source")?)),
        "get_session_metadata" => command!(commands::get_session_metadata(app.clone(),
            argument(&request.args, "app")?, argument(&request.args, "sessionId")?)),
        "search_session_bookmarks" => command!(commands::search_session_bookmarks(app.clone(), argument(&request.args, "request")?)),
        "get_session_bookmark_content" => command!(commands::get_session_bookmark_content(app.clone(),
            argument(&request.args, "id")?, argument(&request.args, "offset")?)),
        "save_session_bookmark" => command!(commands::save_session_bookmark(app.clone(),
            argument(&request.args, "app")?, argument(&request.args, "sessionId")?, argument(&request.args, "messageId")?)),
        "delete_session_bookmark" => command!(commands::delete_session_bookmark(app.clone(), argument(&request.args, "id")?)),
        "update_session_organization" => command!(commands::update_session_organization(app.clone(),
            argument(&request.args, "requests")?, argument(&request.args, "change")?)),
        "export_session_markdown" => command!(commands::export_session_markdown(app.clone(),
            argument(&request.args, "app")?, argument(&request.args, "sessionId")?)),
        "resume_session" => command!(commands::resume_session(app.clone(), argument(&request.args, "app")?, argument(&request.args, "sessionId")?)),
        "delete_session" => command!(commands::delete_session(app.clone(),
            argument(&request.args, "app")?, argument(&request.args, "sessionId")?)),
        "delete_sessions" => command!(commands::delete_sessions(app.clone(), argument(&request.args, "requests")?)),
        _ => return Ok(None),
    };
    result.map(Some)
}
