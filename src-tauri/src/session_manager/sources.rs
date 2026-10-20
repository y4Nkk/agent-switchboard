use super::{codex_titles, conversion, native, parser, SessionIssue, SessionMessage, SessionMeta};
use asb_core::contracts::AppKind;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct SessionSource {
    pub meta: SessionMeta,
    pub paths: Vec<PathBuf>,
    pub native_history: Option<PathBuf>,
    pub converted: Option<PathBuf>,
}

impl SessionSource {
    pub(super) fn visit(
        &self,
        mut visitor: impl FnMut(SessionMessage) -> Result<ControlFlow<()>, String>,
    ) -> Result<Vec<String>, String> {
        if let Some(path) = &self.converted { return conversion::visit(path, visitor); }
        if self.native_history.is_none() && self.paths.is_empty() {
            return Ok(vec!["仅保留会话身份，原始记录和原生消息正文均不可用".into()]);
        }
        let result = if self.native_history.is_some() { super::timeline::visit(self, visitor) }
            else { super::jsonl::visit(&self.paths, &mut visitor) };
        result.map(|mut issues| { let mut seen = HashSet::new(); issues.retain(|issue| seen.insert(issue.clone())); issues })
    }

    pub(super) fn find(&self, id: &str) -> Result<SessionMessage, String> {
        let mut found = None;
        let issues = self.visit(|message| {
            if message.id == id { found = Some(message); Ok(ControlFlow::Break(())) }
            else { Ok(ControlFlow::Continue(())) }
        })?;
        found.ok_or_else(|| if issues.is_empty() { "原消息已改变或不存在；请重新打开会话".into() } else { issues.join("；") })
    }
}

pub(crate) struct CollectedFiles {
    pub paths: Vec<PathBuf>,
    pub issues: Vec<String>,
}

pub(crate) fn collect(root: &Path) -> CollectedFiles {
    let mut result = CollectedFiles { paths: Vec::new(), issues: Vec::new() };
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if directory == root && error.kind() == std::io::ErrorKind::NotFound && !root.exists() => continue,
            Err(error) => { result.issues.push(format!("无法读取会话目录 {}: {error}", directory.display())); continue; }
        };
        for entry in entries {
            let path = match entry.and_then(|entry| Ok((entry.path(), entry.file_type()?))) {
                Ok((path, kind)) if kind.is_symlink() => { result.issues.push(format!("会话链接未读取，以免循环或跨出来源范围: {}", path.display())); continue; },
                Ok((path, kind)) if kind.is_dir() => { pending.push(path); continue; }
                Ok((path, kind)) if kind.is_file() => path,
                Ok(_) => continue,
                Err(error) => { result.issues.push(format!("读取 {} 的会话条目失败: {error}", directory.display())); continue; }
            };
            if path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl")) { result.paths.push(path); }
        }
    }
    result.paths.sort();
    result
}

pub(super) fn scan() -> Result<(Vec<SessionSource>, Vec<SessionIssue>), String> {
    let roots = super::session_roots()?;
    let codex_root = super::codex_root()?;
    let mut indexed = HashMap::new();
    let mut issues = Vec::new();
    let mut native_unavailable = false;
    match native::threads(&codex_root) {
        Ok((threads, warnings)) => {
            issues.extend(warnings.into_iter().map(|message| SessionIssue::new(AppKind::Codex, message)));
            for thread in threads {
                let mut paths = Vec::new();
                if roots.iter().any(|(app, root)| *app == AppKind::Codex && thread.rollout.starts_with(root))
                    && thread.rollout.is_file() && !thread.rollout.is_symlink() {
                    match parser::session_identity(&thread.rollout) {
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
                    meta: thread.meta, paths, native_history: history, converted: None });
            }
        },
        Err(message) => { native_unavailable = true; issues.push(SessionIssue::new(AppKind::Codex, message)); },
    }
    scan_roots(&roots, &mut indexed, &mut issues);
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

pub(super) fn scan_roots(
    roots: &[(AppKind, PathBuf)],
    indexed: &mut HashMap<(AppKind, String), SessionSource>,
    issues: &mut Vec<SessionIssue>,
) {
    for (app, root) in roots {
        let files = collect(root);
        issues.extend(files.issues.into_iter().map(|message| SessionIssue::new(*app, message)));
        for path in files.paths {
            if *app == AppKind::Claude && conversion::is_workflow(&path) {
                issues.push(conversion::pending_issue(&path).unwrap_or_else(|message| SessionIssue::new(*app, message)));
                continue;
            }
            match index_file(*app, path, indexed) {
                Ok(warnings) => issues.extend(warnings.into_iter().map(|message| SessionIssue::new(*app, message))),
                Err(message) => issues.push(SessionIssue::new(*app, message)),
            }
        }
    }
}

fn index_file(app: AppKind, path: PathBuf, indexed: &mut HashMap<(AppKind, String), SessionSource>) -> Result<Vec<String>, String> {
    let mut issues = Vec::new();
    let identity = parser::session_identity(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let Some(identity) = identity else {
        return Err(format!("{} 不含可识别的会话身份", path.display()));
    };
    let id = identity.id;
    let child = app == AppKind::Claude && path.components().any(|component| component.as_os_str() == "subagents");
    let key_id = if child { format!("{id}-{}", path.file_stem().and_then(|name| name.to_str()).ok_or("子代理文件名无效")?) } else { id.clone() };
    if !parser::valid_session_id(&key_id) { return Err(format!("{} 的会话身份超出当前契约", path.display())); }
    if let Some(source) = indexed.get_mut(&(app, key_id.clone())) {
        if app == AppKind::Codex {
            if source.meta.parent_session_id.is_none() { source.meta.parent_session_id = identity.parent; }
            if source.meta.created_at.is_none() { source.meta.created_at = identity.created_at; }
        }
        if source.paths.contains(&path) { return Ok(issues); }
        if source.meta.can_delete {
            let (next, warnings) = parser::parse_session(app, &path).map_err(|error| error.to_string())?;
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
    let (mut meta, warnings) = parser::parse_session(app, &path).map_err(|error| format!("{}: {error}", path.display()))?;
    issues.extend(warnings);
    if child { meta.parent_session_id = Some(id); meta.session_id = key_id.clone(); }
    indexed.insert((app, key_id), SessionSource { meta, paths: vec![path], native_history: None, converted: None });
    Ok(issues)
}
