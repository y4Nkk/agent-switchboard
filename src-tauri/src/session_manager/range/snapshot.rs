use super::{Action, Item, Plan, Preview, Request};
use crate::session_manager::{self as sessions, organization, SessionSource};
use asb_core::AppKind;
use chrono::{DateTime, FixedOffset};
use sha2::{Digest, Sha256};
use std::{collections::{BTreeMap, BTreeSet}, io::Read, ops::ControlFlow, path::Path};

pub(super) fn dates(request: &Request) -> Result<(DateTime<FixedOffset>, DateTime<FixedOffset>), String> {
    let after = DateTime::parse_from_rfc3339(&request.active_after).map_err(|_| "开始日期无效")?;
    let before = DateTime::parse_from_rfc3339(&request.active_before).map_err(|_| "结束日期无效")?;
    if after >= before { return Err("结束日期不能早于开始日期".into()); }
    Ok((after, before))
}

pub(super) fn prepare(root: &Path, request: Request, action: Action) -> Result<Plan, String> {
    let (after, before) = dates(&request)?;
    if (action == Action::Archive && request.archived) || (action == Action::Unarchive && !request.archived) {
        return Err("操作与当前会话列表状态不一致".into());
    }
    let (mut sources, issues) = sessions::scan_filtered_sources(root, request.app, None)?;
    let values = organization::load(root)?;
    for source in &mut sources { organization::apply(&mut source.meta, &values); }
    let roots: Vec<_> = sources.iter().filter(|source| source.archived == request.archived
        && source.meta.last_active_at.as_deref().and_then(|at| DateTime::parse_from_rfc3339(at).ok())
            .is_some_and(|at| at >= after && at < before)).collect();
    let selected: BTreeSet<_> = roots.iter().map(|source| key(source)).collect();
    let mut members = BTreeMap::new();
    for source in &roots { for member in related(source, &sources, action) { members.insert(key(member), member); } }
    let mut items = Vec::new();
    let mut fingerprints = BTreeMap::new();
    for (key, source) in members {
        fingerprints.insert(key.clone(), fingerprint(source)?);
        items.push(Item { session: source.meta.clone(), related: !selected.contains(&key) });
    }
    let requests = roots.iter().map(|source| sessions::SessionDeleteRequest {
        app: source.meta.app, session_id: source.meta.session_id.clone(),
    }).collect();
    Ok(Plan { root: root.into(), preview: Preview { id: uuid::Uuid::new_v4().to_string(), action, request,
        matched: roots.len(), items, issues }, fingerprints, roots: requests })
}

pub(super) fn key(source: &SessionSource) -> (AppKind, String) { (source.meta.app, source.meta.session_id.clone()) }

pub(super) fn related<'a>(source: &'a SessionSource, sources: &'a [SessionSource], action: Action) -> Vec<&'a SessionSource> {
    if source.meta.app == AppKind::Codex { return sessions::codex_deletion::related_sources(source, sources); }
    if action != Action::Delete { return vec![source]; }
    let ids = sessions::removed_claude_ids(source, sources);
    sources.iter().filter(|member| member.meta.app == source.meta.app && ids.contains(&member.meta.session_id)).collect()
}

pub(super) fn fingerprint(source: &SessionSource) -> Result<String, String> {
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(&source.meta).map_err(|error| error.to_string())?);
    hash.update(serde_json::to_vec(&(&source.native_database, &source.native_history)).map_err(|error| error.to_string())?);
    hash.update([u8::from(source.archived)]);
    hash.update(serde_json::to_vec(&source.spawn_parent).map_err(|error| error.to_string())?);
    hash.update(serde_json::to_vec(&source.retained).map_err(|error| error.to_string())?);
    for path in source.paths.iter().chain(source.converted.iter()) {
        hash.update(path.as_os_str().as_encoded_bytes());
        if !path.try_exists().map_err(|error| error.to_string())? { hash.update(b"missing"); continue; }
        let before = sessions::read_index::FileRevision::read(path)?;
        let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
        let mut buffer = [0; 65536];
        loop {
            let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
            if count == 0 { break; } hash.update(&buffer[..count]);
        }
        if sessions::read_index::FileRevision::read(path)? != before { return Err("会话在预览期间变化，请重试".into()); }
    }
    let issues = source.visit(|message| {
        hash.update(serde_json::to_vec(&message).map_err(|error| error.to_string())?);
        Ok(ControlFlow::Continue(()))
    })?;
    if !issues.is_empty() { return Err(format!("无法确认会话 {} 的完整内容：{}", source.meta.session_id, issues.join("；"))); }
    Ok(format!("{:x}", hash.finalize()))
}

pub(super) fn validate(plan: &Plan, sources: &[SessionSource]) -> Result<(), String> {
    let mut current = BTreeSet::new();
    for request in &plan.roots {
        let source = sessions::locate(sources, &[], request.app, &request.session_id)?;
        for member in related(source, sources, plan.preview.action) { current.insert(key(member)); }
    }
    if current != plan.fingerprints.keys().cloned().collect() { return Err("关联会话范围已变化，请重新预览".into()); }
    for source in sources.iter().filter(|source| current.contains(&key(source))) {
        if plan.fingerprints.get(&key(source)) != Some(&fingerprint(source)?) { return Err("会话内容或状态已变化，请重新预览".into()); }
    }
    Ok(())
}
