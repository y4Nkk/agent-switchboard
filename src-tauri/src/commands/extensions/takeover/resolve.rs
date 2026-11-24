//! Strict re-resolution of a cached discovery observation: the document
//! digest and content digest recorded by the discovery pass must still
//! match, and the derived binding target plus takeover material come from
//! this verified re-read.

//! Taking over discovered native items: the library starts owning an
//! existing entry or directory as-is. The takeover itself never writes a
//! client file; the recorded baseline is exactly what a later removal
//! restores.

use std::fs;

use asb_core::contracts::AppKind;
use asb_core::extensions::contracts::{
    ExtensionKind, ExtensionTarget, McpDefinition, ObservedExtension, ObservedOrigin,
};
use asb_core::extensions::mcp::{
    apply_claude_project_private_server_patches, apply_claude_user_server_patches,
    apply_codex_server_patches, import_claude_project_private_server, import_claude_server,
    import_codex_server,
};

use crate::commands::error::CommandError;
use crate::commands::extensions::support::*;
use crate::extensions::discovery::DiscoveredPaths;
use crate::extensions::sources::{self};
use crate::extensions::store::ExtensionStore;

// ---------------------------------------------------------------- sources

// ---------------------------------------------------------------- takeover

/// Everything a confirmed takeover needs after one discovery observation has
/// been strictly re-parsed and re-verified against the recorded digest.
pub(super) struct TakeoverMaterial {
    pub(super) observed: ObservedExtension,
    pub(super) target: ExtensionTarget,
    /// MCP only: the canonical text of the native entry as it stands, plus
    /// the baseline pointer it will be recorded under.
    pub(super) native_entry: Option<String>,
    pub(super) entry_pointer: Option<String>,
}

/// renderer never learns a path; only the typed scope travels.
pub(super) fn takeover_target(
    store: &ExtensionStore,
    observed: &ObservedExtension,
    claude_private_document: bool,
) -> Result<ExtensionTarget, CommandError> {
    match &observed.origin {
        ObservedOrigin::UserRoot => Ok(ExtensionTarget::App {
            client: observed.client,
        }),
        ObservedOrigin::ProjectRoot { project_path } => {
            let project = store
                .list_projects()
                .map_err(store_error)?
                .into_iter()
                .find(|project| project.root == *project_path)
                .ok_or_else(|| {
                    CommandError::keyed(
                        "extension-conflict",
                        "errors.extlib.projectNotRegisteredTakeover",
                        "该项目目录尚未注册；请先在扩展页注册项目后接管",
                    )
                })?;
            match (observed.client, claude_private_document) {
                (AppKind::Codex, _) => Ok(ExtensionTarget::ProjectShared {
                    client: AppKind::Codex,
                    project_id: project.id,
                }),
                (AppKind::Claude, true) => Ok(ExtensionTarget::ProjectPrivate {
                    client: AppKind::Claude,
                    project_id: project.id,
                }),
                (AppKind::Claude, false) => Ok(ExtensionTarget::ProjectShared {
                    client: AppKind::Claude,
                    project_id: project.id,
                }),
            }
        }
        ObservedOrigin::LegacyRoot { detail } | ObservedOrigin::Managed { detail } => {
            Err(CommandError::localized(
                "extension-unsupported-target",
                "errors.extlib.readOnlyOriginNoTakeover",
                format!("该来源（{detail}）只读，不能接管"),
                serde_json::json!({ "detail": detail }),
            ))
        }
    }
}

/// Strictly re-reads and re-parses the observed native MCP server. The
/// document digest recorded by the discovery pass must still match; the
/// native entry text returned is exactly what the takeover baseline will
/// record and what a later removal restores.
pub(super) fn resolve_mcp_takeover(
    store: &ExtensionStore,
    cached: &CachedDiscoveryObservation,
    paths: &DiscoveredPaths,
) -> Result<(McpDefinition, TakeoverMaterial), CommandError> {
    let observed = cached.observed.clone();
    if observed.kind != ExtensionKind::Mcp {
        return Err(CommandError::keyed(
            "extension-invalid",
            "errors.extlib.observationNotMcpTakeover",
            "该发现结果不是 MCP 服务，不能接管",
        ));
    }
    if !observed.managed_binding_ids.is_empty() {
        return Err(CommandError::keyed(
            "extension-already-managed",
            "errors.extlib.mcpAlreadyManagedTakeover",
            "该 MCP 服务已由扩展库管理，无需接管",
        ));
    }
    let document_bytes = fs::read(&observed.path).map_err(|_| {
        CommandError::keyed(
            "observation-stale",
            "errors.extlib.mcpDocumentUnreadable",
            "发现到的 MCP 配置已无法读取；请重新扫描并确认",
        )
    })?;
    let document_digest = sha_hex(&document_bytes);
    if cached.document_digest.as_deref() != Some(document_digest.as_str()) {
        return Err(CommandError::keyed(
            "observation-stale",
            "errors.extlib.mcpChangedSinceScan",
            "该 MCP 配置已在扫描后变化；请重新扫描并确认",
        ));
    }
    let document = String::from_utf8(document_bytes)
        .map_err(|_| CommandError::keyed("source-rejected", "errors.extlib.mcpNotTextTakeover", "MCP 配置不是有效文本，不能安全接管"))?;
    let claude_user_document =
        crate::extensions::paths::claude_user_json_path(&paths.home, paths.claude_dir.as_deref());
    let is_claude_private = matches!(
        (&observed.client, &observed.origin),
        (AppKind::Claude, ObservedOrigin::ProjectRoot { .. })
    ) && std::path::Path::new(&observed.path) == claude_user_document;
    let payload = match (&observed.client, &observed.origin) {
        (AppKind::Claude, ObservedOrigin::ProjectRoot { project_path }) if is_claude_private => {
            import_claude_project_private_server(&document, project_path, &observed.name)
        }
        (AppKind::Codex, _) => import_codex_server(&document, &observed.name),
        (AppKind::Claude, _) => import_claude_server(&document, &observed.name),
    }
    .map_err(|error| CommandError::new("source-rejected", error.to_string()))?;
    let target = takeover_target(store, &observed, is_claude_private)?;
    let key = observed.name.clone();
    // The baseline material comes from a removal patch on the current text:
    // its `before` is the canonical text of the entry as the client sees it.
    let changes = match (&observed.client, &observed.origin) {
        (AppKind::Codex, _) => {
            apply_codex_server_patches(&document, &[(key.clone(), None)]).map_err(adapter_error)?
        }
        (AppKind::Claude, ObservedOrigin::ProjectRoot { project_path }) if is_claude_private => {
            apply_claude_project_private_server_patches(
                &document,
                project_path,
                &[(key.clone(), None)],
            )
            .map_err(adapter_error)?
        }
        (AppKind::Claude, _) => apply_claude_user_server_patches(&document, &[(key.clone(), None)])
            .map_err(adapter_error)?,
    }
    .1;
    let native_entry = changes
        .first()
        .and_then(|change| change.before.clone())
        .ok_or_else(|| {
            CommandError::keyed(
                "source-rejected",
                "errors.extlib.nativeMcpEntryGone",
                "原生 MCP 条目已不存在或为空；请重新扫描并确认",
            )
        })?;
    let entry_pointer = match (&observed.client, &observed.origin) {
        (AppKind::Codex, _) => format!("mcp_servers.{key}"),
        (AppKind::Claude, ObservedOrigin::ProjectRoot { project_path }) if is_claude_private => {
            format!("projects.{project_path}.mcpServers.{key}")
        }
        (AppKind::Claude, _) => format!("mcpServers.{key}"),
    };
    Ok((
        payload,
        TakeoverMaterial {
            observed,
            target,
            native_entry: Some(native_entry),
            entry_pointer: Some(entry_pointer),
        },
    ))
}

/// Re-scans the observed skill directory and verifies it still matches the
/// digest the discovery pass recorded. The returned candidate carries the
/// exact entries the takeover baseline will own.
pub(super) fn resolve_skill_takeover(
    store: &ExtensionStore,
    cached: &CachedDiscoveryObservation,
) -> Result<(sources::SkillCandidate, TakeoverMaterial), CommandError> {
    let observed = cached.observed.clone();
    if observed.kind != ExtensionKind::Skill {
        return Err(CommandError::keyed(
            "extension-invalid",
            "errors.extlib.observationNotSkillTakeover",
            "该发现结果不是 Skill，不能接管",
        ));
    }
    if !observed.managed_binding_ids.is_empty() {
        return Err(CommandError::keyed(
            "extension-already-managed",
            "errors.extlib.skillAlreadyManagedTakeover",
            "该 Skill 已由扩展库管理，无需接管",
        ));
    }
    let digest = observed.content_digest.clone().ok_or_else(|| {
        CommandError::keyed(
            "source-rejected",
            "errors.extlib.skillNoDigestTakeover",
            "该 Skill 没有可验证的内容摘要，不能安全接管",
        )
    })?;
    let parent = std::path::Path::new(&observed.path)
        .parent()
        .ok_or_else(|| CommandError::keyed("source-rejected", "errors.extlib.discoveredSkillPathInvalid", "发现到的 Skill 路径无效"))?;
    let scanned = sources::scan_local_source(parent, None).map_err(source_error)?;
    let candidate = scanned
        .into_iter()
        .find(|candidate| candidate.content_digest == digest)
        .ok_or_else(|| {
            CommandError::keyed(
                "observation-stale",
                "errors.extlib.skillChangedSinceScan",
                "该 Skill 内容已在扫描后变化；请重新扫描并确认",
            )
        })?;
    let target = takeover_target(store, &observed, false)?;
    Ok((
        candidate,
        TakeoverMaterial {
            observed,
            target,
            native_entry: None,
            entry_pointer: None,
        },
    ))
}
