use super::*;

macro_rules! command {
    ($future:expr) => { as_json($future.await) };
}

pub(super) async fn usage(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
        "test_usage_query" => command!(commands::test_usage_query(argument(
            &request.args,
            "request",
        )?)),
        "query_profile_usage" => command!(commands::query_profile_usage(
            app.clone(),
            argument(&request.args, "target")?,
            argument(&request.args, "profileId")?,
        )),
        "read_profile_usage" => command!(commands::read_profile_usage(
            app.clone(),
            argument(&request.args, "target")?,
            argument(&request.args, "profileId")?,
        )),
        "read_model_usage_snapshot" => command!(commands::model_usage::read_model_usage_snapshot(app.clone())),
        "refresh_model_usage" => command!(commands::model_usage::refresh_model_usage(app.clone())),
        "read_model_usage_refresh" => command!(commands::model_usage::read_model_usage_refresh(app.clone(), argument(&request.args, "id")?)),
        "read_request_usage" => command!(commands::request_usage::read_request_usage(
            app.clone(), argument(&request.args, "query")?)),
        "read_native_telemetry" => command!(commands::native_telemetry::read_native_telemetry(app.clone())),
        "retry_native_telemetry" => command!(commands::native_telemetry::retry_native_telemetry(app.clone())),
        "preview_native_telemetry" => command!(commands::native_telemetry::preview_native_telemetry(
            app.clone(), argument(&request.args, "target")?, argument(&request.args, "enabled")?)),
        "commit_native_telemetry" => command!(commands::native_telemetry::commit_native_telemetry(
            app.clone(), argument(&request.args, "request")?, argument(&request.args, "confirmWrite")?)),
        "get_usage_history" => command!(commands::usage_history::get_usage_history(
            app.clone(),
            argument::<UsageHistoryRequest>(&request.args, "request")?,
        )),
        "list_runtime_logs" => command!(commands::runtime_log::list_runtime_logs(app.clone(),
            optional_argument(&request.args, "level")?)),
        "open_runtime_log_dir" => {
            command!(commands::runtime_log::open_runtime_log_dir(app.clone()))
        }
        _ => return Ok(None),
    };
    result.map(Some)
}
