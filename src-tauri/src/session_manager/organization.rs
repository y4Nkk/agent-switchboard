mod store;

use super::{locate, scan_sources, SessionDeleteRequest, SessionMeta};
use asb_core::contracts::AppKind;
use rusqlite::TransactionBehavior;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Organization {
    pub alias: Option<String>,
    pub pinned: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SessionOrganizationChange {
    Pin { pinned: bool },
    Alias { alias: Option<String> },
    SetTags { tags: Vec<String> },
    AddTags { tags: Vec<String> },
    RemoveTags { tags: Vec<String> },
}

pub(super) type Organizations = HashMap<(AppKind, String), Organization>;

pub(super) fn load(root: &Path) -> Result<Organizations, String> {
    if !store::exists(root)? { return Ok(HashMap::new()); }
    store::read_all(&store::open(root)?)
}

pub(super) fn apply(meta: &mut SessionMeta, organizations: &Organizations) {
    if let Some(value) = organizations.get(&(meta.app, meta.session_id.clone())) {
        meta.alias = value.alias.clone();
        meta.pinned = value.pinned;
        meta.tags = value.tags.clone();
    }
}

pub fn update_session_organization(
    root: &Path,
    requests: &[SessionDeleteRequest],
    change: &SessionOrganizationChange,
) -> Result<Vec<SessionMeta>, String> {
    validate_request(requests, change)?;
    let (sources, issues) = scan_sources(root)?;
    let mut seen = HashSet::new();
    let metas = requests.iter().filter(|request| seen.insert((request.app, request.session_id.clone())))
        .map(|request| locate(&sources, &issues, request.app, &request.session_id).map(|source| source.meta.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    update_resolved(root, metas, change)
}

fn update_resolved(
    root: &Path,
    mut metas: Vec<SessionMeta>,
    change: &SessionOrganizationChange,
) -> Result<Vec<SessionMeta>, String> {
    let mut connection = store::open(root)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store::error)?;
    let mut values = store::read_all(&transaction)?;
    for meta in &mut metas {
        let key = (meta.app, meta.session_id.clone());
        let value = values.entry(key).or_default();
        update_value(value, change)?;
        store::save(&transaction, meta.app, &meta.session_id, value)?;
        apply(meta, &values);
    }
    transaction.commit().map_err(store::error)?;
    Ok(metas)
}

fn validate_request(requests: &[SessionDeleteRequest], change: &SessionOrganizationChange) -> Result<(), String> {
    if requests.is_empty() || requests.len() > 200 {
        return Err("一次整理需选择 1 至 200 个会话".into());
    }
    if requests.iter().any(|request| !super::parser::valid_session_id(&request.session_id)) {
        return Err("会话 ID 无效".into());
    }
    if matches!(change, SessionOrganizationChange::Alias { .. }) && requests.len() != 1 {
        return Err("别名只能逐个会话设置".into());
    }
    update_value(&mut Organization::default(), change)
}

fn update_value(value: &mut Organization, change: &SessionOrganizationChange) -> Result<(), String> {
    match change {
        SessionOrganizationChange::Pin { pinned } => value.pinned = *pinned,
        SessionOrganizationChange::Alias { alias } => {
            value.alias = alias.as_deref().map(str::trim).filter(|alias| !alias.is_empty()).map(str::to_string);
        }
        SessionOrganizationChange::SetTags { tags } => value.tags = normalize_tags(tags)?,
        SessionOrganizationChange::AddTags { tags } => {
            value.tags.extend(normalize_tags(tags)?);
            value.tags.sort();
            value.tags.dedup();
        }
        SessionOrganizationChange::RemoveTags { tags } => {
            let tags = normalize_tags(tags)?;
            value.tags.retain(|tag| !tags.contains(tag));
        }
    }
    validate_value(value)
}

fn normalize_tags(tags: &[String]) -> Result<Vec<String>, String> {
    if tags.len() > 20 { return Err("每个会话最多 20 个标签".into()); }
    let mut tags = tags.iter().map(|tag| tag.trim().to_string()).collect::<Vec<_>>();
    if tags.iter().any(|tag| tag.is_empty() || tag.chars().count() > 40 || tag.chars().any(char::is_control)) {
        return Err("标签必须为 1 至 40 个字符，且不含控制字符".into());
    }
    tags.sort();
    tags.dedup();
    Ok(tags)
}

fn validate_value(value: &Organization) -> Result<(), String> {
    if value.alias.as_ref().is_some_and(|alias| alias.chars().count() > 120 || alias.chars().any(char::is_control)) {
        return Err("会话别名最多 120 个字符，且不含控制字符".into());
    }
    if normalize_tags(&value.tags)? != value.tags {
        return Err("会话整理标签格式无效".into());
    }
    if value.alias.as_deref().is_some_and(|alias| alias.is_empty() || alias.trim() != alias) {
        return Err("会话整理别名格式无效".into());
    }
    Ok(())
}

pub(super) fn delete_with_sources(
    root: &Path, app: AppKind, session_ids: &[String], remove: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if !store::exists(root)? { return remove(); }
    let mut connection = store::open(root)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(store::error)?;
    for session_id in session_ids { store::remove(&transaction, app, session_id)?; }
    remove()?;
    transaction.commit().map_err(|error| format!("会话文件已删除，但本地整理数据清理提交失败：{error}"))
}

#[cfg(test)]
mod tests;
