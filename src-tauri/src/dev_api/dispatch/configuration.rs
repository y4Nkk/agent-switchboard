use super::*;

macro_rules! command {
    ($future:expr) => { as_json($future.await) };
}

pub(super) async fn configuration(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
        "preview_client_configuration_apply" => {
            command!(commands::client_configuration_apply::preview_client_configuration_apply(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
                argument::<String>(&request.args, "expectedSourceHash")?,
                argument::<bool>(&request.args, "expectedSourceExists")?,
                argument::<String>(&request.args, "expectedSettingsHash")?,
                argument::<asb_core::adapter::ClientConfigurationChanges>(&request.args, "changes")?,
                optional_argument::<String>(&request.args, "directory")?,
            ))
        }
        "inspect_client_configuration_sources" => command!(commands::client_configuration_sources::inspect_client_configuration_sources(
            app.clone(), argument::<AppKind>(&request.args, "target")?, optional_argument::<String>(&request.args, "directory")?,
        )),
        "preview_client_configuration_reset" => {
            command!(commands::client_configuration_apply::preview_client_configuration_reset(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
                argument::<commands::client_configuration_apply::ClientConfigurationResetKind>(&request.args, "resetKind")?,
                argument::<std::collections::BTreeSet<String>>(&request.args, "selected")?,
                optional_argument::<String>(&request.args, "expectedSourceHash")?,
            ))
        }
        "commit_client_configuration_apply" => {
            command!(commands::client_configuration_apply::commit_client_configuration_apply(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
                argument::<String>(&request.args, "expectedHash")?,
                argument::<String>(&request.args, "expectedRenderedHash")?,
                argument::<String>(&request.args, "expectedSettingsHash")?,
                argument::<String>(&request.args, "expectedSettingsResultHash")?,
                argument::<bool>(&request.args, "expectedTargetExisted")?,
                argument::<asb_core::adapter::ClientConfigurationChanges>(&request.args, "changes")?,
                optional_argument::<String>(&request.args, "directory")?,
                argument::<bool>(&request.args, "confirmWrite")?,
            ))
        }
        "commit_client_configuration_reset" => {
            command!(commands::client_configuration_apply::commit_client_configuration_reset(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
                argument::<commands::client_configuration_apply::ClientConfigurationResetKind>(&request.args, "resetKind")?,
                argument::<std::collections::BTreeSet<String>>(&request.args, "selected")?,
                argument::<String>(&request.args, "expectedHash")?,
                argument::<String>(&request.args, "expectedRenderedHash")?,
                argument::<String>(&request.args, "expectedSettingsHash")?,
                argument::<bool>(&request.args, "expectedTargetExisted")?,
                argument::<bool>(&request.args, "confirmWrite")?,
            ))
        }
        _ => return Ok(None),
    };
    result.map(Some)
}

pub(super) async fn manual(app: &AppHandle, request: &InvokeRequest) -> Result<Option<Value>, CommandError> {
    let result = match request.command.as_str() {
        "preview_manual_client_configuration" => {
            command!(commands::client_configuration_manual::preview_manual_client_configuration(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
                argument::<String>(&request.args, "expectedSourceHash")?,
                argument::<String>(&request.args, "displayContent")?,
            ))
        }
        "commit_manual_client_configuration" => {
            command!(commands::client_configuration_manual::commit_manual_client_configuration(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
                argument::<String>(&request.args, "expectedSourceHash")?,
                argument::<String>(&request.args, "expectedRenderedHash")?,
                argument::<String>(&request.args, "expectedSettingsHash")?,
                argument::<bool>(&request.args, "expectedTargetExisted")?,
                argument::<String>(&request.args, "displayContent")?,
                argument::<bool>(&request.args, "confirmWrite")?,
            ))
        }
        "preview_client_configuration_repair" => {
            command!(commands::client_configuration_repair::preview_client_configuration_repair(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
                argument::<String>(&request.args, "expectedSourceHash")?,
            ))
        }
        "commit_client_configuration_repair" => {
            command!(commands::client_configuration_repair::commit_client_configuration_repair(
                app.clone(),
                argument::<AppKind>(&request.args, "target")?,
                argument::<String>(&request.args, "expectedSourceHash")?,
                argument::<String>(&request.args, "expectedRenderedHash")?,
                argument::<bool>(&request.args, "expectedTargetExisted")?,
                argument::<bool>(&request.args, "confirmWrite")?,
            ))
        }
        _ => return Ok(None),
    };
    result.map(Some)
}
