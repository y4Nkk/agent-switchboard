//! Definition lifecycle commands: creating, editing, and deleting library
//! definitions, project registration, and the secret-handle interface.

use std::fs;

use asb_core::extensions::contracts::{
    ExtensionDefinition, ExtensionPayload, FileState, McpEditRequest, McpMetadata,
    ProjectRegistration, EXTENSIONS_SCHEMA_VERSION,
};
use asb_core::extensions::edit::{apply_mcp_edit, apply_mcp_metadata, mcp_edit_view, McpEditView};
use asb_core::extensions::validate::{validate_definition, validate_definition_draft};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::support::*;
use crate::commands::error::{blocking, require_write_confirmation, state, CommandError};
use crate::extensions::discovery::binding_file_state;
use crate::extensions::secrets::SecretBackend;
use crate::extensions::secrets::SystemSecrets;
use crate::extensions::store::{ExtensionStore, LibraryCommit};

// ---------------------------------------------------------------- save / edit / delete

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionDraft {
    name: String,
    #[serde(default)]
    mcp_metadata: Option<McpMetadata>,
    payload: ExtensionPayload,
}

/// Creates one library definition. Editing an existing definition has its
/// own typed paths (`update_mcp_definition`, `set_skill_version`);
/// there is deliberately no whole-payload update.
#[tauri::command]
pub async fn save_extension(
    app: AppHandle,
    draft: ExtensionDraft,
) -> Result<ExtensionMutationDto, CommandError> {
    blocking(move || {
        let state = state(&app)?;
        let store = extension_store(&state);
        save_definition(&store, draft)
    })
    .await
}

fn save_definition(
    store: &ExtensionStore,
    draft: ExtensionDraft,
) -> Result<ExtensionMutationDto, CommandError> {
    validate_definition_draft(&draft.name, &draft.payload)
        .map_err(|error| CommandError::new("extension-invalid", error.message))?;
    if let Some(source) = draft.mcp_metadata.as_ref().and_then(|metadata| metadata.registry.as_ref()) {
        if !matches!(&draft.payload, ExtensionPayload::Mcp(asb_core::extensions::contracts::McpDefinition::Http { url, .. }) if url == &source.endpoint) {
            return Err(CommandError::new("extension-invalid", "MCP 配置与目录端点不一致"));
        }
    }
    let definition = ExtensionDefinition {
        schema_version: EXTENSIONS_SCHEMA_VERSION,
        id: new_id("ext"),
        name: draft.name,
        mcp_metadata: draft.mcp_metadata,
        revision: 1,
        mcp_config_revision: None,
        created_at: now(),
        updated_at: now(),
        payload: draft.payload,
    };
    validate_definition(&definition)
        .map_err(|error| CommandError::new("extension-invalid", error.message))?;
    store.create_definition(&definition).map_err(store_error)?;
    Ok(extension_mutation(&definition))
}

/// The editor's prefilled, redacted projection of one MCP definition. Every
/// position the edit contract can change is present except credential
/// material: stored secret references are reduced to a presence marker and
/// their handles never cross the IPC boundary.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpEditViewDto {
    pub id: String,
    pub revision: u64,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_metadata: Option<McpMetadata>,
    #[serde(flatten)]
    pub view: McpEditView,
}

pub(super) fn mcp_edit_view_of(
    definition: &ExtensionDefinition,
) -> Result<McpEditViewDto, CommandError> {
    let ExtensionPayload::Mcp(mcp) = &definition.payload else {
        return Err(CommandError::keyed(
            "extension-invalid",
            "errors.extlib.notAnMcpDefinition",
            "该扩展不是 MCP 定义",
        ));
    };
    Ok(McpEditViewDto {
        id: definition.id.clone(),
        revision: definition.revision,
        name: definition.name.clone(),
        mcp_metadata: definition.mcp_metadata.clone(),
        view: mcp_edit_view(mcp),
    })
}

#[tauri::command]
pub async fn get_mcp_edit_view(
    app: AppHandle,
    definition_id: String,
) -> Result<McpEditViewDto, CommandError> {
    blocking(move || {
        let state = state(&app)?;
        let store = extension_store(&state);
        let definition = store
            .get_definition(&definition_id)
            .map_err(store_error)?
            .ok_or_else(|| CommandError::keyed("extension-not-found", "errors.extlib.extensionNotFound", "扩展不存在或已被删除"))?;
        mcp_edit_view_of(&definition)
    })
    .await
}

/// Applies one field-level MCP edit. Kept positions are copied from the
/// stored definition inside the backend, so a saved credential survives an
/// edit without its value ever returning to the renderer. The revision the
/// editor saw must still be current; an unchanged edit writes nothing.
#[tauri::command]
pub async fn update_mcp_definition(
    app: AppHandle,
    definition_id: String,
    edit: McpEditRequest,
) -> Result<ExtensionMutationDto, CommandError> {
    blocking(move || {
        let state = state(&app)?;
        let store = extension_store(&state);
        update_mcp(&store, &definition_id, edit)
    })
    .await
}

fn update_mcp(
    store: &ExtensionStore,
    id: &str,
    edit: McpEditRequest,
) -> Result<ExtensionMutationDto, CommandError> {
    let mut definition = store
        .get_definition(id)
        .map_err(store_error)?
        .ok_or_else(|| CommandError::keyed("extension-not-found", "errors.extlib.extensionNotFound", "扩展不存在或已被删除"))?;
    if definition.revision != edit.expected_revision {
        return Err(CommandError::keyed(
            "extension-conflict",
            "errors.extlib.extensionConflictRefresh",
            "扩展已被其他窗口修改；请刷新后重试",
        ));
    }
    let ExtensionPayload::Mcp(current) = &definition.payload else {
        return Err(CommandError::keyed(
            "extension-invalid",
            "errors.extlib.notAnMcpDefinition",
            "该扩展不是 MCP 定义",
        ));
    };
    let (name, payload) = apply_mcp_edit(&definition.name, current, &edit)
        .map_err(|error| CommandError::new("extension-invalid", error.message))?;
    let metadata = apply_mcp_metadata(&definition.mcp_metadata, edit.mcp_metadata.as_ref())
        .map_err(|error| CommandError::new("extension-invalid", error.message))?;
    let payload = ExtensionPayload::Mcp(payload);
    if definition.name == name
        && definition.payload == payload
        && definition.mcp_metadata == metadata
    {
        return Ok(extension_mutation(&definition));
    }
    let previous_revision = definition.revision;
    definition.name = name;
    definition.payload = payload;
    definition.mcp_metadata = metadata;
    definition.revision += 1;
    definition.updated_at = now();
    validate_definition(&definition)
        .map_err(|error| CommandError::new("extension-invalid", error.message))?;
    store
        .update_definition(&definition, previous_revision)
        .map_err(store_error)?;
    Ok(extension_mutation(&definition))
}

#[tauri::command]
pub async fn delete_extension(
    app: AppHandle,
    id: String,
    confirm_write: bool,
) -> Result<(), CommandError> {
    require_write_confirmation(confirm_write, "删除扩展定义")?;
    blocking(move || {
        let state = state(&app)?;
        let store = extension_store(&state);
        store
            .delete_definition_if_unbound(&id)
            .map_err(store_error)?;
        Ok(())
    })
    .await
}

/// Pins or unpins one Skill binding's content version. A library-only
/// record change: pinning requires the target to be in sync with the
/// definition's present content, and afterwards the version store keeps
/// serving exactly the pinned bytes to that target.
#[tauri::command]
pub async fn set_binding_lock(
    app: AppHandle,
    binding_id: String,
    locked: bool,
) -> Result<(), CommandError> {
    blocking(move || {
        let state = state(&app)?;
        let store = extension_store(&state);
        let binding = store
            .list_bindings()
            .map_err(store_error)?
            .into_iter()
            .find(|binding| binding.id == binding_id)
            .ok_or_else(|| CommandError::keyed("extension-not-found", "errors.extlib.bindingNotFound", "绑定不存在或已被移除"))?;
        let definition = store
            .get_definition(&binding.resource_id)
            .map_err(store_error)?
            .ok_or_else(|| CommandError::keyed("extension-not-found", "errors.extlib.extensionDefinitionMissing", "扩展定义不存在"))?;
        let ExtensionPayload::Skill(skill) = &definition.payload else {
            return Err(CommandError::keyed(
                "extension-invalid",
                "errors.extlib.bindingLockOnlySkill",
                "版本固定只适用于 Skill 绑定",
            ));
        };
        let mut next = binding.clone();
        if locked {
            let baseline = store.get_baseline_file(&binding.id).map_err(store_error)?;
            let projects = store.list_projects().map_err(store_error)?;
            let mut current_content = binding.clone();
            current_content.locked_digest = None;
            if binding_file_state(&current_content, &definition, baseline.as_ref(), &projects)
                != FileState::InSync
            {
                return Err(CommandError::keyed(
                    "extension-conflict",
                    "errors.extlib.bindingLockRequiresSync",
                    "只有内容与当前版本一致的目标才能固定；请先更新部署或恢复同步",
                ));
            }
            next.locked_digest = Some(skill.content_digest.clone());
        } else {
            next.locked_digest = None;
        }
        next.updated_at = now();
        // The generation + definition-revision precondition rejects a pin
        // racing a plan apply or a concurrent library edit.
        let generation = store.manifest().map_err(store_error)?.generation;
        store
            .commit_batch_if_current(
                &LibraryCommit {
                    history_operation_id: new_id("op"),
                    binding_upserts: vec![next],
                    baseline_files: Vec::new(),
                    baseline_deletes: Vec::new(),
                    binding_deletes: Vec::new(),
                    snapshot: None,
                    pre_bindings: vec![binding],
                    pre_baselines: Vec::new(),
                },
                generation,
                &[(definition.id.clone(), definition.revision)],
            )
            .map_err(store_error)?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn register_project(
    app: AppHandle,
    root: String,
) -> Result<ProjectRegistration, CommandError> {
    blocking(move || {
        let state = state(&app)?;
        let store = extension_store(&state);
        let canonical = fs::canonicalize(&root).map_err(|error| {
            CommandError::localized(
                "extension-invalid",
                "errors.extlib.projectRootUnresolvable",
                format!("项目目录无法解析：{error}"),
                serde_json::json!({ "detail": error.to_string() }),
            )
        })?;
        if !canonical.is_dir() {
            return Err(CommandError::keyed(
                "extension-invalid",
                "errors.extlib.projectPathNotDirectory",
                "项目路径不是目录",
            ));
        }
        let existing = store.list_projects().map_err(store_error)?;
        if let Some(found) = existing
            .iter()
            .find(|project| project.root == canonical.to_string_lossy())
        {
            return Ok(found.clone());
        }
        let project = ProjectRegistration {
            schema_version: EXTENSIONS_SCHEMA_VERSION,
            id: new_id("proj"),
            root: canonical.to_string_lossy().to_string(),
            display_name: canonical
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| canonical.to_string_lossy().to_string()),
            registered_at: now(),
        };
        store.create_project(&project).map_err(store_error)?;
        Ok(project)
    })
    .await
}

// ---------------------------------------------------------------- secrets

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretRefDto {
    reference: String,
}

#[tauri::command]
pub async fn put_extension_secret(
    value: String,
    purpose: String,
) -> Result<SecretRefDto, CommandError> {
    blocking(move || {
        if value.trim().is_empty() {
            return Err(CommandError::keyed("extension-invalid", "errors.extlib.secretValueEmpty", "凭据值不能为空"));
        }
        let reference = format!("secret-{}", uuid::Uuid::new_v4().simple());
        SystemSecrets
            .put(&reference, &value)
            .map_err(|error| CommandError::new("secret-unavailable", error.to_string()))?;
        // The purpose is metadata only; the value never leaves this call.
        let _ = purpose;
        Ok(SecretRefDto { reference })
    })
    .await
}

