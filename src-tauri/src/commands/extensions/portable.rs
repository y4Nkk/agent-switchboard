//! Portable export/import packages: the only shape that may leave this
//! machine. Exports carry immutable skill content or a stdio MCP skeleton;
//! remote endpoints and credential values never travel. Imports report
//! the credential slots the user must re-bind locally.

use std::fs;

use asb_core::extensions::contracts::{
    ExtensionDefinition, ExtensionPayload, SkillDefinition, EXTENSIONS_SCHEMA_VERSION,
};
use asb_core::extensions::validate::validate_definition;
use serde::Serialize;
use tauri::AppHandle;

use super::support::*;
use crate::commands::error::{blocking, require_write_confirmation, state, CommandError};

// ------------------------------------------------------- portable packages

pub(super) fn portable_error(error: asb_core::extensions::portable::PortableError) -> CommandError {
    CommandError::new("extension-invalid", error.message)
}

/// Writes one extension's portable package to a user-chosen path. The
/// package carries immutable skill content or a stdio MCP skeleton; remote
/// endpoints, credential values, and client documents never leave.
#[cfg_attr(test, allow(dead_code))]
#[tauri::command]
pub async fn export_extension_portable(
    app: AppHandle,
    definition_id: String,
    target_path: String,
) -> Result<(), CommandError> {
    blocking(move || {
        let state = state(&app)?;
        let store = extension_store(&state);
        let definition = store
            .get_definition(&definition_id)
            .map_err(store_error)?
            .ok_or_else(|| CommandError::keyed("extension-not-found", "errors.extlib.extensionNotFound", "扩展不存在或已被删除"))?;
        let package = match &definition.payload {
            ExtensionPayload::Skill(skill) => {
                let entries = store
                    .load_skill_version(&definition.id, &skill.content_digest)
                    .map_err(store_error)?;
                asb_core::extensions::portable::export_skill(skill, &entries)
                    .map_err(portable_error)?
            }
            ExtensionPayload::Mcp(mcp) => {
                asb_core::extensions::portable::export_mcp(&definition.name, mcp)
                    .map_err(portable_error)?
            }
        };
        let path = std::path::PathBuf::from(&target_path);
        if path.is_dir() {
            return Err(CommandError::keyed(
                "extension-invalid",
                "errors.extlib.exportTargetIsDirectory",
                "导出目标是一个目录，请提供文件路径",
            ));
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.is_dir() {
                return Err(CommandError::keyed(
                    "extension-invalid",
                    "errors.extlib.exportParentMissing",
                    "导出目标的父目录不存在",
                ));
            }
        }
        let json = serde_json::to_string_pretty(&package).map_err(|error| {
            CommandError::localized(
                "extension-store",
                "errors.extlib.portableSerializeFailed",
                format!("便携包序列化失败：{error}"),
                serde_json::json!({ "detail": error.to_string() }),
            )
        })?;
        fs::write(&path, json).map_err(|error| {
            CommandError::localized(
                "extension-store",
                "errors.extlib.portableWriteFailed",
                format!("无法写入便携包：{error}"),
                serde_json::json!({ "detail": error.to_string() }),
            )
        })?;
        Ok(())
    })
    .await
}

/// The import result: the created definition plus the credential slots the
/// user must re-bind before deploying on this machine.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableImportReportDto {
    definition: ExtensionMutationDto,
    missing_env_slots: Vec<String>,
    warnings: Vec<String>,
}

#[cfg_attr(test, allow(dead_code))]
#[tauri::command]
pub async fn import_extension_portable(
    app: AppHandle,
    package_path: String,
    confirm_write: bool,
) -> Result<PortableImportReportDto, CommandError> {
    require_write_confirmation(confirm_write, "导入便携包")?;
    blocking(move || {
        let state = state(&app)?;
        let store = extension_store(&state);
        let text = fs::read_to_string(&package_path)
            .map_err(|_| CommandError::keyed("extension-invalid", "errors.extlib.portableUnreadable", "便携包文件无法读取"))?;
        let package: asb_core::extensions::portable::PortablePackage = serde_json::from_str(&text)
            .map_err(|error| {
                CommandError::localized(
                    "extension-invalid",
                    "errors.extlib.portableInvalidFormat",
                    format!("便携包不是有效格式：{error}"),
                    serde_json::json!({ "detail": error.to_string() }),
                )
            })?;
        let material =
            asb_core::extensions::portable::prepare_import(&package).map_err(portable_error)?;
        match material {
            asb_core::extensions::portable::ImportMaterial::Skill {
                manifest,
                source,
                host_scoped,
                dependencies,
                entries,
            } => {
                let digest = asb_core::extensions::skill::content_digest(&entries);
                let compatibility =
                    asb_core::extensions::skill::claude_compatibility_notes(&manifest);
                let definition = ExtensionDefinition {
                    schema_version: EXTENSIONS_SCHEMA_VERSION,
                    mcp_metadata: None,
                    mcp_config_revision: None,
                    id: new_id("ext"),
                    name: manifest.name.clone(),
                    revision: 1,
                    created_at: now(),
                    updated_at: now(),
                    payload: ExtensionPayload::Skill(SkillDefinition {
                        content_digest: digest.clone(),
                        manifest,
                        source,
                        host_scoped,
                        compatibility,
                        dependencies: asb_core::extensions::portable::portable_dependencies(
                            &dependencies,
                        ),
                    }),
                };
                validate_definition(&definition)
                    .map_err(|error| CommandError::new("extension-invalid", error.message))?;
                store
                    .save_skill_version(&definition.id, &digest, &entries)
                    .map_err(store_error)?;
                store.create_definition(&definition).map_err(store_error)?;
                Ok(PortableImportReportDto {
                    definition: extension_mutation(&definition),
                    missing_env_slots: Vec::new(),
                    warnings: vec![
                        "依赖已在库中登记为待关联；请在详情中绑定对应 MCP 定义".to_string()
                    ],
                })
            }
            asb_core::extensions::portable::ImportMaterial::McpStdio {
                name,
                definition: mcp,
                missing_env_slots,
            } => {
                if let Some(existing) = store
                    .list_definitions()
                    .map_err(store_error)?
                    .into_iter()
                    .find(|definition| {
                        definition.name == name
                            && definition.payload == ExtensionPayload::Mcp(mcp.clone())
                    })
                {
                    return Ok(PortableImportReportDto {
                        definition: extension_mutation(&existing),
                        missing_env_slots,
                        warnings: vec!["库中已有完全相同的定义，已复用".to_string()],
                    });
                }
                let definition = ExtensionDefinition {
                    schema_version: EXTENSIONS_SCHEMA_VERSION,
                    mcp_metadata: None,
                    mcp_config_revision: None,
                    id: new_id("ext"),
                    name,
                    revision: 1,
                    created_at: now(),
                    updated_at: now(),
                    payload: ExtensionPayload::Mcp(mcp),
                };
                validate_definition(&definition)
                    .map_err(|error| CommandError::new("extension-invalid", error.message))?;
                store.create_definition(&definition).map_err(store_error)?;
                Ok(PortableImportReportDto {
                    definition: extension_mutation(&definition),
                    missing_env_slots,
                    warnings: Vec::new(),
                })
            }
        }
    })
    .await
}
