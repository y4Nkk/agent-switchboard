//! Extension browser-development IPC uses the same commands as the desktop app.
use super::http::{argument, as_json, InvokeRequest};
use crate::commands::{self, error::CommandError};
use asb_core::contracts::AppKind;
use asb_core::extensions::contracts::ExtensionTarget;
use serde_json::Value;
use tauri::AppHandle;

pub(super) async fn dispatch(
    app: &AppHandle,
    request: &InvokeRequest,
) -> Result<Option<Value>, CommandError> {
    macro_rules! command {
        ($future:expr) => {
            as_json($future.await)
        };
    }
    let result = match request.command.as_str() {
        "list_extensions" => command!(commands::extensions::list_extensions(app.clone())),
        "recover_extension_transactions" => {
            command!(commands::extensions::recover_extension_transactions(
                app.clone()
            ))
        }
        "discover_extensions" => {
            command!(commands::extensions::discover_extensions(app.clone()))
        }
        "save_extension" => command!(commands::extensions::save_extension(
            app.clone(),
            argument::<commands::extensions::ExtensionDraft>(&request.args, "draft")?,
        )),
        "get_mcp_edit_view" => command!(commands::extensions::get_mcp_edit_view(
            app.clone(),
            argument(&request.args, "definitionId")?,
        )),
        "update_mcp_definition" => command!(commands::extensions::update_mcp_definition(
            app.clone(),
            argument(&request.args, "definitionId")?,
            argument::<asb_core::extensions::McpEditRequest>(&request.args, "edit")?,
        )),
        "delete_extension" => command!(commands::extensions::delete_extension(
            app.clone(),
            argument(&request.args, "id")?,
            argument(&request.args, "confirmWrite")?,
        )),
        "register_project" => command!(commands::extensions::register_project(
            app.clone(),
            argument(&request.args, "root")?,
        )),
        "scan_local_skill_source" => command!(commands::extensions::scan_local_skill_source(
            app.clone(),
            argument(&request.args, "root")?,
        )),
        "resolve_skill_source" => command!(commands::extensions::resolve_skill_source(
            argument(&request.args, "repo")?,
            argument(&request.args, "subpath")?,
            argument::<Option<String>>(&request.args, "refName")?,
        )),
        "list_skill_repositories" => {
            command!(commands::extensions::list_skill_repositories(app.clone()))
        }
        "save_skill_repository" => command!(commands::extensions::save_skill_repository(
            app.clone(),
            argument::<commands::extensions::SkillRepositoryInput>(&request.args, "input")?,
        )),
        "remove_skill_repository" => command!(commands::extensions::remove_skill_repository(
            app.clone(),
            argument(&request.args, "id")?,
        )),
        "scan_skill_repositories" => {
            command!(commands::extensions::scan_skill_repositories(app.clone()))
        }
        "search_skill_directory" => command!(commands::extensions::search_skill_directory(
            argument(&request.args, "query")?,
        )),
        "search_mcp_registry" => command!(commands::extensions::search_mcp_registry(
            argument(&request.args, "query")?,
            argument(&request.args, "cursor")?,
        )),
        "check_mcp_registry_updates" => command!(commands::extensions::check_mcp_registry_updates(
            app.clone(), argument(&request.args, "definitionIds")?,
        )),
        "apply_mcp_registry_update" => command!(commands::extensions::apply_mcp_registry_update(
            app.clone(), argument(&request.args, "definitionId")?,
            argument(&request.args, "expectedRevision")?, argument(&request.args, "source")?,
            argument(&request.args, "action")?,
        )),
        "resolve_directory_skill" => {
            command!(commands::extensions::resolve_directory_skill(argument::<
                commands::extensions::SkillDirectoryEntry,
            >(
                &request.args,
                "entry"
            )?,))
        }
        "scan_skill_zip" => command!(commands::extensions::scan_skill_zip(argument(
            &request.args,
            "path"
        )?,)),
        "import_skill_candidate" => command!(commands::extensions::import_skill_candidate(
            app.clone(),
            argument(&request.args, "candidateId")?,
            argument(&request.args, "name")?,
            argument::<Option<AppKind>>(&request.args, "hostScoped")?,
        )),
        "import_discovered_skill" => command!(commands::extensions::import_discovered_skill(
            app.clone(),
            argument(&request.args, "observationId")?,
        )),
        "import_discovered_mcp" => command!(commands::extensions::import_discovered_mcp(
            app.clone(),
            argument(&request.args, "observationId")?,
        )),
        "takeover_discovered_extension" => {
            command!(commands::extensions::takeover_discovered_extension(
                app.clone(),
                argument(&request.args, "observationId")?,
                argument(&request.args, "confirmWrite")?,
            ))
        }
        "export_extension_portable" => {
            command!(commands::extensions::export_extension_portable(
                app.clone(),
                argument(&request.args, "definitionId")?,
                argument(&request.args, "targetPath")?,
            ))
        }
        "import_extension_portable" => {
            command!(commands::extensions::import_extension_portable(
                app.clone(),
                argument(&request.args, "packagePath")?,
                argument(&request.args, "confirmWrite")?,
            ))
        }
        "check_skill_updates" => command!(commands::extensions::check_skill_updates(
            app.clone(),
            argument(&request.args, "definitionIds")?,
        )),
        "set_skill_version" => command!(commands::extensions::set_skill_version(
            app.clone(),
            argument(&request.args, "definitionId")?,
            argument(&request.args, "expectedRevision")?,
            argument(&request.args, "change")?,
        )),
        "create_local_skill" => command!(commands::extensions::create_local_skill(
            app.clone(),
            argument(&request.args, "draft")?,
        )),
        "fork_local_skill" => command!(commands::extensions::fork_local_skill(
            app.clone(),
            argument(&request.args, "definitionId")?,
        )),
        "get_skill_editor" => command!(commands::extensions::get_skill_editor(
            app.clone(),
            argument(&request.args, "definitionId")?,
        )),
        "update_skill_files" => command!(commands::extensions::update_skill_files(
            app.clone(),
            argument(&request.args, "definitionId")?,
            argument(&request.args, "update")?,
        )),
        "list_skill_versions" => command!(commands::extensions::list_skill_versions(
            app.clone(),
            argument(&request.args, "definitionId")?,
        )),
        "update_skill_dependencies" => {
            command!(commands::extensions::update_skill_dependencies(
                app.clone(),
                argument(&request.args, "definitionId")?,
                argument(&request.args, "update")?,
            ))
        }
        "list_skill_backups" => {
            command!(commands::extensions::list_skill_backups(app.clone()))
        }
        "restore_skill_backup" => command!(commands::extensions::restore_skill_backup(
            app.clone(),
            argument(&request.args, "backupId")?,
            argument(&request.args, "confirmWrite")?,
        )),
        "delete_skill_backup" => command!(commands::extensions::delete_skill_backup(
            app.clone(),
            argument(&request.args, "backupId")?,
            argument(&request.args, "confirmWrite")?,
        )),
        "put_extension_secret" => command!(commands::extensions::put_extension_secret(
            argument(&request.args, "value")?,
            argument(&request.args, "purpose")?,
        )),
        "prepare_extension_plan" => command!(commands::extensions::prepare_extension_plan(
            app.clone(),
            argument::<commands::extensions::PlanRequest>(&request.args, "request")?,
        )),
        "prepare_extension_repair" => {
            command!(commands::extensions::prepare_extension_repair(
                app.clone(),
                argument::<commands::extensions::RepairRequest>(&request.args, "request")?,
            ))
        }
        "apply_extension_plan" => command!(commands::extensions::apply_extension_plan(
            app.clone(),
            argument(&request.args, "planId")?,
            argument(&request.args, "confirmWrite")?,
        )),
        "get_extension_operation" => command!(commands::extensions::get_extension_operation(
            app.clone(),
            argument(&request.args, "operationId")?,
        )),
        "prepare_extension_restore" => {
            command!(commands::extensions::prepare_extension_restore(
                app.clone(),
                argument(&request.args, "operationId")?,
            ))
        }
        "check_mcp_connection" => command!(commands::extensions::check_mcp_connection(
            app.clone(),
            argument(&request.args, "definitionId")?,
            argument(&request.args, "definitionRevision")?,
            argument::<ExtensionTarget>(&request.args, "target")?,
            argument(&request.args, "confirm")?,
        )),
        "get_mcp_check" => command!(commands::extensions::get_mcp_check(argument(
            &request.args,
            "checkId"
        )?,)),
        "cancel_mcp_check" => command!(commands::extensions::cancel_mcp_check(argument(
            &request.args,
            "checkId"
        )?,)),
        _ => return Ok(None),
    };
    result.map(Some)
}
