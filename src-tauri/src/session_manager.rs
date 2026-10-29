//! Local session discovery, controlled resume, and explicit deletion for the
//! two clients Agent Switchboard owns.
//!
//! Client discovery is read-only. Claude records and referenced assets are
//! preserved in the application's session library before consumers read them.
//! Explicit resume reconstructs a managed transcript and starts the fixed CLI
//! with current credentials. Explicit deletion removes original and saved data.
//! Codex's registered native history remains owned by Codex and read-only.
//! Explicit workflow conversion saves complete application-owned copies; all
//! consumers resolve the same local state root and shared message contract.

pub(crate) mod codex_titles;
pub(crate) mod parser;
pub(crate) mod records;
pub(crate) mod record_file;
pub(crate) mod diagnostics;
mod content;
mod images;
mod native;
mod timeline;
mod jsonl;
mod layout;
pub(crate) use layout::initialize_storage;
mod sources;
pub(crate) mod read_index;
pub(crate) mod history;
pub(crate) use sources::{collect as collect_session_files, SessionSource};
mod resume;
mod bookmarks;
mod search;
pub(crate) mod search_tasks;
mod read_observer;
mod export;
mod organization;
mod messages;
mod text;
mod copy;
mod conversion;
#[cfg(test)]
mod black_box;
pub use copy::{load_copy_text, SessionCopySource};

pub use bookmarks::{delete_bookmark, load_bookmark_content, save_bookmark, search_bookmarks,
    BookmarkPage, BookmarkSearchRequest, BookmarkSummary};
pub use messages::{load_content, load_page, ContentPage, MessageContentRequest, MessagePage, MessagePageRequest};
pub use export::export_markdown;
pub use history::PurgePlan;
pub use search::{SessionSearchPage, SessionSearchRequest};
#[cfg(test)]
pub use search::search_sessions;
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
    pub resume_command: Option<String>,
    pub alias: Option<String>,
    pub pinned: bool,
    pub tags: Vec<String>,
    pub parent_session_id: Option<String>,
    pub can_delete: bool,
    pub preservation: Option<history::Preservation>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIssue {
    pub app: AppKind,
    pub message: String,
    #[serde(skip)]
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
    let mut source = resolve_session(root, app, session_id)?;
    if app == AppKind::Claude {
        if let Some(parent) = &source.meta.parent_session_id {
            source = resolve_session(root, app, parent)?;
        }
    }
    let project_dir = source
        .meta
        .project_dir
        .as_deref()
        .filter(|path| Path::new(path).is_dir());
    let target = if app == AppKind::Claude {
        history::prepare_resume(root, &source)?.to_str().ok_or("恢复路径无法无损传递给客户端")?.to_string()
    } else { session_id.to_string() };
    launch_terminal(&resume_arguments(app, &target), project_dir)?;

    Ok(SessionResume {
        command: resume_command(app, &target),
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
    organization::delete_with_sources(root, app, &ids, || remove_source(root, source, &ids, None))
}

/// Deletes several records against one scan. Each request resolves and
/// fails on its own line; a repeated (client, id) pair is answered once.
/// Only an unreadable root set aborts the batch before anything is removed.
pub fn delete_sessions(
    root: &Path,
    requests: &[SessionDeleteRequest],
) -> Result<Vec<SessionDeleteOutcome>, String> {
    let (sources, issues) = scan_sources(root)?;
    Ok(delete_resolved(root, &sources, &issues, requests, None))
}

/// One bulk session reclamation line: the sessions that went away and the
/// reasons others stayed, never a silent skip.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPurgeOutcome {
    pub removed: u64,
    pub warnings: Vec<asb_core::contracts::LocalizedMessage>,
}

/// Reclaims every Claude preservation session whose latest activity is older
/// than the cutoff by running the regular per-session deletion path, then
/// releases the freed file space. The cutoff is the whole contract: the
/// selection is re-derived at execution time, so the preview numbers are
/// advisory and a concurrent capture only shifts the boundary, never the
/// semantics.
pub fn purge_claude_sessions(root: &Path, cutoff: &str) -> Result<SessionPurgeOutcome, String> {
    let date = history::purge_cutoff(cutoff)?;
    let (sources, issues) = scan_sources(root)?;
    purge_resolved(root, &sources, &issues, date)
}

fn purge_resolved(
    root: &Path, sources: &[SessionSource], issues: &[SessionIssue], date: chrono::DateTime<chrono::Utc>,
) -> Result<SessionPurgeOutcome, String> {
    let ids = history::stale_session_ids(root, date)?;
    if ids.is_empty() { return Ok(SessionPurgeOutcome { removed: 0, warnings: Vec::new() }); }
    let requests: Vec<SessionDeleteRequest> = ids.into_iter()
        .map(|session_id| SessionDeleteRequest { app: AppKind::Claude, session_id }).collect();
    let mut warnings: Vec<asb_core::contracts::LocalizedMessage> = Vec::new();
    let mut removed = 0_u64;
    for outcome in delete_resolved(root, sources, issues, &requests, Some(date)) {
        if outcome.deleted { removed += 1; }
        else if let Some(error) = outcome.error {
            let detail = format!("{}：{error}", outcome.session_id);
            warnings.push(asb_core::contracts::LocalizedMessage::new(
                "sessions.purge.partialFailed", serde_json::json!({"detail": detail}),
                format!("部分会话回收未完成：{detail}"),
            ));
        }
    }
    if let Err(error) = history::reclaim_space(root) { warnings.push(asb_core::contracts::LocalizedMessage::new(
        "sessions.purge.compactFailed", serde_json::json!({"detail": error.clone()}),
        format!("已删除会话记录，但保留库空间压缩未完成：{error}"),
    )); }
    Ok(SessionPurgeOutcome { removed, warnings })
}

/// Preview for `purge_claude_sessions`: how many preserved sessions and how
/// much preservation bytes the cutoff would reclaim right now.
pub fn preview_session_purge(root: &Path, cutoff: &str) -> Result<history::PurgePlan, String> {
    history::preview_purge(root, cutoff)
}

fn delete_resolved(
    root: &Path,
    sources: &[SessionSource],
    issues: &[SessionIssue],
    requests: &[SessionDeleteRequest],
    cutoff: Option<chrono::DateTime<chrono::Utc>>,
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
                || remove_source(root, source, &ids, cutoff))
        });
        let deleted = result.is_ok() || source.is_ok_and(|source| source.retained.is_empty() && source.meta.can_delete
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

fn remove_session_source(path: &Path) -> Result<(), String> {
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
        if candidate.meta.app == AppKind::Claude && candidate.converted.is_none()
            && candidate.meta.parent_session_id.as_deref() == Some(source.meta.session_id.as_str()) {
            ids.push(candidate.meta.session_id.clone());
            continue;
        }
        if candidate.meta.app != AppKind::Claude || candidate.meta.session_id == source.meta.session_id || candidate.paths.is_empty() { continue; }
        if candidate.paths.iter().all(|path| directories.iter().any(|directory| path.starts_with(directory))) {
            ids.push(candidate.meta.session_id.clone());
        }
    }
    ids
}

fn remove_source(
    root: &Path, source: &SessionSource, ids: &[String], cutoff: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<(), String> {
    if cutoff.is_some() && (source.meta.app != AppKind::Claude
        || source.retained.is_empty() || source.converted.is_some()) {
        return Err("会话来源已变化，不再属于本次保留库清理范围".into());
    }
    if let Some(path) = &source.converted { return conversion::remove(root, path, &source.meta.session_id); }
    if !source.retained.is_empty() { return history::delete(root, ids, cutoff); }
    if !source.meta.can_delete {
        return Err("Codex 原生历史由 Codex 管理，请在 Codex 中删除此会话；本应用只读原生历史库".into());
    }
    if source.meta.app == AppKind::Claude { return Err("Claude 会话必须通过本机保留库管理删除".into()); }
    for path in &source.paths { remove_session_source(path)?; }
    Ok(())
}

pub(crate) fn resolve_session(root: &Path, app: AppKind, session_id: &str) -> Result<SessionSource, String> {
    if !parser::valid_session_id(session_id) { return Err("会话 ID 无效".into()); }
    let (sources, issues) = scan_filtered_sources(root, Some(app), Some(session_id))?;
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
    scan_filtered_sources(root, None, None)
}

pub(crate) fn scan_filtered_sources(root: &Path, app: Option<AppKind>, session: Option<&str>) -> Result<(Vec<SessionSource>, Vec<SessionIssue>), String> {
    scan_filtered_sources_observed(root, app, session, &|| Ok(()))
}

pub(crate) fn scan_filtered_sources_observed(root: &Path, app: Option<AppKind>, session: Option<&str>,
    checkpoint: &dyn Fn() -> Result<(), String>) -> Result<(Vec<SessionSource>, Vec<SessionIssue>), String> {
    let (mut sources, mut issues) = sources::scan_observed(root, app, session, checkpoint)?;
    if app != Some(AppKind::Codex) && session.is_none_or(|id| id.starts_with("workflow-")) {
        if let Err(message) = conversion::include_observed(root, &mut sources, &mut issues, checkpoint) {
            issues.push(SessionIssue::new(AppKind::Claude, message));
        }
    }
    checkpoint()?;
    sources.retain(|source| app.is_none_or(|app| source.meta.app == app)
        && session.is_none_or(|id| source.meta.session_id == id));
    if session.is_none() {
        let area = match app { Some(AppKind::Codex) => "Codex 会话来源", Some(AppKind::Claude) => "Claude 会话来源", None => "会话来源" };
        diagnostics::report(root, area, issues.iter().map(|issue| format!("{}: {}", issue.app.label(), issue.message)));
    }
    Ok((sources, issues))
}

pub(crate) fn scan_usage_sources(root: &Path) -> Result<(Vec<SessionSource>, Vec<SessionIssue>), String> {
    scan_filtered_sources(root, None, None)
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

/// Codex owns its raw source roots. Claude sources are owned by its persistent library.
pub(crate) fn codex_session_roots() -> Result<Vec<PathBuf>, String> {
    Ok(crate::local_state::codex_paths::session_roots(&codex_root()?).into_iter().collect())
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
    SessionSource { paths: vec![path.to_path_buf()], native_history: None, converted: None, retained: Vec::new(),
        index_root: path.parent().unwrap_or(Path::new(".")).join(".asb-read-index"),
        meta: SessionMeta { app, session_id: id.into(), title: id.into(), summary: String::new(),
            project_dir: None, created_at: None, last_active_at: None,
            resume_command: Some(resume_command(app, id)), alias: None, pinned: false, tags: Vec::new(),
            parent_session_id: None, can_delete: true, preservation: None } }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod compressed_tests;
