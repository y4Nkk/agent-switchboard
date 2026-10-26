use super::*;

macro_rules! command {
    ($future:expr) => { as_json($future.await) };
}

pub(super) async fn profiles(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
        "cancel_profile_save" => command!(commands::switching::cancel_profile_save(
            app.clone(), argument(&request.args, "source")?, argument(&request.args, "preparationId")?,
        )),
        "list_profiles" => command!(commands::list_profiles(app.clone())),
        "repair_application_data" => command!(commands::repair_application_data(app.clone())),
        "prepare_profile_save" => command!(commands::switching::prepare_profile_save(
            app.clone(),
            argument::<Option<String>>(&request.args, "profileId")?,
            argument::<ProviderDraft>(&request.args, "draft")?,
            argument::<Option<String>>(&request.args, "expectedFileHash")?,
            argument(&request.args, "intent")?,
        )),
        "commit_profile_save" => command!(commands::switching::commit_profile_save(
            app.clone(),
            argument::<String>(&request.args, "preparationId")?,
            argument::<bool>(&request.args, "confirmWrite")?,
        )),
        "delete_profile" => command!(commands::delete_profile(
            app.clone(),
            argument(&request.args, "profileId")?,
            argument::<String>(&request.args, "expectedFileHash")?,
        )),
        "reorder_claude_profiles" => command!(commands::reorder_claude_profiles(
            app.clone(),
            argument(&request.args, "orderedIds")?,
            argument(&request.args, "expectedFileHashes")?,
        )),
        "import_discovered_claude_profile" => {
            command!(commands::import_discovered_claude_profile(app.clone()))
        }
        "discover_local" => command!(commands::discover_local(app.clone())),
        "discover_cached" => command!(commands::discover_cached(app.clone())),
        _ => return Ok(None),
    };
    result.map(Some)
}

pub(super) async fn imports(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
        "scan_ccswitch" => command!(commands::scan_ccswitch(
            app.clone(),
            argument::<Option<String>>(&request.args, "dbDirectory")?,
        )),
        "import_ccswitch_claude_profiles" => {
            command!(commands::import_ccswitch_claude_profiles(
                app.clone(),
                argument(&request.args, "keys")?,
                argument::<Option<String>>(&request.args, "dbDirectory")?,
            ))
        }
        "export_providers_sql" => command!(commands::export_providers_sql(
            app.clone(),
            argument(&request.args, "targetPath")?,
        )),
        "apply_providers_sql" => command!(commands::apply_providers_sql(
            app.clone(),
            argument(&request.args, "sqlPath")?,
        )),
        "import_providers_sql" => command!(commands::import_providers_sql(
            app.clone(),
            argument(&request.args, "ids")?,
            argument(&request.args, "sqlPath")?,
        )),
        _ => return Ok(None),
    };
    result.map(Some)
}

pub(super) async fn client_settings(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
        "get_provider_parameters_catalog" => as_json(Ok::<_, CommandError>(
            commands::client_settings::get_provider_parameters_catalog(argument::<AppKind>(
                &request.args,
                "target",
            )?),
        )),
        "get_client_settings_editor" => {
            command!(commands::client_settings::get_client_settings_editor(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
            ))
        }
        "save_client_settings" => command!(commands::client_settings::save_client_settings(
            app.clone(),
            argument::<AppKind>(&request.args, "target")?,
            argument::<SettingsValues>(&request.args, "settings")?,
            argument::<String>(&request.args, "expectedSettingsHash")?,
        )),
        "get_current_client_configuration" => {
            command!(commands::client_settings::get_current_client_configuration(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
            ))
        }
        "parse_claude_extra_configuration" => {
            command!(commands::client_settings::parse_claude_extra_configuration(
                argument::<String>(&request.args, "content")?,
            ))
        }
        _ => return Ok(None),
    };
    result.map(Some)
}

pub(super) async fn prompts(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
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
        _ => return Ok(None),
    };
    result.map(Some)
}

pub(super) async fn requests(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
        "probe_endpoint" => {
            command!(commands::probe_endpoint(argument(&request.args, "url",)?))
        }
        "resolve_provider_endpoints" => as_json(commands::resolve_provider_endpoints(
            argument(&request.args, "request")?,
        )),
        "fetch_provider_models" => command!(commands::fetch_provider_models(
            app.clone(),
            argument(&request.args, "request",)?
        )),
        "prepare_provider_request" => {
            command!(commands::provider_request::prepare_provider_request(
                app.clone(),
                argument(&request.args, "target")?,
            ))
        }
        "fetch_provider_request_models" => {
            command!(commands::provider_request::fetch_provider_request_models(
                app.clone(),
                argument(&request.args, "requestId")?,
            ))
        }
        "execute_provider_request" => {
            command!(commands::provider_request::execute_provider_request(
                app.clone(),
                argument(&request.args, "request")?,
            ))
        }
        "cancel_provider_request" => {
            command!(commands::provider_request::cancel_provider_request(
                app.clone(),
                argument(&request.args, "requestId")?,
            ))
        }
        _ => return Ok(None),
    };
    result.map(Some)
}
