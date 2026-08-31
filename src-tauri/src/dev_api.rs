//! Browser-development bridge for the real local application backend.
//!
//! This module exists only in debug builds. The browser never gets a second
//! implementation of configuration behavior: every request dispatches to the
//! same typed Tauri command used by the desktop shell.

use crate::commands::{self, error::CommandError};
use crate::local_state::{AppSettings, CloudBackupSettings};
use asb_core::contracts::{AppKind, CommonConfigPatch, ProviderDraft, UsageQuery};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::thread;
use tauri::AppHandle;
use tiny_http::{Header, Method, Response, Server, StatusCode};

pub(crate) const DEV_API_ADDRESS: &str = "127.0.0.1:1422";
pub(crate) const DEV_ORIGIN: &str = "http://localhost:1420";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvokeRequest {
    command: String,
    #[serde(default = "empty_object")]
    args: Value,
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum InvokeResponse {
    Success { result: Value },
    Failure { error: CommandError },
}

/// Binds the loopback-only RPC bridge before the browser is opened.
pub(crate) fn start(app: AppHandle) -> Result<(), String> {
    let server =
        Server::http(DEV_API_ADDRESS).map_err(|error| format!("无法启动本机开发后端：{error}"))?;
    thread::Builder::new()
        .name("asb-web-dev-api".to_string())
        .spawn(move || serve(server, app))
        .map_err(|error| format!("无法运行本机开发后端：{error}"))?;
    Ok(())
}

fn serve(server: Server, app: AppHandle) {
    for mut request in server.incoming_requests() {
        let response = handle_request(&mut request, &app);
        let _ = request.respond(response);
    }
}

fn handle_request(
    request: &mut tiny_http::Request,
    app: &AppHandle,
) -> Response<std::io::Cursor<Vec<u8>>> {
    if request.method() != &Method::Post || request.url() != "/invoke" {
        return error_response(404, "web-command-not-found", "开发后端不存在该接口");
    }
    if !request
        .headers()
        .iter()
        .any(|header| header.field.equiv("Origin") && header.value.as_str() == DEV_ORIGIN)
    {
        return error_response(
            403,
            "web-origin-rejected",
            "开发后端只接受本机 Vite 页面请求",
        );
    }
    if !request.headers().iter().any(|header| {
        header.field.equiv("Content-Type") && header.value.as_str().starts_with("application/json")
    }) {
        return error_response(415, "web-content-type-invalid", "开发后端请求必须使用 JSON");
    }

    let mut body = String::new();
    if request.as_reader().read_to_string(&mut body).is_err() {
        return error_response(400, "web-request-unreadable", "无法读取开发后端请求");
    }
    let request = match serde_json::from_str::<InvokeRequest>(&body) {
        Ok(request) => request,
        Err(_) => return error_response(400, "web-request-invalid", "开发后端请求格式无效"),
    };
    match dispatch(app, request) {
        Ok(result) => json_response(200, &InvokeResponse::Success { result }),
        Err(error) => json_response(200, &InvokeResponse::Failure { error }),
    }
}

fn error_response(
    status: u16,
    code: &'static str,
    message: &'static str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(
        status,
        &InvokeResponse::Failure {
            error: CommandError::new(code, message),
        },
    )
}

fn json_response<T: Serialize>(status: u16, value: &T) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_string(value).unwrap_or_else(|_| {
        "{\"kind\":\"failure\",\"error\":{\"code\":\"web-response-invalid\",\"message\":\"开发后端响应无法序列化\"}}".to_string()
    });
    Response::from_string(body)
        .with_status_code(StatusCode(status))
        .with_header(
            Header::from_bytes("Content-Type", "application/json; charset=utf-8")
                .expect("static development response header is valid"),
        )
}

fn argument<T: DeserializeOwned>(args: &Value, name: &str) -> Result<T, CommandError> {
    let value = args
        .get(name)
        .cloned()
        .ok_or_else(|| CommandError::new("web-argument-missing", format!("缺少参数：{name}")))?;
    serde_json::from_value(value)
        .map_err(|_| CommandError::new("web-argument-invalid", format!("参数无效：{name}")))
}

fn as_json<T: Serialize>(result: Result<T, CommandError>) -> Result<Value, CommandError> {
    result.and_then(|value| {
        serde_json::to_value(value)
            .map_err(|_| CommandError::new("web-response-invalid", "开发后端响应无法序列化"))
    })
}

fn dispatch(app: &AppHandle, request: InvokeRequest) -> Result<Value, CommandError> {
    tauri::async_runtime::block_on(async {
        macro_rules! command {
            ($future:expr) => {
                as_json($future.await)
            };
        }

        match request.command.as_str() {
            "config_status" => command!(commands::status::config_status(app.clone())),
            "list_profiles" => command!(commands::list_profiles(app.clone())),
            "reset_profile_store" => command!(commands::reset_profile_store(
                app.clone(),
                argument(&request.args, "confirmWrite")?,
            )),
            "create_profile" => command!(commands::create_profile(
                app.clone(),
                argument::<ProviderDraft>(&request.args, "draft")?,
            )),
            "update_profile" => command!(commands::update_profile(
                app.clone(),
                argument(&request.args, "profileId")?,
                argument::<ProviderDraft>(&request.args, "draft")?,
            )),
            "delete_profile" => command!(commands::delete_profile(
                app.clone(),
                argument(&request.args, "profileId")?,
            )),
            "reorder_profiles" => command!(commands::reorder_profiles(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
                argument(&request.args, "orderedIds")?,
            )),
            "import_discovered_profile" => command!(commands::import_discovered_profile(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
            )),
            "scan_ccswitch" => command!(commands::scan_ccswitch(app.clone())),
            "import_ccswitch_profiles" => command!(commands::import_ccswitch_profiles(
                app.clone(),
                argument(&request.args, "keys")?,
            )),
            "get_common" => command!(commands::common_settings::get_common(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
            )),
            "set_common" => command!(commands::common_settings::set_common(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
                argument::<CommonConfigPatch>(&request.args, "patch")?,
            )),
            "common_toggles" => command!(commands::common_settings::common_toggles(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
            )),
            "common_choices" => command!(commands::common_settings::common_choices(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
            )),
            "preview_common" => command!(commands::common_settings::preview_common(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
            )),
            "apply_common" => command!(commands::common_settings::apply_common(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
                argument::<CommonConfigPatch>(&request.args, "patch")?,
                argument(&request.args, "confirmWrite")?,
            )),
            "get_global_prompt_document" => {
                command!(commands::prompt_management::get_global_prompt_document(
                    app.clone(),
                    argument::<AppKind>(&request.args, "target")?,
                ))
            }
            "save_global_prompt_document" => {
                command!(commands::prompt_management::save_global_prompt_document(
                    app.clone(),
                    argument::<AppKind>(&request.args, "target")?,
                    argument(&request.args, "content")?,
                    argument(&request.args, "expectedHash")?,
                    argument(&request.args, "confirmWrite")?,
                ))
            }
            "get_app_settings" => command!(commands::get_app_settings(app.clone())),
            "set_app_settings" => command!(commands::set_app_settings(
                app.clone(),
                argument::<AppSettings>(&request.args, "settings")?,
            )),
            "get_cloud_backup_settings" => {
                command!(commands::cloud_backup::get_cloud_backup_settings(
                    app.clone()
                ))
            }
            "set_cloud_backup_settings" => {
                command!(commands::cloud_backup::set_cloud_backup_settings(
                    app.clone(),
                    argument::<CloudBackupSettings>(&request.args, "settings")?,
                ))
            }
            "cloud_backup_setup_sql" => {
                as_json(Ok(commands::cloud_backup::cloud_backup_setup_sql()))
            }
            "upload_cloud_backup" => command!(commands::cloud_backup::upload_cloud_backup(
                app.clone(),
                argument(&request.args, "accountPassword")?,
                argument(&request.args, "backupPassword")?,
                argument(&request.args, "confirmWrite")?,
            )),
            "restore_cloud_backup" => command!(commands::cloud_backup::restore_cloud_backup(
                app.clone(),
                argument(&request.args, "accountPassword")?,
                argument(&request.args, "backupPassword")?,
                argument(&request.args, "confirmWrite")?,
            )),
            "list_system_fonts" => command!(commands::list_system_fonts()),
            "preview_switch" => command!(commands::switching::preview_switch(
                app.clone(),
                argument(&request.args, "profileId")?,
            )),
            "execute_switch" => command!(commands::switching::execute_switch(
                app.clone(),
                argument(&request.args, "profileId")?,
                argument(&request.args, "expectedHash")?,
                argument(&request.args, "expectedRenderedHash")?,
                argument(&request.args, "confirmWrite")?,
            )),
            "list_backups" => command!(commands::switching::list_backups(app.clone())),
            "list_runtime_logs" => command!(commands::runtime_log::list_runtime_logs(app.clone())),
            "open_runtime_log_dir" => {
                command!(commands::runtime_log::open_runtime_log_dir(app.clone()))
            }
            "restore_backup" => command!(commands::switching::restore_backup(
                app.clone(),
                argument(&request.args, "backupId")?,
                argument(&request.args, "confirmWrite")?,
            )),
            "undo_last_switch" => command!(commands::switching::undo_last_switch(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
                argument(&request.args, "confirmWrite")?,
            )),
            "backup_diff" => command!(commands::switching::backup_diff(
                app.clone(),
                argument(&request.args, "backupId")?,
            )),
            "open_backup_dir" => command!(commands::switching::open_backup_dir(app.clone())),
            "probe_endpoint" => {
                command!(commands::probe_endpoint(argument(&request.args, "url",)?))
            }
            "fetch_provider_models" => command!(commands::fetch_provider_models(
                argument(&request.args, "url")?,
                argument(&request.args, "apiKey")?,
            )),
            "test_usage_query" => command!(commands::test_usage_query(
                argument::<UsageQuery>(&request.args, "query")?,
                argument(&request.args, "apiKey")?,
                argument(&request.args, "baseUrl")?,
            )),
            "check_update" => command!(commands::check_update(app.clone())),
            "get_cached_codex_reset_status" => {
                command!(commands::get_cached_codex_reset_status(app.clone()))
            }
            "check_codex_reset_status" => command!(commands::check_codex_reset_status(app.clone())),
            "lock_status" => command!(commands::status::lock_status(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
            )),
            "recover_stale_lock" => command!(commands::status::recover_stale_lock(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
            )),
            "discover_local" => command!(commands::discover_local()),
            "list_sessions" => command!(commands::list_sessions()),
            "get_session_messages" => command!(commands::get_session_messages(
                argument::<AppKind>(&request.args, "app")?,
                argument(&request.args, "sessionId")?,
            )),
            "resume_session" => command!(commands::resume_session(
                argument::<AppKind>(&request.args, "app")?,
                argument(&request.args, "sessionId")?,
            )),
            _ => Err(CommandError::new(
                "web-command-unavailable",
                "浏览器开发环境不支持该原生窗口命令",
            )),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_require_a_command_and_object_arguments() {
        assert!(serde_json::from_str::<InvokeRequest>(r#"{"command":"config_status"}"#).is_ok());
        assert!(serde_json::from_str::<InvokeRequest>(r#"{"command":7}"#).is_err());
        assert!(serde_json::from_str::<InvokeRequest>(
            r#"{"command":"config_status","extra":true}"#
        )
        .is_err());
    }

    #[test]
    fn argument_errors_are_typed_and_do_not_accept_wrong_shapes() {
        let args = serde_json::json!({ "target": "codex" });
        assert_eq!(
            argument::<AppKind>(&args, "target").unwrap(),
            AppKind::Codex
        );
        assert_eq!(
            argument::<String>(&args, "missing").unwrap_err().code,
            "web-argument-missing"
        );
        assert_eq!(
            argument::<bool>(&args, "target").unwrap_err().code,
            "web-argument-invalid"
        );
    }
}
