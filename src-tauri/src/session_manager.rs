//! Local session discovery, controlled resume, and explicit deletion for the
//! two clients Agent Switchboard owns.
//!
//! Discovery and transcript reads never index or modify client data. An
//! explicit resume request only starts the client's fixed CLI command in a
//! new terminal after resolving an approved local session source. An
//! explicit delete removes the resolved raw records from approved roots.
//! Codex's registered native history remains owned by Codex and read-only.
//! Explicit workflow conversion saves complete application-owned copies; all
//! consumers resolve the same local state root and shared message contract.

pub(crate) mod codex_titles;
pub(crate) mod parser;
pub(crate) mod records;
mod content;
mod images;
mod native;
mod timeline;
mod jsonl;
mod layout;
pub(crate) use layout::initialize_storage;
mod sources;
pub(crate) use sources::{collect as collect_session_files, SessionSource};
mod resume;
mod bookmarks;
mod search;
mod export;
mod organization;
mod messages;
mod text;
mod copy;
mod conversion;
pub use copy::{load_copy_text, SessionCopySource};
pub use conversion::{convert_session_sources, SessionConversionOutcome};

pub use bookmarks::{delete_bookmark, load_bookmark_content, save_bookmark, search_bookmarks,
    BookmarkPage, BookmarkSearchRequest, BookmarkSummary};
pub use messages::{load_content, load_page, ContentPage, MessageContentRequest, MessagePage, MessagePageRequest};
pub use export::export_markdown;
pub use search::{search_sessions, SessionSearchPage, SessionSearchRequest};
pub use organization::{update_session_organization, SessionOrganizationChange};

use asb_core::contracts::AppKind;
#[cfg(test)]
use parser::collect_messages;
use resume::{launch_terminal, resume_arguments, resume_command};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[cfg(test)]
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const TITLE_LIMIT: usize = 120;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionMeta {
    pub app: AppKind,
    pub session_id: String,
    pub title: String,
    pub summary: String,
    pub project_dir: Option<String>,
    pub created_at: Option<String>,
    pub last_active_at: Option<String>,
    pub resume_command: String,
    pub alias: Option<String>,
    pub pinned: bool,
    pub tags: Vec<String>,
    pub parent_session_id: Option<String>,
    pub can_delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub at: Option<String>,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIssue {
    pub app: AppKind,
    pub message: String,
    pub conversion_id: Option<String>,
}

impl SessionIssue {
    pub(super) fn new(app: AppKind, message: String) -> Self { Self { app, message, conversion_id: None } }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResume {
    pub command: String,
    pub used_project_dir: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionDeleteRequest {
    pub app: AppKind,
    pub session_id: String,
}

/// One line of a batch deletion: the record either went away or the reason
/// it stayed, never a silent skip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDeleteOutcome {
    pub app: AppKind,
    pub session_id: String,
    pub deleted: bool,
    pub error: Option<String>,
}


pub fn load_metadata(root: &Path, app: AppKind, session_id: &str) -> Result<SessionMeta, String> {
    let mut meta = resolve_session(root, app, session_id)?.meta;
    organization::apply(&mut meta, &organization::load(root)?);
    Ok(meta)
}

/// Opens the selected local session in a new terminal window. The renderer
/// supplies only the supported client plus a validated session id; the source
/// path and working directory stay resolved inside this module.
pub fn resume_session(root: &Path, app: AppKind, session_id: &str) -> Result<SessionResume, String> {
    let source = resolve_session(root, app, session_id)?;
    let project_dir = source
        .meta
        .project_dir
        .as_deref()
        .filter(|path| Path::new(path).is_dir());
    let resume_id = if app == AppKind::Claude { source.meta.parent_session_id.as_deref().unwrap_or(session_id) } else { session_id };
    launch_terminal(&resume_arguments(app, resume_id), project_dir)?;

    Ok(SessionResume {
        command: resume_command(app, resume_id),
        used_project_dir: project_dir.is_some(),
    })
}

/// Permanently removes the local session record resolved from the approved
/// roots. The renderer supplies only the supported client plus a validated
/// session id; the resolved path never crosses the IPC boundary. Claude Code
/// keeps an optional directory named after the transcript stem beside the
/// record; it belongs to this session's identity and is removed with it.
pub fn delete_session(root: &Path, app: AppKind, session_id: &str) -> Result<(), String> {
    let (sources, issues) = scan_sources(root)?;
    let source = locate(&sources, &issues, app, session_id)?;
    let ids = removed_organization_ids(source, &sources);
    organization::delete_with_sources(root, app, &ids, || remove_source(source))
}

/// Deletes several records against one scan. Each request resolves and
/// fails on its own line; a repeated (client, id) pair is answered once.
/// Only an unreadable root set aborts the batch before anything is removed.
pub fn delete_sessions(
    root: &Path,
    requests: &[SessionDeleteRequest],
) -> Result<Vec<SessionDeleteOutcome>, String> {
    let (sources, issues) = scan_sources(root)?;
    Ok(delete_resolved(root, &sources, &issues, requests))
}

fn delete_resolved(
    root: &Path,
    sources: &[SessionSource],
    issues: &[SessionIssue],
    requests: &[SessionDeleteRequest],
) -> Vec<SessionDeleteOutcome> {
    let mut seen = HashSet::new();
    let mut outcomes = Vec::with_capacity(requests.len());
    for request in requests {
        if !seen.insert((request.app, request.session_id.clone())) {
            continue;
        }
        let source = locate(sources, issues, request.app, &request.session_id);
        let result = source.as_ref().map_err(|error| error.clone()).and_then(|source| {
            let ids = removed_organization_ids(source, sources);
            organization::delete_with_sources(root, request.app, &ids,
                || remove_source(source))
        });
        let deleted = result.is_ok() || source.is_ok_and(|source| source.meta.can_delete
            && ((!source.paths.is_empty() && source.paths.iter().all(|path| matches!(path.try_exists(), Ok(false))))
                || source.converted.as_ref().is_some_and(|path| matches!(path.try_exists(), Ok(false)))));
        outcomes.push(SessionDeleteOutcome {
            app: request.app,
            session_id: request.session_id.clone(),
            deleted,
            error: result.err(),
        });
    }
    outcomes
}

fn remove_session_source(app: AppKind, path: &Path) -> Result<(), String> {
    if app == AppKind::Claude {
        if let Some(stem) = path.file_stem() {
            let sidecar = path.with_file_name(stem);
            if sidecar.is_dir() {
                fs::remove_dir_all(&sidecar).map_err(|error| {
                    format!("无法删除会话附属目录 {}: {error}", sidecar.display())
                })?;
            }
        }
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("无法删除会话记录 {}: {error}", path.display())),
    }
}

fn removed_organization_ids(source: &SessionSource, sources: &[SessionSource]) -> Vec<String> {
    let mut ids = vec![source.meta.session_id.clone()];
    if source.meta.app != AppKind::Claude { return ids; }
    let directories: Vec<_> = source.paths.iter().filter_map(|path| path.file_stem().map(|stem| path.with_file_name(stem))).collect();
    for candidate in sources {
        if candidate.meta.app != AppKind::Claude || candidate.meta.session_id == source.meta.session_id || candidate.paths.is_empty() { continue; }
        if candidate.paths.iter().all(|path| directories.iter().any(|directory| path.starts_with(directory))) {
            ids.push(candidate.meta.session_id.clone());
        }
    }
    ids
}

fn remove_source(source: &SessionSource) -> Result<(), String> {
    if let Some(path) = &source.converted { return conversion::remove(path); }
    if !source.meta.can_delete {
        return Err("Codex 原生历史由 Codex 管理，请在 Codex 中删除此会话；本应用只读原生历史库".into());
    }
    for path in &source.paths { remove_session_source(source.meta.app, path)?; }
    Ok(())
}

pub(crate) fn resolve_session(root: &Path, app: AppKind, session_id: &str) -> Result<SessionSource, String> {
    let (sources, issues) = scan_sources(root)?;
    locate(&sources, &issues, app, session_id).cloned()
}

fn locate<'a>(
    sources: &'a [SessionSource],
    issues: &[SessionIssue],
    app: AppKind,
    session_id: &str,
) -> Result<&'a SessionSource, String> {
    if !parser::valid_session_id(session_id) {
        return Err("会话 ID 无效".to_string());
    }
    sources
        .iter()
        .find(|source| source.meta.app == app && source.meta.session_id == session_id)
        .ok_or_else(|| {
            issues
                .first()
                .map(|issue| issue.message.clone())
                .unwrap_or_else(|| "找不到指定会话；请刷新会话列表后重试".to_string())
        })
}

pub(crate) fn scan_sources(root: &Path) -> Result<(Vec<SessionSource>, Vec<SessionIssue>), String> {
    let (mut sources, mut issues) = scan_native_sources()?;
    conversion::include(root, &mut sources, &mut issues);
    Ok((sources, issues))
}

pub(crate) fn scan_native_sources() -> Result<(Vec<SessionSource>, Vec<SessionIssue>), String> {
    sources::scan()
}

/// The Codex root every Codex-owned sidecar (sessions, rename index, state
/// database) hangs off; honours the same `CODEX_HOME` as the config file.
pub(crate) fn codex_root() -> Result<PathBuf, String> {
    let codex = crate::local_state::LocalState::user_config_path(AppKind::Codex)?;
    codex
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Codex 配置目录无效".to_string())
}

/// The only approved local JSONL roots. Both session browsing and usage
/// aggregation consume this list so neither feature accepts a renderer path.
pub(crate) fn session_roots() -> Result<Vec<(AppKind, PathBuf)>, String> {
    let claude = crate::local_state::LocalState::user_config_path(AppKind::Claude)?;
    let claude_root = claude.parent().ok_or("Claude 配置目录无效")?;
    let mut roots = crate::local_state::codex_paths::session_roots(&codex_root()?)
        .into_iter()
        .map(|path| (AppKind::Codex, path))
        .collect::<Vec<_>>();
    roots.push((AppKind::Claude, claude_root.join("projects")));
    Ok(roots)
}

#[cfg(test)]
fn scan_session_source_roots(
    roots: &[(AppKind, PathBuf)],
    titles: &HashMap<String, String>,
) -> (Vec<SessionSource>, Vec<SessionIssue>) {
    let mut indexed = HashMap::new();
    let mut issues = Vec::new();
    sources::scan_roots(roots, &mut indexed, &mut issues);
    for source in indexed.values_mut() {
        source.paths.sort();
        if source.meta.app == AppKind::Codex {
            if let Some(title) = titles.get(&source.meta.session_id) { source.meta.title = parser::clamp_title(title); }
        }
    }
    (indexed.into_values().collect(), issues)
}

#[cfg(test)]
pub(crate) fn fixture_source(path: &Path, app: AppKind, id: &str) -> SessionSource {
    SessionSource { paths: vec![path.to_path_buf()], native_history: None, converted: None,
        meta: SessionMeta { app, session_id: id.into(), title: id.into(), summary: String::new(),
            project_dir: None, created_at: None, last_active_at: None,
            resume_command: resume_command(app, id), alias: None, pinned: false, tags: Vec::new(),
            parent_session_id: None, can_delete: true } }
}

#[cfg(test)]
mod tests;
