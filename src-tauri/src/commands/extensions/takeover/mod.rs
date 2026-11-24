//! Taking over discovered native items: the library starts owning an
//! existing entry or directory as-is. The takeover itself never writes a
//! client file; the recorded baseline is exactly what a later removal
//! restores.

use std::fs;

use crate::extensions::store::LibraryCommit;
use asb_core::extensions::contracts::{
    DesiredState, ExtensionBinding, ExtensionDefinition, ExtensionKind, ExtensionPayload,
    ManagedBaseline, ManagedBaselineFile, ManagedFileEntry, PlanOperation,
    EXTENSIONS_SCHEMA_VERSION,
};
use asb_core::extensions::validate::{validate_binding, validate_definition};
use tauri::AppHandle;

use super::planner::Planner;
use super::sources::import_candidate_content;
use super::support::*;
use crate::commands::error::{blocking, require_write_confirmation, state, CommandError};
use crate::extensions::discovery::DiscoveredPaths;
use crate::extensions::store::ExtensionStore;

// ---------------------------------------------------------------- sources

// ---------------------------------------------------------------- takeover
use self::resolve::{resolve_mcp_takeover, resolve_skill_takeover};
/// Takes over one discovered native item: the library starts owning the
/// existing entry or directory as-is. No client file is written by the
/// takeover itself; the recorded baseline is what a later removal restores.
#[cfg_attr(test, allow(dead_code))]
#[tauri::command]
pub async fn takeover_discovered_extension(
    app: AppHandle,
    observation_id: String,
    confirm_write: bool,
) -> Result<ExtensionMutationDto, CommandError> {
    require_write_confirmation(confirm_write, "接管本机扩展")?;
    blocking(move || {
        let state = state(&app)?;
        let store = extension_store(&state);
        let paths = DiscoveredPaths::from_env()
            .map_err(|error| CommandError::new("app-state-unavailable", error))?;
        let cached = discovered_observations()
            .lock()
            .expect("observations")
            .get(&observation_id)
            .cloned()
            .ok_or_else(|| {
                CommandError::keyed(
                    "observation-expired",
                    "errors.extlib.observationExpired",
                    "发现结果已过期；请重新扫描本机扩展",
                )
            })?;
        match cached.observed.kind {
            ExtensionKind::Mcp => takeover_observed_mcp(&store, &cached, &paths),
            ExtensionKind::Skill => takeover_observed_skill(&store, &cached, &paths),
        }
    })
    .await
}

pub(super) fn takeover_observed_mcp(
    store: &ExtensionStore,
    cached: &CachedDiscoveryObservation,
    paths: &DiscoveredPaths,
) -> Result<ExtensionMutationDto, CommandError> {
    let (payload, material) = resolve_mcp_takeover(store, cached, paths)?;
    let observed = &material.observed;
    let definition = match store
        .list_definitions()
        .map_err(store_error)?
        .into_iter()
        .find(|definition| {
            definition.name == observed.name
                && definition.payload == ExtensionPayload::Mcp(payload.clone())
        }) {
        Some(existing) => existing,
        None => {
            let definition = ExtensionDefinition {
                schema_version: EXTENSIONS_SCHEMA_VERSION,
                mcp_metadata: None,
                mcp_config_revision: None,
                id: new_id("ext"),
                name: observed.name.clone(),
                revision: 1,
                created_at: now(),
                updated_at: now(),
                payload: ExtensionPayload::Mcp(payload),
            };
            validate_definition(&definition)
                .map_err(|error| CommandError::new("source-rejected", error.message))?;
            store.create_definition(&definition).map_err(store_error)?;
            definition
        }
    };
    let binding = ExtensionBinding {
        schema_version: EXTENSIONS_SCHEMA_VERSION,
        id: new_id("bind"),
        resource_id: definition.id.clone(),
        target: material.target.clone(),
        native_key: Some(observed.name.clone()),
        deploy_name: None,
        desired: DesiredState::Enabled,
        locked_digest: None,
        last_applied_revision: Some(definition.revision),
        updated_at: now(),
    };
    validate_binding(&definition, &binding)
        .map_err(|error| CommandError::new("extension-invalid", error.message))?;
    let no_secrets = |_: &str| -> Option<String> { None };
    let planner = Planner {
        store,
        paths,
        secrets: &no_secrets,
    };
    planner.assert_native_key_free(&binding)?;
    for other in store.list_bindings().map_err(store_error)? {
        if other.resource_id == definition.id && other.target == binding.target {
            return Err(CommandError::keyed(
                "extension-conflict",
                "errors.extlib.alreadyBoundNoTakeover",
                "该定义已绑定到所选目标，不能重复接管",
            ));
        }
    }
    planner.require_write_capabilities(&definition, &binding, PlanOperation::Install)?;
    let document = fs::read_to_string(&observed.path).map_err(|_| {
        CommandError::keyed(
            "observation-stale",
            "errors.extlib.mcpDocumentUnreadable",
            "发现到的 MCP 配置已无法读取；请重新扫描并确认",
        )
    })?;
    let baseline = ManagedBaselineFile::new(
        &binding.id,
        vec![ManagedBaseline::DocumentEntry {
            target_path: observed.path.clone(),
            entry_pointer: material
                .entry_pointer
                .clone()
                .expect("mcp takeover carries a pointer"),
            original_value: material.native_entry.clone(),
            last_written_value: material.native_entry.clone(),
            last_document_hash: sha_hex(document.as_bytes()),
            target_existed_before: true,
            backup_reference: None,
        }],
    );
    let baseline_id = binding.id.clone();
    commit_takeover(store, &definition, binding, vec![(baseline_id, baseline)])
}

pub(super) fn takeover_observed_skill(
    store: &ExtensionStore,
    cached: &CachedDiscoveryObservation,
    paths: &DiscoveredPaths,
) -> Result<ExtensionMutationDto, CommandError> {
    let (candidate, material) = resolve_skill_takeover(store, cached)?;
    let observed = &material.observed;
    // Managing includes adding to the library: an equivalent definition
    // (same immutable content digest) is reused, never duplicated.
    let existing = store
        .list_definitions()
        .map_err(store_error)?
        .into_iter()
        .find(|definition| {
            matches!(
                &definition.payload,
                ExtensionPayload::Skill(skill) if skill.content_digest == candidate.content_digest
            )
        });
    let definition = match existing {
        Some(definition) => definition,
        None => {
            let mutation = import_candidate_content(
                store,
                &candidate,
                observed.name.clone(),
                Some(observed.client),
            )?;
            store
                .get_definition(&mutation.id)
                .map_err(store_error)?
                .ok_or_else(|| CommandError::keyed("extension-store", "errors.extlib.takeoverDefinitionNotPersisted", "接管定义未持久化"))?
        }
    };
    let ExtensionPayload::Skill(skill) = &definition.payload else {
        return Err(CommandError::keyed(
            "extension-invalid",
            "errors.extlib.notASkillDefinition",
            "该扩展不是 Skill 定义",
        ));
    };
    let deploy_name = asb_core::extensions::skill::deploy_name_for(&skill.manifest)
        .map_err(|error| CommandError::new("extension-invalid", error.message))?;
    let binding = ExtensionBinding {
        schema_version: EXTENSIONS_SCHEMA_VERSION,
        id: new_id("bind"),
        resource_id: definition.id.clone(),
        target: material.target.clone(),
        native_key: None,
        deploy_name: Some(deploy_name),
        desired: DesiredState::Enabled,
        locked_digest: None,
        last_applied_revision: Some(definition.revision),
        updated_at: now(),
    };
    validate_binding(&definition, &binding)
        .map_err(|error| CommandError::new("extension-invalid", error.message))?;
    let no_secrets = |_: &str| -> Option<String> { None };
    let planner = Planner {
        store,
        paths,
        secrets: &no_secrets,
    };
    planner.require_write_capabilities(&definition, &binding, PlanOperation::Install)?;
    for other in store.list_bindings().map_err(store_error)? {
        if other.target == binding.target && other.deploy_name == binding.deploy_name {
            return Err(CommandError::keyed(
                "extension-conflict",
                "errors.extlib.targetManagedNoTakeover",
                "该目标目录已被另一绑定管理，不能重复接管",
            ));
        }
    }
    let managed: Vec<ManagedFileEntry> = candidate
        .entries
        .iter()
        .map(|entry| ManagedFileEntry {
            relative_path: entry.relative_path.clone(),
            digest: if entry.kind == asb_core::extensions::validate::ContentEntryKind::Dir {
                String::new()
            } else {
                sha_hex(&entry.bytes)
            },
            mode: entry.mode,
            size: entry.bytes.len() as u64,
        })
        .collect();
    let baseline = ManagedBaselineFile::new(
        &binding.id,
        vec![ManagedBaseline::Directory {
            target_dir: observed.path.clone(),
            original_existed: true,
            original_files: Some(managed.clone()),
            original_digest: Some(candidate.content_digest.clone()),
            last_files: managed,
            last_digest: candidate.content_digest.clone(),
            backup_reference: None,
        }],
    );
    let baseline_id = binding.id.clone();
    commit_takeover(store, &definition, binding, vec![(baseline_id, baseline)])
}

/// One library transaction for a takeover: the binding plus its recorded
/// baseline land together under the generation check.
pub(super) fn commit_takeover(
    store: &ExtensionStore,
    definition: &ExtensionDefinition,
    binding: ExtensionBinding,
    baseline_files: Vec<(String, ManagedBaselineFile)>,
) -> Result<ExtensionMutationDto, CommandError> {
    let generation = store.manifest().map_err(store_error)?.generation;
    store
        .commit_batch_if_current(
            &LibraryCommit {
                history_operation_id: new_id("op"),
                binding_upserts: vec![binding],
                baseline_files,
                baseline_deletes: Vec::new(),
                binding_deletes: Vec::new(),
                snapshot: None,
                pre_bindings: Vec::new(),
                pre_baselines: Vec::new(),
            },
            generation,
            &[(definition.id.clone(), definition.revision)],
        )
        .map_err(store_error)?;
    Ok(extension_mutation(definition))
}

mod resolve;
