use super::super::{SessionMeta, SessionSource};
use asb_core::contracts::AppKind;
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionProjectFilter {
    pub app: AppKind,
    pub project_dir: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionSearchRequest {
    pub query: String,
    pub app: Option<AppKind>,
    pub offset: usize,
    pub project: Option<SessionProjectFilter>,
    pub tag: Option<String>,
    pub pinned_only: bool,
    pub active_after: Option<String>,
    pub active_before: Option<String>,
    pub archived: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionProject {
    pub app: AppKind,
    pub project_dir: Option<String>,
    pub count: usize,
}

pub(super) struct Filters {
    pub request: SessionSearchRequest,
    after: Option<DateTime<FixedOffset>>,
    before: Option<DateTime<FixedOffset>>,
}

impl Filters {
    pub(super) fn parse(request: SessionSearchRequest) -> Result<Self, String> {
        if request.query.chars().count() > 4000 { return Err("搜索内容最多 4000 个字符".into()); }
        if request.tag.as_ref().is_some_and(|tag| tag.is_empty() || tag.chars().count() > 40 || tag.trim() != tag) {
            return Err("筛选标签无效".into());
        }
        if let Some(project) = &request.project {
            if request.app.is_some_and(|app| app != project.app)
                || project.project_dir.as_ref().is_some_and(|path| path.is_empty() || path.chars().count() > 32768) {
                return Err("筛选项目无效".into());
            }
        }
        let after = parse_time(request.active_after.as_deref())?;
        let before = parse_time(request.active_before.as_deref())?;
        if after.zip(before).is_some_and(|(after, before)| after >= before) {
            return Err("时间范围的开始必须早于结束".into());
        }
        Ok(Self { request, after, before })
    }

    pub(super) fn matches(&self, meta: &SessionMeta) -> bool {
        if self.request.project.as_ref().is_some_and(|project| project.app != meta.app || project.project_dir != meta.project_dir)
            || self.request.pinned_only && !meta.pinned
            || self.request.tag.as_ref().is_some_and(|tag| !meta.tags.contains(tag)) { return false; }
        if self.after.is_none() && self.before.is_none() { return true; }
        let Some(at) = meta.last_active_at.as_deref().and_then(|at| DateTime::parse_from_rfc3339(at).ok()) else { return false; };
        self.after.is_none_or(|after| at >= after) && self.before.is_none_or(|before| at < before)
    }
}

fn parse_time(value: Option<&str>) -> Result<Option<DateTime<FixedOffset>>, String> {
    value.map(|value| DateTime::parse_from_rfc3339(value).map_err(|_| "时间筛选必须为 RFC3339 时间".to_string())).transpose()
}

pub(super) fn catalog(sources: &[SessionSource]) -> (Vec<SessionProject>, Vec<String>) {
    let mut projects = BTreeMap::new();
    let mut tags = BTreeSet::new();
    for source in sources {
        *projects.entry((source.meta.app, source.meta.project_dir.clone())).or_insert(0) += 1;
        tags.extend(source.meta.tags.iter().cloned());
    }
    (projects.into_iter().map(|((app, project_dir), count)| SessionProject { app, project_dir, count }).collect(),
        tags.into_iter().collect())
}
