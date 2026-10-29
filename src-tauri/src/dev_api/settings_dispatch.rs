use super::http::{argument, as_json, InvokeRequest};
use crate::commands::{self, error::CommandError, workspace};
use crate::local_state::AppSettings;
use serde_json::Value;
use tauri::AppHandle;

pub(super) async fn dispatch(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
        "get_outbound_proxy_settings" => as_json(commands::get_outbound_proxy_settings(app.clone()).await),
        "set_outbound_proxy_settings" => as_json(commands::set_outbound_proxy_settings(
            app.clone(), argument(&request.args, "settings")?, argument(&request.args, "expectedRevision")?,
        ).await),
        "repair_outbound_proxy_settings" => as_json(commands::repair_outbound_proxy_settings(app.clone()).await),
        "storage_usage" => as_json(commands::storage::storage_usage(app.clone()).await),
        "clear_webview_cache" => as_json(commands::storage::clear_webview_cache(app.clone()).await),
        "preview_history_archive" => as_json(commands::storage::preview_history_archive(app.clone(), argument(&request.args, "cutoff")?).await),
        "archive_history" => as_json(commands::storage::archive_history(app.clone(), argument(&request.args, "plan")?).await),
        "preview_backup_cleanup" => as_json(commands::storage::preview_backup_cleanup(app.clone(), argument(&request.args, "cutoff")?).await),
        "cleanup_backups" => as_json(commands::storage::cleanup_backups(app.clone(), argument(&request.args, "plan")?).await),
        "get_app_settings" => as_json(commands::get_app_settings(app.clone()).await),
        "set_app_settings" => as_json(commands::set_app_settings(
            app.clone(), argument::<AppSettings>(&request.args, "settings")?,
        ).await),
        "repair_app_settings" => as_json(commands::repair_app_settings(app.clone()).await),
        "list_system_fonts" => as_json(commands::list_system_fonts().await),
        "get_startup_page" => as_json(workspace::get_startup_page(app.clone()).await),
        "remember_workspace_page" => as_json(workspace::remember_workspace_page(
            app.clone(), argument(&request.args, "page")?,
        ).await),
        "get_sidebar_collapsed" => as_json(workspace::get_sidebar_collapsed(app.clone()).await),
        "set_sidebar_collapsed" => as_json(workspace::set_sidebar_collapsed(
            app.clone(), argument(&request.args, "collapsed")?,
        ).await),
        "set_shortcut_recording" => {
            workspace::set_shortcut_recording(app.clone(), argument(&request.args, "recording")?);
            as_json(Ok(()))
        }
        _ => return Ok(None),
    };
    result.map(Some)
}
