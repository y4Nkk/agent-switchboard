use super::{codex_titles, conversion, native, parser, SessionIssue, SessionMessage, SessionMeta};
use asb_core::contracts::AppKind;
use std::collections::HashMap;
use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use super::read_index::ReadIndex;
use std::sync::Mutex;

static DISCOVERY_GATE: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone)]
pub(crate) struct SessionSource {
    pub meta: SessionMeta,
    pub paths: Vec<PathBuf>,
    pub native_history: Option<PathBuf>,
    pub converted: Option<PathBuf>,
    pub index_root: PathBuf,
    pub retained: Vec<super::history::Transcript>,
}

impl SessionSource {
    pub(crate) fn record_sources(&self) -> Vec<super::records::Source> {
        if self.retained.is_empty() {
            self.paths.iter().cloned().map(super::records::Source::File).collect()
        } else {
            self.retained.iter().cloned().map(super::records::Source::Saved).collect()
        }
    }

    pub(super) fn resolve_images(&self, references: &[String]) -> (Vec<String>, Vec<String>) {
        if self.retained.is_empty() { super::images::resolve(references) }
        else { super::history::resolve_images(&self.retained, references) }
    }

    pub(super) fn copy_text(&self, message: &SessionMessage) -> Result<String, String> {
        let (images, issues) = self.resolve_images(&message.images);
        if !issues.is_empty() { return Err(issues.join("；")); }
        let mut message = message.clone();
        message.images = images;
        super::images::copy_text(&message)
    }
    pub(super) fn visit(
        &self,
        visitor: impl FnMut(SessionMessage) -> Result<ControlFlow<()>, String>,
    ) -> Result<Vec<String>, String> {
        super::timeline::index(self)?.visit(visitor)
    }

    pub(super) fn find(&self, id: &str) -> Result<SessionMessage, String> {
        super::timeline::index(self)?.find(id)
    }
}

pub(crate) struct CollectedFiles {
    pub paths: Vec<PathBuf>,
    pub issues: Vec<String>,
}

pub(super) fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    { use std::os::windows::fs::MetadataExt; metadata.file_attributes() & 0x400 != 0 }
    #[cfg(not(windows))]
    { metadata.file_type().is_symlink() }
}

pub(crate) fn collect(root: &Path) -> CollectedFiles {
    collect_observed(root, &|| Ok(())).expect("unobserved discovery cannot be cancelled")
}

pub(super) fn collect_observed(root: &Path, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<CollectedFiles, String> {
    let mut result = CollectedFiles { paths: Vec::new(), issues: Vec::new() };
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        checkpoint()?;
        match fs::symlink_metadata(&directory) {
            Ok(metadata) if is_link(&metadata) => { result.issues.push(format!("会话链接未读取：{}", directory.display())); continue; }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && directory == root => continue,
            Err(error) => { result.issues.push(format!("读取会话目录失败 {}: {error}", directory.display())); continue; }
            _ => {}
        }
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if directory == root && error.kind() == std::io::ErrorKind::NotFound && !root.exists() => continue,
            Err(error) => { result.issues.push(format!("无法读取会话目录 {}: {error}", directory.display())); continue; }
        };
        for entry in entries {
            checkpoint()?;
            let path = match entry.and_then(|entry| Ok((entry.path(), fs::symlink_metadata(entry.path())?))) {
                Ok((path, kind)) if is_link(&kind) => { result.issues.push(format!("会话链接未读取，以免循环或跨出来源范围: {}", path.display())); continue; },
                Ok((path, kind)) if kind.is_dir() => { pending.push(path); continue; }
                Ok((path, kind)) if kind.is_file() => path,
                Ok(_) => continue,
                Err(error) => { result.issues.push(format!("读取 {} 的会话条目失败: {error}", directory.display())); continue; }
            };
            if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl")) { result.paths.push(path); }
        }
    }
    result.paths.sort();
    Ok(result)
}

pub(super) fn scan(root: &Path, app: Option<AppKind>, session: Option<&str>) -> Result<(Vec<SessionSource>, Vec<SessionIssue>), String> {
    scan_observed(root, app, session, &|| Ok(()))
}

pub(super) fn scan_observed(root: &Path, app: Option<AppKind>, session: Option<&str>, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<(Vec<SessionSource>, Vec<SessionIssue>), String> {
    checkpoint()?;
    let _guard = super::read_observer::lock(&DISCOVERY_GATE, checkpoint)?;
    checkpoint()?;
    let index = ReadIndex::open_observed(root, checkpoint)?;
    let roots: Vec<_> = super::codex_session_roots()?.into_iter().map(|path| (AppKind::Codex, path))
        .filter(|(kind, _)| app.is_none_or(|app| app == *kind)).collect();
    let codex_root = super::codex_root()?;
    let mut indexed = HashMap::new();
    let mut issues = Vec::new();
    let mut native_unavailable = false;
    match if app == Some(AppKind::Claude) { Ok((Vec::new(), Vec::new())) } else { native::threads_matching(&codex_root, session, checkpoint) } {
        Ok((threads, warnings)) => {
            issues.extend(warnings.into_iter().map(|message| SessionIssue::new(AppKind::Codex, message)));
            for thread in threads {
                checkpoint()?;
                let mut paths = Vec::new();
                if roots.iter().any(|(app, root)| *app == AppKind::Codex && thread.rollout.starts_with(root))
                    && thread.rollout.is_file() && !thread.rollout.is_symlink() {
                    match index.file_prefix("identity", &thread.rollout, checkpoint, |reader| parser::session_identity_reader(&thread.rollout, reader, checkpoint).map_err(|error| error.to_string())) {
                        Ok(Some(identity)) if identity.id == thread.meta.session_id => paths.push(thread.rollout),
                        _ => issues.push(SessionIssue::new(AppKind::Codex, format!("会话 {} 的原生记录身份与登记信息不一致", thread.meta.session_id))),
                    }
                }
                let history = match thread.history.try_exists() {
                    Ok(false) => None,
                    Ok(true) => Some(thread.history),
                    Err(error) => {
                        issues.push(SessionIssue::new(AppKind::Codex, error.to_string()));
                        Some(thread.history)
                    }
                };
                indexed.insert((AppKind::Codex, thread.meta.session_id.clone()), SessionSource {
                    meta: thread.meta, paths, native_history: history, converted: None, retained: Vec::new(), index_root: root.into() });
            }
        },
        Err(message) => { native_unavailable = true; issues.push(SessionIssue::new(AppKind::Codex, message)); },
    }
    scan_roots_indexed(root, &roots, &mut indexed, &mut issues, &index, session, checkpoint)?;
    checkpoint()?;
    if app != Some(AppKind::Codex) {
        match super::history::scan_observed(root, checkpoint) {
            Ok((saved, warnings)) => {
                issues.extend(warnings);
                for source in saved.into_iter().filter(|source| session.is_none_or(|id| source.meta.session_id == id)) {
                    indexed.insert((AppKind::Claude, source.meta.session_id.clone()), source);
                }
            }
            Err(message) => issues.push(SessionIssue::new(AppKind::Claude, message)),
        }
    }
    checkpoint()?;
    let (titles, warnings) = if indexed.values().any(|source| source.meta.app == AppKind::Codex && source.meta.can_delete) {
        codex_titles::load(&codex_root)
    } else { (HashMap::new(), Vec::new()) };
    issues.extend(warnings.into_iter().map(|message| SessionIssue::new(AppKind::Codex, message)));
    for source in indexed.values_mut() {
        source.paths.sort();
        if source.meta.app == AppKind::Codex && source.meta.can_delete {
            if let Some(title) = titles.get(&source.meta.session_id) { source.meta.title = parser::clamp_title(title); }
            if native_unavailable { source.meta.can_delete = false; }
        }
    }
    Ok((indexed.into_values().collect(), issues))
}

#[cfg(test)]
pub(super) fn scan_roots(
    roots: &[(AppKind, PathBuf)],
    indexed: &mut HashMap<(AppKind, String), SessionSource>,
    issues: &mut Vec<SessionIssue>,
) {
    let Some((_, first)) = roots.first() else { return; };
    let root = first.join(".asb-read-index");
    match ReadIndex::open(&root) {
        Ok(index) => {
            if let Err(message) = scan_roots_indexed(&root, roots, indexed, issues, &index, None, &|| Ok(())) {
                issues.push(SessionIssue::new(roots[0].0, message));
            }
        },
        Err(message) => issues.push(SessionIssue::new(roots[0].0, message)),
    }
}

fn scan_roots_indexed(
    index_root: &Path, roots: &[(AppKind, PathBuf)], indexed: &mut HashMap<(AppKind, String), SessionSource>,
    issues: &mut Vec<SessionIssue>, index: &ReadIndex, session: Option<&str>, checkpoint: &dyn Fn() -> Result<(), String>,
) -> Result<(), String> {
    for (app, directory) in roots {
        let files = collect_observed(directory, checkpoint)?;
        issues.extend(files.issues.into_iter().map(|message| SessionIssue::new(*app, message)));
        for path in files.paths {
            checkpoint()?;
            if *app == AppKind::Claude && conversion::is_workflow(&path) {
                issues.push(index.file("workflow-issue", &path, || conversion::pending_issue(&path))
                    .unwrap_or_else(|message| SessionIssue::new(*app, message)));
                continue;
            }
            let result = index_file(index_root, *app, path, indexed, index, session, checkpoint);
            checkpoint()?;
            match result {
                Ok(warnings) => issues.extend(warnings.into_iter().map(|message| SessionIssue::new(*app, message))),
                Err(message) => issues.push(SessionIssue::new(*app, message)),
            }
        }
    }
    Ok(())
}

fn index_file(root: &Path, app: AppKind, path: PathBuf, indexed: &mut HashMap<(AppKind, String), SessionSource>, index: &ReadIndex, session: Option<&str>, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<Vec<String>, String> {
    let mut issues = Vec::new();
    let identity = index.file_prefix("identity", &path, checkpoint, |reader| parser::session_identity_reader(&path, reader, checkpoint).map_err(|error| error.to_string()))?;
    let Some(identity) = identity else {
        return Err(format!("{} 不含可识别的会话身份", path.display()));
    };
    let id = identity.id;
    let child = app == AppKind::Claude && path.components().any(|component| component.as_os_str() == "subagents");
    let key_id = if child { format!("{id}-{}", path.file_stem().and_then(|name| name.to_str()).ok_or("子代理文件名无效")?) } else { id.clone() };
    if session.is_some_and(|requested| requested != key_id) { return Ok(issues); }
    if !parser::valid_session_id(&key_id) { return Err(format!("{} 的会话身份超出当前契约", path.display())); }
    if let Some(source) = indexed.get_mut(&(app, key_id.clone())) {
        if app == AppKind::Codex {
            if source.meta.parent_session_id.is_none() { source.meta.parent_session_id = identity.parent; }
            if source.meta.created_at.is_none() { source.meta.created_at = identity.created_at; }
        }
        if source.paths.contains(&path) { return Ok(issues); }
        if source.meta.can_delete {
            let (next, warnings) = index.file_prefix(&format!("metadata-{}", app.label()), &path, checkpoint, |reader| parser::parse_session_reader(app, &path, reader, checkpoint).map_err(|error| error.to_string()))?;
            if next.session_id != id { return Err("会话身份在发现期间被替换，已跳过本次来源".into()); }
            issues.extend(warnings);
            if next.last_active_at > source.meta.last_active_at {
                source.meta.last_active_at = next.last_active_at;
                source.meta.title = next.title;
            }
            if next.created_at.is_some() && (source.meta.created_at.is_none() || next.created_at < source.meta.created_at) {
                source.meta.created_at = next.created_at;
                source.meta.summary = next.summary;
            }
        }
        source.paths.push(path);
        return Ok(issues);
    }
    let (mut meta, warnings) = index.file_prefix(&format!("metadata-{}", app.label()), &path, checkpoint, |reader| parser::parse_session_reader(app, &path, reader, checkpoint).map_err(|error| error.to_string()))?;
    if meta.session_id != id { return Err("会话身份在发现期间被替换，已跳过本次来源".into()); }
    issues.extend(warnings);
    if child { meta.parent_session_id = Some(id); meta.session_id = key_id.clone(); }
    indexed.insert((app, key_id), SessionSource { meta, paths: vec![path], native_history: None, converted: None, retained: Vec::new(), index_root: root.into() });
    Ok(issues)
}

#[cfg(test)]
mod live_tests;
