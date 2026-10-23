use super::*;

macro_rules! command {
    ($future:expr) => { as_json($future.await) };
}

pub(super) async fn windows(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
        "update_channel" => {
            as_json(Ok::<_, CommandError>(crate::distribution::update_channel()))
        }
        "pick_directory" => {
            command!(commands::window::pick_directory(app.clone()))
        }
        "pick_file" => command!(commands::window::pick_file(
            app.clone(),
            argument(&request.args, "filterName")?,
            argument(&request.args, "extensions")?,
        )),
        "config_status" => command!(commands::status::config_status(app.clone())),
        "runtime_overview" => command!(commands::status::runtime_overview(app.clone())),
        "workspace_visible" => as_json(crate::workspace_visibility::workspace_visible(app.clone())),
        "read_resource_usage" => command!(commands::resource_usage::read_resource_usage(
            app.clone(), argument(&request.args, "request")?,
        )),
        "open_config_file_location" => command!(commands::status::open_config_file_location(
            app.clone(),
            argument::<AppKind>(&request.args, "target")?,
        )),
        "open_app_data_dir" => command!(commands::status::open_app_data_dir(app.clone())),
        "lock_status" => command!(commands::status::lock_status(
            app.clone(),
            argument::<AppKind>(&request.args, "target")?,
        )),
        "recover_stale_lock" => command!(commands::status::recover_stale_lock(
            app.clone(),
            argument::<AppKind>(&request.args, "target")?,
        )),
        _ => return Ok(None),
    };
    result.map(Some)
}

pub(super) async fn gateway(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
        "preview_claude_gateway_stop" => command!(
            commands::switching::claude_gateway::preview_claude_gateway_stop(app.clone())
        ),
        "stop_claude_gateway" => {
            command!(commands::switching::claude_gateway::stop_claude_gateway(
                app.clone(),
                argument(&request.args, "preview")?,
                argument(&request.args, "confirmWrite")?
            ))
        }
        "gateway_status" => command!(commands::gateway::gateway_status(app.clone())),
        "gateway_retry_bind" => {
            command!(commands::gateway::gateway_retry_bind(app.clone()))
        }
        "gateway_prepare_port_change" => {
            command!(commands::gateway::gateway_prepare_port_change(
                app.clone(),
                argument(&request.args, "newPort")?,
            ))
        }
        "gateway_commit_port_change" => {
            command!(commands::gateway::gateway_commit_port_change(
                app.clone(),
                argument(&request.args, "preparationId")?,
                argument(&request.args, "confirmWrite")?,
            ))
        }
        "gateway_cancel_port_change" => {
            command!(commands::gateway::gateway_cancel_port_change(
                app.clone(),
                argument(&request.args, "preparationId")?,
            ))
        }
        "gateway_discard_port_change" => {
            command!(commands::gateway::gateway_discard_port_change(
                app.clone(),
                argument(&request.args, "confirmWrite")?,
            ))
        }
        _ => return Ok(None),
    };
    result.map(Some)
}

pub(super) async fn cloud(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
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
        "test_cloud_backup_connection" => {
            command!(commands::cloud_backup::test_cloud_backup_connection(
                argument::<CloudBackupSettings>(&request.args, "settings")?,
                argument(&request.args, "accountPassword")?,
            ))
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
        _ => return Ok(None),
    };
    result.map(Some)
}

pub(super) async fn switching(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
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
            optional_argument(&request.args, "authHash")?,
            optional_argument(&request.args, "authExisted")?,
            optional_argument(&request.args, "authRenderedHash")?,
        )),
        "list_backups" => command!(commands::switching::list_backups(app.clone())),
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
        _ => return Ok(None),
    };
    result.map(Some)
}

pub(super) async fn login(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
        "official_login_start" => {
            command!(commands::official_login::official_login_start(argument::<
                AppKind,
            >(
                &request.args,
                "target"
            )?,))
        }
        "official_login_poll" => {
            command!(commands::official_login::official_login_poll(app.clone(), argument::<
                AppKind,
            >(
                &request.args,
                "target"
            )?,))
        }
        "official_login_cancel" => {
            command!(commands::official_login::official_login_cancel(argument::<
                AppKind,
            >(
                &request.args,
                "target"
            )?,))
        }
        _ => return Ok(None),
    };
    result.map(Some)
}
