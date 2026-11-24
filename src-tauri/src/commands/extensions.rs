//! Tauri commands for the extensions workspace. Commands orchestrate the
//! extension store, discovery, sources, secrets, and checks; every real
//! client-file write goes through `asb_switch::extensions`, and every plan
//! must be prepared, previewed, and confirmed before it can be applied.

mod actions;
mod apply;
mod catalog;
mod checks;
mod definitions;
mod planner;
mod portable;
mod registry;
mod repair;
mod skill_backups;
mod skills;
mod sources;
mod support;
mod takeover;
mod workspace;

pub use apply::{
    __cmd__apply_extension_plan, __cmd__get_extension_operation, __cmd__prepare_extension_restore,
    __tauri_command_name_apply_extension_plan, __tauri_command_name_get_extension_operation,
    __tauri_command_name_prepare_extension_restore, apply_extension_plan, get_extension_operation,
    prepare_extension_restore,
};
pub use catalog::{
    __cmd__list_skill_repositories, __cmd__remove_skill_repository, __cmd__resolve_directory_skill,
    __cmd__save_skill_repository, __cmd__scan_skill_repositories, __cmd__search_skill_directory,
    __tauri_command_name_list_skill_repositories, __tauri_command_name_remove_skill_repository,
    __tauri_command_name_resolve_directory_skill, __tauri_command_name_save_skill_repository,
    __tauri_command_name_scan_skill_repositories, __tauri_command_name_search_skill_directory,
    list_skill_repositories, remove_skill_repository, resolve_directory_skill,
    save_skill_repository, scan_skill_repositories, search_skill_directory,
};
// The two catalog types only surface through the browser-development IPC
// dispatch, which release builds compile out.
#[cfg(debug_assertions)]
pub use catalog::{SkillDirectoryEntry, SkillRepositoryInput};
pub use checks::{
    __cmd__cancel_mcp_check, __cmd__check_mcp_connection, __cmd__get_mcp_check,
    __tauri_command_name_cancel_mcp_check, __tauri_command_name_check_mcp_connection,
    __tauri_command_name_get_mcp_check, cancel_mcp_check, check_mcp_connection, get_mcp_check,
};
#[cfg(debug_assertions)]
pub use definitions::ExtensionDraft;
pub use definitions::{
    __cmd__delete_extension, __cmd__get_mcp_edit_view, __cmd__put_extension_secret,
    __cmd__register_project, __cmd__save_extension, __cmd__set_binding_lock,
    __cmd__update_mcp_definition, __tauri_command_name_delete_extension,
    __tauri_command_name_get_mcp_edit_view, __tauri_command_name_put_extension_secret,
    __tauri_command_name_register_project, __tauri_command_name_save_extension,
    __tauri_command_name_set_binding_lock, __tauri_command_name_update_mcp_definition,
    delete_extension, get_mcp_edit_view, put_extension_secret, register_project, save_extension,
    set_binding_lock, update_mcp_definition,
};
#[cfg(debug_assertions)]
pub use planner::PlanRequest;
pub use planner::{
    __cmd__prepare_extension_plan, __tauri_command_name_prepare_extension_plan,
    prepare_extension_plan,
};
pub use portable::{
    __cmd__export_extension_portable, __cmd__import_extension_portable,
    __tauri_command_name_export_extension_portable, __tauri_command_name_import_extension_portable,
    export_extension_portable, import_extension_portable,
};
pub use registry::{
    __cmd__apply_mcp_registry_update, __cmd__check_mcp_registry_updates, __cmd__search_mcp_registry,
    __tauri_command_name_apply_mcp_registry_update, __tauri_command_name_check_mcp_registry_updates,
    __tauri_command_name_search_mcp_registry, apply_mcp_registry_update, check_mcp_registry_updates,
    search_mcp_registry,
};
#[cfg(debug_assertions)]
pub use repair::RepairRequest;
pub use repair::{
    __cmd__prepare_extension_repair, __tauri_command_name_prepare_extension_repair,
    prepare_extension_repair,
};
pub use skill_backups::{
    __cmd__delete_skill_backup, __cmd__list_skill_backups, __cmd__restore_skill_backup,
    __tauri_command_name_delete_skill_backup, __tauri_command_name_list_skill_backups,
    __tauri_command_name_restore_skill_backup, delete_skill_backup, list_skill_backups,
    restore_skill_backup,
};
pub use skills::{
    __cmd__check_skill_updates, __cmd__create_local_skill, __cmd__fork_local_skill,
    __cmd__get_skill_editor, __cmd__list_skill_versions, __cmd__set_skill_version,
    __cmd__update_skill_dependencies, __cmd__update_skill_files,
    __tauri_command_name_check_skill_updates, __tauri_command_name_create_local_skill,
    __tauri_command_name_fork_local_skill, __tauri_command_name_get_skill_editor,
    __tauri_command_name_list_skill_versions, __tauri_command_name_set_skill_version,
    __tauri_command_name_update_skill_dependencies, __tauri_command_name_update_skill_files,
    check_skill_updates, create_local_skill, fork_local_skill, get_skill_editor,
    list_skill_versions, set_skill_version, update_skill_dependencies, update_skill_files,
};
pub use sources::{
    __cmd__import_discovered_mcp, __cmd__import_discovered_skill, __cmd__import_skill_candidate,
    __cmd__resolve_skill_source, __cmd__scan_local_skill_source, __cmd__scan_skill_zip,
    __tauri_command_name_import_discovered_mcp, __tauri_command_name_import_discovered_skill,
    __tauri_command_name_import_skill_candidate, __tauri_command_name_resolve_skill_source,
    __tauri_command_name_scan_local_skill_source, __tauri_command_name_scan_skill_zip,
    import_discovered_mcp, import_discovered_skill, import_skill_candidate, resolve_skill_source,
    scan_local_skill_source, scan_skill_zip,
};
pub use takeover::{
    __cmd__takeover_discovered_extension, __tauri_command_name_takeover_discovered_extension,
    takeover_discovered_extension,
};
pub use workspace::{
    __cmd__discover_extensions, __cmd__list_extensions, __cmd__recover_extension_transactions,
    __tauri_command_name_discover_extensions, __tauri_command_name_list_extensions,
    __tauri_command_name_recover_extension_transactions, discover_extensions, list_extensions,
    recover_extension_transactions,
};

/// Programmable enable/disable entry for orchestrators (the Codex project
/// plan apply). It builds the very same staged plan the workspace UI builds,
/// then applies it immediately because the orchestrator has already taken the
/// user's explicit confirmation for the whole project plan. A rejection or a
/// rollback surfaces as a typed error so the caller can report it as one
/// resource's failure and keep applying the rest.
pub(crate) async fn apply_binding_states(
    app: tauri::AppHandle,
    toggles: Vec<(String, bool)>,
) -> Result<(), crate::commands::error::CommandError> {
    use crate::commands::error::CommandError;
    if toggles.is_empty() {
        return Ok(());
    }
    let view =
        planner::prepare_extension_plan(app.clone(), planner::binding_state_request(toggles)).await?;
    let outcome = apply::apply_extension_plan(app, view.plan_id, true).await?;
    if let Some(rejected) = outcome.rejected {
        return Err(CommandError::new("extension-plan-rejected", rejected));
    }
    if outcome.rolled_back {
        return Err(CommandError::keyed(
            "extension-plan-rolled-back",
            "errors.extlib.planRolledBack",
            "扩展变更已回滚，客户端文件保持原状",
        ));
    }
    Ok(())
}
