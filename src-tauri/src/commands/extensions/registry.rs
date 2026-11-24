use asb_core::extensions::contracts::{
    ExtensionDefinition, ExtensionPayload, McpDefinition, McpRegistrySource,
};
use asb_core::extensions::validate::{mcp_registry_endpoint, validate_definition};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::sources::detailed_source_error;
use super::support::{extension_mutation, extension_store, now, store_error, ExtensionMutationDto};
use crate::commands::error::{blocking, state, CommandError};
use crate::extensions::{mcp_registry, sources, store::ExtensionStore};

#[tauri::command]
pub async fn search_mcp_registry(
    query: String,
    cursor: Option<String>,
) -> Result<mcp_registry::McpRegistryPage, CommandError> {
    blocking(move || {
        mcp_registry::search(&query, cursor.as_deref(), &sources::http_fetch)
            .map_err(detailed_source_error)
    })
    .await
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "camelCase", rename_all_fields = "camelCase")]
enum RegistryUpdate {
    Current,
    Available { source: McpRegistrySource, action: RegistryUpdateAction },
    Manual { version: String, docs: Option<String>, connections: Vec<mcp_registry::McpRegistryConnection> },
    Failed { message: String },
}

#[derive(Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RegistryUpdateAction {
    ReplaceEndpoint,
    AssociateCurrent,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRegistryUpdateReport {
    definition_id: String,
    revision: u64,
    #[serde(flatten)]
    update: RegistryUpdate,
}

fn has_same_origin(first: &str, second: &str) -> bool {
    mcp_registry_endpoint(first)
        .zip(mcp_registry_endpoint(second))
        .is_some_and(|(first, second)| first.origin() == second.origin())
}

fn check_update(definition: &ExtensionDefinition) -> Result<RegistryUpdate, CommandError> {
    let ExtensionPayload::Mcp(server) = &definition.payload else {
        return Err(CommandError::keyed("extension-invalid", "errors.extlib.notAnMcpDefinition", "该扩展不是 MCP 定义"));
    };
    let source = definition.mcp_metadata.as_ref()
        .and_then(|metadata| metadata.registry.as_ref())
        .ok_or_else(|| CommandError::new("registry-source-missing", "此 MCP 未关联在线来源"))?;
    let latest = mcp_registry::latest(source, &sources::http_fetch)
        .map_err(detailed_source_error)?;
    Ok(classify_update(server, source, &latest))
}

fn classify_update(
    server: &McpDefinition,
    source: &McpRegistrySource,
    latest: &mcp_registry::RegistryRelease,
) -> RegistryUpdate {
    if let Some(next) = latest.configured_source(server) {
        if &next == source {
            return RegistryUpdate::Current;
        }
        return RegistryUpdate::Available { source: next, action: RegistryUpdateAction::AssociateCurrent };
    }
    if latest.version == source.version {
        return RegistryUpdate::Current;
    }
    if let (McpDefinition::Http { url, .. }, Some(next)) =
        (server, latest.default_source())
    {
        if url == &source.endpoint && has_same_origin(url, &next.endpoint) {
            return RegistryUpdate::Available { source: next, action: RegistryUpdateAction::ReplaceEndpoint };
        }
    }
    RegistryUpdate::Manual {
        version: latest.version.clone(), docs: latest.docs(), connections: latest.connections(),
    }
}

fn load_definition(store: &ExtensionStore, id: &str) -> Result<ExtensionDefinition, CommandError> {
    store.get_definition(id).map_err(store_error)?.ok_or_else(|| {
        CommandError::keyed("extension-not-found", "errors.extlib.extensionNotFound", "扩展不存在或已被删除")
    })
}

#[tauri::command]
pub async fn check_mcp_registry_updates(
    app: AppHandle,
    definition_ids: Vec<String>,
) -> Result<Vec<McpRegistryUpdateReport>, CommandError> {
    blocking(move || {
        let store = extension_store(&state(&app)?);
        Ok(definition_ids.into_iter().map(|definition_id| {
            match load_definition(&store, &definition_id) {
                Ok(definition) => McpRegistryUpdateReport {
                    definition_id,
                    revision: definition.revision,
                    update: check_update(&definition).unwrap_or_else(|error| {
                        RegistryUpdate::Failed { message: error.message }
                    }),
                },
                Err(error) => McpRegistryUpdateReport {
                    definition_id,
                    revision: 0,
                    update: RegistryUpdate::Failed { message: error.message },
                },
            }
        }).collect())
    }).await
}

#[tauri::command]
pub async fn apply_mcp_registry_update(
    app: AppHandle,
    definition_id: String,
    expected_revision: u64,
    source: McpRegistrySource,
    action: RegistryUpdateAction,
) -> Result<ExtensionMutationDto, CommandError> {
    blocking(move || {
        let store = extension_store(&state(&app)?);
        let mut definition = load_definition(&store, &definition_id)?;
        let conflict = || CommandError::keyed(
            "registry-update-stale", "extensions.registry.updateStale", "MCP 定义或目录版本已变化，请重新检查更新",
        );
        if definition.revision != expected_revision {
            return Err(conflict());
        }
        let RegistryUpdate::Available { source: next, action: next_action } = check_update(&definition)? else {
            return Err(conflict());
        };
        if next != source || next_action != action {
            return Err(conflict());
        }
        if action == RegistryUpdateAction::ReplaceEndpoint {
            let ExtensionPayload::Mcp(McpDefinition::Http { url, .. }) = &mut definition.payload else {
                return Err(conflict());
            };
            *url = next.endpoint.clone();
        }
        definition.mcp_metadata.as_mut().ok_or_else(conflict)?.registry = Some(next);
        definition.revision += 1;
        definition.updated_at = now();
        validate_definition(&definition).map_err(|error| CommandError::new("extension-invalid", error.message))?;
        store.update_definition(&definition, expected_revision).map_err(store_error)?;
        Ok(extension_mutation(&definition))
    })
    .await
}

#[cfg(test)]
mod tests;
