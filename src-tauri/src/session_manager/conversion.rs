mod store;

use super::{content, parser, records, sources, SessionIssue, SessionMessage, SessionMeta, SessionSource};
use asb_core::contracts::AppKind;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::File;
#[cfg(test)]
use std::fs;
use std::io::Read;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static CONVERSION_GATE: Mutex<()> = Mutex::new(());

pub(super) struct Candidate {
    path: PathBuf,
    source_key: String,
    revision: String,
    parent_id: String,
    workflow: String,
    parent_file: PathBuf,
}

impl Candidate {
    fn id(&self) -> String { format!("{}:{}", self.source_key, self.revision) }

    fn issue(&self) -> SessionIssue {
        SessionIssue { app: AppKind::Claude, conversion_id: Some(self.id()),
            message: format!("{} 是 workflow 事件日志，可完整转换为可阅读记录", self.path.display()) }
    }
}

#[derive(Debug)]
struct SessionConversionOutcome {
    pub converted: usize,
    pub records: usize,
    pub failures: Vec<SessionIssue>,
}

pub(super) fn is_workflow(path: &Path) -> bool { workflow_location(path).is_some() }

fn workflow_location(path: &Path) -> Option<(String, String, PathBuf)> {
    if path.file_name()? != "journal.jsonl" { return None; }
    let workflow = path.parent()?;
    let workflows = workflow.parent()?;
    let subagents = workflows.parent()?;
    let parent = subagents.parent()?;
    if workflows.file_name()? != "workflows" || subagents.file_name()? != "subagents" { return None; }
    let parent_id = parent.file_name()?.to_str()?;
    let name = workflow.file_name()?.to_str()?;
    if !parser::valid_session_id(parent_id) || !parser::valid_session_id(name) { return None; }
    Some((parent_id.into(), name.into(), parent.with_extension("jsonl")))
}

fn candidate(path: &Path) -> Result<Candidate, String> {
    let (parent_id, workflow, parent_file) = workflow_location(path).ok_or("不是受支持的 workflow 来源")?;
    Ok(Candidate { path: path.into(), source_key: source_key(path),
        revision: fingerprint(path)?, parent_id, workflow, parent_file })
}

fn source_key(path: &Path) -> String {
    let normalized: PathBuf = path.components().collect();
    format!("{:x}", Sha256::digest(normalized.as_os_str().as_encoded_bytes()))
}

pub(super) fn pending_issue(path: &Path) -> Result<SessionIssue, String> { Ok(candidate(path)?.issue()) }

fn fingerprint(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| format!("无法读取 {}: {error}", path.display()))?;
    let mut hash = Sha256::new();
    let mut bytes = [0; 64 * 1024];
    loop {
        let count = file.read(&mut bytes).map_err(|error| format!("无法读取 {}: {error}", path.display()))?;
        if count == 0 { break; }
        hash.update(&bytes[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub(super) fn include(
    root: &Path,
    sources: &mut Vec<SessionSource>,
    issues: &mut Vec<SessionIssue>,
) {
    if let Err(error) = include_observed(root, sources, issues, &|| Ok(())) {
        issues.push(SessionIssue::new(AppKind::Claude, error));
    }
}

pub(super) fn include_observed(root: &Path, sources: &mut Vec<SessionSource>, issues: &mut Vec<SessionIssue>,
    checkpoint: &dyn Fn() -> Result<(), String>) -> Result<(), String> {
    checkpoint()?;
    let deleted = super::history::deleted_ids(root)?;
    issues.retain(|issue| issue.conversion_id.as_deref().is_none_or(|id| !suppressed(id, &deleted)));
    let (mut converted, mut failures) = store::list(root);
    let ready: HashSet<_> = converted.iter().map(|(header, _)| format!("{}:{}", header.source_key, header.revision)).collect();
    let pending: Vec<_> = issues.iter().filter_map(|issue| issue.conversion_id.as_ref())
        .filter(|id| !ready.contains(*id)).cloned().collect::<HashSet<_>>().into_iter().collect();
    if !pending.is_empty() {
        match repair_sources(root, &pending, checkpoint) {
            Ok(outcome) => {
                if outcome.converted > 0 {
                    log::info!("已自动整理 {} 个 Claude workflow，{} 条记录", outcome.converted, outcome.records);
                    let refreshed = store::list(root);
                    converted = refreshed.0;
                    failures.extend(refreshed.1);
                }
                issues.extend(outcome.failures);
            }
            Err(error) => failures.push(error),
        }
    }
    let mut completed = HashSet::new();
    let mut known: HashSet<_> = sources.iter().map(|source| (source.meta.app, source.meta.session_id.clone())).collect();
    for (header, path) in converted {
        checkpoint()?;
        if deleted.contains(&header.meta.session_id) { continue; }
        if !known.insert((AppKind::Claude, header.meta.session_id.clone())) {
            issues.push(SessionIssue::new(AppKind::Claude, format!("转换记录 {} 与现有会话身份冲突，未覆盖现有来源", path.display())));
            continue;
        }
        completed.insert(format!("{}:{}", header.source_key, header.revision));
        sources.push(SessionSource {
            meta: header.meta, paths: Vec::new(), native_history: None, converted: Some(path), retained: Vec::new(), index_root: root.into() });
    }
    issues.retain(|issue| issue.conversion_id.as_ref().is_none_or(|id| !completed.contains(id)));
    issues.extend(failures.into_iter().map(|message| SessionIssue::new(AppKind::Claude, message)));
    checkpoint()
}

fn suppressed(id: &str, deleted: &HashSet<String>) -> bool {
    id.split_once(':').is_some_and(|(key, _)| deleted.contains(&format!("workflow-{key}")))
}

fn repair_sources(root: &Path, ids: &[String], checkpoint: &dyn Fn() -> Result<(), String>) -> Result<SessionConversionOutcome, String> {
    let roots = super::history::workflow_roots(root)?.into_iter().map(|path| (AppKind::Claude, path)).collect::<Vec<_>>();
    convert_from_roots(root, ids, &roots, checkpoint)
}

fn convert_from_roots(root: &Path, ids: &[String], roots: &[(AppKind, PathBuf)], checkpoint: &dyn Fn() -> Result<(), String>) -> Result<SessionConversionOutcome, String> {
    if ids.is_empty() { return Err("没有选择可转换的来源".into()); }
    let _guard = CONVERSION_GATE.lock().map_err(|_| "会话转换锁不可用")?;
    let deleted = super::history::deleted_ids(root)?;
    let completed: HashSet<_> = store::list(root).0.into_iter()
        .map(|(header, _)| format!("{}:{}", header.source_key, header.revision)).collect();
    let mut requested: HashSet<_> = ids.iter().cloned().collect();
    let mut outcome = SessionConversionOutcome { converted: 0, records: 0, failures: Vec::new() };
    let mut unavailable = Vec::new();
    for (app, directory) in roots {
        if *app != AppKind::Claude { continue; }
        checkpoint()?;
        let files = sources::collect_observed(directory, checkpoint)?;
        unavailable.extend(files.issues);
        for path in files.paths.into_iter().filter(|path| is_workflow(path)) {
            checkpoint()?;
            let prefix = format!("{}:", source_key(&path));
            let selected: Vec<_> = requested.iter().filter(|id| id.starts_with(prefix.as_str())).cloned().collect();
            if selected.is_empty() { continue; }
            let current = candidate(&path);
            for id in selected {
                requested.remove(&id);
                if suppressed(&id, &deleted) { continue; }
                if completed.contains(&id) && current.as_ref().is_ok_and(|source| source.id() == id) { continue; }
                let result = current.as_ref().map_err(|error| error.clone()).and_then(|source| {
                    if source.id() != id { return Err("来源已经变化，请刷新后重新转换".into()); }
                    convert(root, source, checkpoint)
                });
                match result {
                    Ok(count) => { outcome.converted += 1; outcome.records += count; }
                    Err(error) => outcome.failures.push(SessionIssue { app: *app, conversion_id: Some(id),
                        message: format!("{}: {error}", path.display()) }),
                }
                checkpoint()?;
            }
        }
    }
    for id in requested {
        outcome.failures.push(SessionIssue { app: AppKind::Claude, conversion_id: Some(id),
            message: if unavailable.is_empty() { "来源已经变化或不可用，请刷新后重新转换".into() }
                else { unavailable.join("；") } });
    }
    Ok(outcome)
}

fn convert(root: &Path, source: &Candidate, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<usize, String> {
    let (meta, count) = metadata(source, checkpoint)?;
    store::write(root, source, meta, count, checkpoint)
}

fn metadata(source: &Candidate, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<(SessionMeta, usize), String> {
    if source.parent_file.is_symlink() { return Err("所属主会话是符号链接，未读取链接目标".into()); }
    let parent = if source.parent_file.try_exists().map_err(|error| error.to_string())? {
        let (meta, issues) = parser::parse_session_observed(AppKind::Claude, &source.parent_file, checkpoint).map_err(|error| error.to_string())?;
        if meta.session_id != source.parent_id { return Err("所属主会话身份与目录不一致".into()); }
        if !issues.is_empty() { return Err(issues.join("；")); }
        Some(meta)
    } else { None };
    let mut created_at = None;
    let mut last_active_at = None;
    let mut count = 0;
    let scan = records::scan_records_observed(&source.path, 0, 0, checkpoint, |_, _, value| {
        count += 1;
        if let Some(at) = parser::timestamp_from(&value) {
            if created_at.as_ref().is_none_or(|current| &at < current) { created_at = Some(at.clone()); }
            if last_active_at.as_ref().is_none_or(|current| &at > current) { last_active_at = Some(at); }
        }
        Ok(ControlFlow::Continue(()))
    })?;
    if !scan.issues.is_empty() { return Err(scan.issues.join("；")); }
    if count == 0 { return Err("事件日志为空，没有可转换记录".into()); }
    let title = parser::clamp_title(&format!("Workflow · {}", source.workflow));
    Ok((SessionMeta { app: AppKind::Claude, session_id: format!("workflow-{}", source.source_key),
        title: title.clone(), summary: title, project_dir: parent.and_then(|meta| meta.project_dir),
        created_at, last_active_at, resume_command: Some(super::resume::resume_command(AppKind::Claude, &source.parent_id)),
        parent_session_id: Some(source.parent_id.clone()), alias: None, pinned: false, tags: Vec::new(), can_delete: true, preservation: None }, count))
}

fn message(ordinal: usize, value: &serde_json::Value) -> SessionMessage {
    SessionMessage { id: format!("converted-{ordinal}-{}", parser::record_identity(value)),
        role: if value.get("type").and_then(serde_json::Value::as_str) == Some("result") { "assistant" } else { "tool" }.into(),
        content: content::json(value), at: parser::timestamp_from(value), images: Vec::new() }
}

pub(super) fn remove(root: &Path, path: &Path, id: &str) -> Result<(), String> {
    let _guard = CONVERSION_GATE.lock().map_err(|_| "会话转换锁不可用")?;
    if path.file_stem().and_then(|name| name.to_str()).is_none_or(|name| format!("workflow-{name}") != id) {
        return Err("转换记录与删除身份不一致".into());
    }
    super::history::delete_converted(root, id, path)
}

#[cfg(test)]
mod tests;
