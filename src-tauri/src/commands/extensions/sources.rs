//! Local skill sources: scanning a directory into cached candidates and
//! importing one candidate (or a discovered native skill/MCP) into the
//! library as host-scoped definitions.

use std::fs;

use asb_core::contracts::AppKind;
use asb_core::extensions::contracts::{
    ExtensionDefinition, ExtensionKind, ExtensionPayload, ObservedOrigin, SkillDefinition,
    SkillManifest, EXTENSIONS_SCHEMA_VERSION,
};
use asb_core::extensions::mcp::{
    import_claude_project_private_server, import_claude_server, import_codex_server,
};
use asb_core::extensions::validate::validate_definition;
use serde::Serialize;
use tauri::AppHandle;

use super::support::*;
use crate::commands::error::{blocking, state, CommandError};
use crate::extensions::discovery::DiscoveredPaths;
use crate::extensions::sources::{self, SkillCandidate};
use crate::extensions::store::ExtensionStore;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCandidateDto {
    candidate_id: String,
    digest: String,
    pub(super) name: String,
    description: Option<String>,
    file_count: usize,
    diagnostics: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<SkillSourceViewDto>,
}

pub(super) fn candidate_dto(candidate: &SkillCandidate) -> SkillCandidateDto {
    SkillCandidateDto {
        candidate_id: candidate_id(candidate),
        digest: candidate.content_digest.clone(),
        name: candidate.name.clone(),
        description: candidate.description.clone(),
        file_count: candidate
            .entries
            .iter()
            .filter(|entry| entry.kind == asb_core::extensions::validate::ContentEntryKind::File)
            .count(),
        diagnostics: candidate.diagnostics.clone(),
        source: candidate.resolved_commit.as_ref().map(|_| skill_source_view(candidate.source_ref())),
    }
}

pub(super) fn candidate_id(candidate: &SkillCandidate) -> String {
    let key = serde_json::to_vec(&(candidate.source_ref(), &candidate.content_digest))
        .expect("serializable Skill candidate identity");
    sha_hex(&key)
}

pub(super) fn cache_candidates(list: Vec<SkillCandidate>) {
    let mut cache = candidates().lock().expect("candidates");
    for candidate in list {
        cache.entry(candidate_id(&candidate)).or_insert(candidate);
    }
}

pub(super) fn cached_candidate(id: &str) -> Option<SkillCandidate> {
    candidates().lock().expect("candidates").get(id).cloned()
}

pub(super) fn detailed_source_error(error: sources::SourceError) -> CommandError {
    let code = match &error {
        sources::SourceError::Unreachable(_) => "source-unreachable",
        sources::SourceError::Rejected(_) => "source-rejected",
    };
    CommandError::new(code, error.to_string())
}

#[tauri::command]
pub async fn scan_local_skill_source(
    app: AppHandle,
    root: String,
) -> Result<Vec<SkillCandidateDto>, CommandError> {
    let _ = app;
    blocking(move || {
        let scanned =
            sources::scan_local_source(std::path::Path::new(&root), None).map_err(source_error)?;
        let dtos: Vec<SkillCandidateDto> = scanned.iter().map(candidate_dto).collect();
        cache_candidates(scanned);
        Ok(dtos)
    })
    .await
}

#[tauri::command]
pub async fn resolve_skill_source(
    repo: String,
    subpath: String,
    ref_name: Option<String>,
) -> Result<Vec<SkillCandidateDto>, CommandError> {
    blocking(move || {
        let fetch: sources::HttpFetch = &sources::http_fetch;
        let scanned = sources::resolve_github_source(&repo, &subpath, ref_name.as_deref(), fetch)
            .map_err(source_error)?;
        let dtos: Vec<SkillCandidateDto> = scanned.iter().map(candidate_dto).collect();
        cache_candidates(scanned);
        Ok(dtos)
    })
    .await
}

#[tauri::command]
pub async fn scan_skill_zip(path: String) -> Result<Vec<SkillCandidateDto>, CommandError> {
    blocking(move || {
        let scanned =
            sources::scan_zip_source(std::path::Path::new(&path)).map_err(detailed_source_error)?;
        let dtos = scanned.iter().map(candidate_dto).collect();
        cache_candidates(scanned);
        Ok(dtos)
    })
    .await
}

/// Imports one cached candidate into the library: the immutable content
/// version first, then a definition pointing at it. Nothing is deployed.
fn import_cached_skill_candidate(
    store: &ExtensionStore,
    candidate_id: &str,
    name: String,
    host_scoped: Option<AppKind>,
) -> Result<ExtensionMutationDto, CommandError> {
    let candidate = cached_candidate(candidate_id).ok_or_else(|| {
        CommandError::keyed("candidate-expired", "errors.extlib.candidateExpiredRescan", "候选内容已过期；请重新扫描来源")
    })?;
    import_candidate_content(store, &candidate, name, host_scoped)
}

pub(super) fn import_candidate_content(
    store: &ExtensionStore,
    candidate: &SkillCandidate,
    name: String,
    host_scoped: Option<AppKind>,
) -> Result<ExtensionMutationDto, CommandError> {
    let manifest = candidate_manifest(candidate)?;
    let compatibility = asb_core::extensions::skill::claude_compatibility_notes(&manifest);
    if let Some(existing) = store
        .list_definitions()
        .map_err(store_error)?
        .into_iter()
        .find(|definition| {
            matches!(&definition.payload, ExtensionPayload::Skill(skill)
                if skill.content_digest == candidate.content_digest && skill.host_scoped == host_scoped)
        })
    {
        return Ok(extension_mutation(&existing));
    }
    let definition = ExtensionDefinition {
        mcp_metadata: None,
        mcp_config_revision: None,
        schema_version: EXTENSIONS_SCHEMA_VERSION,
        id: new_id("ext"),
        name,
        revision: 1,
        created_at: now(),
        updated_at: now(),
        payload: ExtensionPayload::Skill(SkillDefinition {
            content_digest: candidate.content_digest.clone(),
            manifest,
            source: Some(candidate.source_ref()),
            host_scoped,
            compatibility,
            dependencies: Vec::new(),
        }),
    };
    validate_definition(&definition)
        .map_err(|error| CommandError::new("extension-invalid", error.message))?;
    store
        .save_skill_version(
            &definition.id,
            &candidate.content_digest,
            &candidate.entries,
        )
        .map_err(store_error)?;
    store.create_definition(&definition).map_err(store_error)?;
    Ok(extension_mutation(&definition))
}

fn candidate_manifest(candidate: &SkillCandidate) -> Result<SkillManifest, CommandError> {
    if !candidate.diagnostics.is_empty() {
        return Err(CommandError::new(
            "source-rejected",
            candidate.diagnostics.join("；"),
        ));
    }
    manifest_from_entries(&candidate.entries)
}

pub(super) fn manifest_from_entries(entries: &[asb_core::extensions::skill::ContentEntry]) -> Result<SkillManifest, CommandError> {
    let text = entries
        .iter()
        .find(|entry| entry.relative_path == "SKILL.md")
        .and_then(|entry| std::str::from_utf8(&entry.bytes).ok())
        .ok_or_else(|| {
            CommandError::keyed(
                "source-rejected",
                "errors.extlib.candidateMissingSkillMd",
                "候选内容缺少 UTF-8 格式的 SKILL.md",
            )
        })?;
    match asb_core::extensions::skill::extract_manifest(text) {
        asb_core::extensions::skill::ManifestExtraction::Parsed(manifest) => Ok(manifest),
        _ => Err(CommandError::keyed(
            "source-rejected",
            "errors.extlib.candidateSkillMdNoFrontmatter",
            "候选内容缺少可解析的 SKILL.md frontmatter",
        )),
    }
}

#[tauri::command]
pub async fn import_skill_candidate(
    app: AppHandle,
    candidate_id: String,
    name: String,
    host_scoped: Option<AppKind>,
) -> Result<ExtensionMutationDto, CommandError> {
    blocking(move || {
        let state = state(&app)?;
        let store = extension_store(&state);
        import_cached_skill_candidate(&store, &candidate_id, name, host_scoped)
    })
    .await
}

/// Imports a currently displayed local Skill without sending its directory
/// path through the renderer. The observation id is invalidated by the next
/// discovery pass; the directory is scanned again and must still match the
/// originally observed content digest.
// Tauri resolves this entrypoint through its generated runtime handler; the
// unit-test crate reaches it only through the HTTP command dispatcher.
#[cfg_attr(test, allow(dead_code))]
#[tauri::command]
pub async fn import_discovered_skill(
    app: AppHandle,
    observation_id: String,
) -> Result<ExtensionMutationDto, CommandError> {
    blocking(move || {
        let observed = discovered_observations()
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
            })?
            .observed;
        if observed.kind != ExtensionKind::Skill {
            return Err(CommandError::keyed(
                "extension-invalid",
                "errors.extlib.observationNotSkillImport",
                "该发现结果不是 Skill，不能按 Skill 导入",
            ));
        }
        let digest = observed.content_digest.clone().ok_or_else(|| {
            CommandError::keyed(
                "source-rejected",
                "errors.extlib.skillNoDigestImport",
                "该 Skill 没有可验证的内容摘要，不能安全导入",
            )
        })?;
        let root = std::path::Path::new(&observed.path)
            .parent()
            .ok_or_else(|| CommandError::keyed("source-rejected", "errors.extlib.discoveredSkillPathInvalid", "发现到的 Skill 路径无效"))?;
        let scanned = sources::scan_local_source(root, None).map_err(source_error)?;
        let candidate = scanned
            .iter()
            .find(|candidate| candidate.content_digest == digest)
            .ok_or_else(|| {
                CommandError::keyed(
                    "observation-stale",
                    "errors.extlib.skillChangedSinceScan",
                    "该 Skill 内容已在扫描后变化；请重新扫描并确认",
                )
            })?
            .clone();
        let state = state(&app)?;
        let store = extension_store(&state);
        import_candidate_content(&store, &candidate, observed.name, Some(observed.client))
    })
    .await
}

/// Imports a discovered native MCP server into the library without exposing
/// its document or raw parameters to the renderer. This copies a definition
/// only; it never claims ownership of the live native entry.
#[allow(dead_code)] // Tauri invokes this command through its generated runtime handler.
#[tauri::command]
pub async fn import_discovered_mcp(
    app: AppHandle,
    observation_id: String,
) -> Result<ExtensionMutationDto, CommandError> {
    blocking(move || {
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
        let observed = cached.observed;
        if observed.kind != ExtensionKind::Mcp {
            return Err(CommandError::keyed(
                "extension-invalid",
                "errors.extlib.observationNotMcpImport",
                "该发现结果不是 MCP 服务，不能按 MCP 导入",
            ));
        }
        if !observed.managed_binding_ids.is_empty() {
            return Err(CommandError::keyed(
                "extension-already-managed",
                "errors.extlib.mcpAlreadyManagedImport",
                "该 MCP 服务已由扩展库管理，无需再次导入",
            ));
        }
        let document =
            read_unchanged_mcp_document(&observed.path, cached.document_digest.as_deref())?;
        let discovery_paths = DiscoveredPaths::from_env()
            .map_err(|error| CommandError::new("app-state-unavailable", error))?;
        let claude_user_document = crate::extensions::paths::claude_user_json_path(
            &discovery_paths.home,
            discovery_paths.claude_dir.as_deref(),
        );
        let payload = match (&observed.client, &observed.origin) {
            (AppKind::Claude, ObservedOrigin::ProjectRoot { project_path })
                if std::path::Path::new(&observed.path) == claude_user_document =>
            {
                import_claude_project_private_server(&document, project_path, &observed.name)
            }
            (AppKind::Codex, _) => import_codex_server(&document, &observed.name),
            (AppKind::Claude, _) => import_claude_server(&document, &observed.name),
        }
        .map_err(|error| CommandError::new("source-rejected", error.to_string()))?;

        let state = state(&app)?;
        let store = extension_store(&state);
        if let Some(existing) = store
            .list_definitions()
            .map_err(store_error)?
            .into_iter()
            .find(|definition| {
                definition.name == observed.name
                    && definition.payload == ExtensionPayload::Mcp(payload.clone())
            })
        {
            return Ok(extension_mutation(&existing));
        }
        let definition = ExtensionDefinition {
            mcp_metadata: None,
            mcp_config_revision: None,
            schema_version: EXTENSIONS_SCHEMA_VERSION,
            id: new_id("ext"),
            name: observed.name,
            revision: 1,
            created_at: now(),
            updated_at: now(),
            payload: ExtensionPayload::Mcp(payload),
        };
        validate_definition(&definition)
            .map_err(|error| CommandError::new("source-rejected", error.message))?;
        store.create_definition(&definition).map_err(store_error)?;
        Ok(extension_mutation(&definition))
    })
    .await
}

fn read_unchanged_mcp_document(
    path: &str,
    expected_digest: Option<&str>,
) -> Result<String, CommandError> {
    let document = fs::read(path).map_err(|_| {
        CommandError::keyed(
            "observation-stale",
            "errors.extlib.mcpDocumentUnreadable",
            "发现到的 MCP 配置已无法读取；请重新扫描并确认",
        )
    })?;
    let digest = sha_hex(&document);
    if expected_digest != Some(digest.as_str()) {
        return Err(CommandError::keyed(
            "observation-stale",
            "errors.extlib.mcpChangedSinceScan",
            "该 MCP 配置已在扫描后变化；请重新扫描并确认",
        ));
    }
    String::from_utf8(document)
        .map_err(|_| CommandError::keyed("source-rejected", "errors.extlib.mcpNotTextImport", "MCP 配置不是有效文本，不能安全导入"))
}

#[cfg(test)]
mod source_tests {
    use super::*;

    #[test]
    fn candidate_import_preserves_source_ref_and_reuses_immutable_library_content() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("source");
        fs::create_dir(&root).unwrap();
        let name = format!("source-{}", uuid::Uuid::new_v4());
        let bytes = format!("---\nname: {name}\ndescription: Source import test\n---\n");
        fs::write(root.join("SKILL.md"), &bytes).unwrap();
        let mut candidate = sources::scan_local_source(&root, None).unwrap().remove(0);
        candidate.source_identity = "org/repo".into();
        candidate.subpath = "skills/helper".into();
        candidate.ref_name = Some("feature/skills".into());
        candidate.resolved_commit = Some("0123456789abcdef0123456789abcdef01234567".into());
        let digest = candidate.content_digest.clone();
        let id = candidate_id(&candidate);
        cache_candidates(vec![candidate.clone()]);
        fs::write(root.join("SKILL.md"), "changed after scan").unwrap();
        let store = ExtensionStore::from_root(temp.path().join("library"));
        let first = import_cached_skill_candidate(&store, &id, name.clone(), None).unwrap();
        let second = import_cached_skill_candidate(&store, &id, name, None).unwrap();
        assert_eq!(first.id, second.id);
        let definitions = store.list_definitions().unwrap();
        assert_eq!(definitions.len(), 1);
        let ExtensionPayload::Skill(skill) = &definitions[0].payload else {
            panic!("not a Skill")
        };
        let source = skill.source.as_ref().unwrap();
        assert_eq!(source.source_id, "org/repo");
        assert_eq!(source.subpath, "skills/helper");
        assert_eq!(source.ref_name.as_deref(), Some("feature/skills"));
        assert_eq!(source.resolved_commit, candidate.resolved_commit);
        let stored = store.load_skill_version(&first.id, &digest).unwrap();
        assert_eq!(stored, candidate.entries);
    }

    #[test]
    fn malformed_candidate_cannot_be_imported_through_the_command_boundary() {
        let temp = tempfile::tempdir().unwrap();
        let content = format!("---\ndescription: {}\n---\n", uuid::Uuid::new_v4());
        fs::write(temp.path().join("SKILL.md"), content).unwrap();
        let candidate = sources::scan_local_source(temp.path(), None)
            .unwrap()
            .remove(0);
        let id = candidate_id(&candidate);
        cache_candidates(vec![candidate]);
        let store = ExtensionStore::from_root(temp.path().join("library"));
        assert!(import_cached_skill_candidate(&store, &id, "invalid".into(), None).is_err());
        assert!(!temp.path().join("library").exists());
    }
}

