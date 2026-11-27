mod filters;
mod content;
mod snapshot;
pub(super) mod transcript;
pub use filters::{SessionProject, SessionSearchRequest};
use filters::{catalog, Filters};
pub(super) use snapshot::SearchSnapshot;
use snapshot::SearchRows;
use super::{organization,
    text::TextQuery, SessionIssue, SessionMeta, SessionSource};
#[cfg(test)]
use super::parser::collect_messages;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use super::search_tasks::SearchControl;

const PAGE_SIZE: usize = 50;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchHit {
    pub session: SessionMeta,
    pub message_id: Option<String>,
    pub content_offset: usize,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchPage {
    pub results: Vec<SessionSearchHit>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub issues: Vec<SessionIssue>,
    pub projects: Vec<SessionProject>,
    pub tags: Vec<String>,
}

#[cfg(test)]
pub fn search_sessions(
    root: &Path,
    request: SessionSearchRequest,
) -> Result<SessionSearchPage, String> {
    search_observed(root, request, &SearchControl::default())?.page(1)
}

pub(super) fn search_observed(root: &Path, request: SessionSearchRequest, control: &SearchControl) -> Result<SearchSnapshot, String> {
    let filters = Filters::parse(request)?;
    let (mut sources, issues) = super::scan_filtered_sources_observed(root, filters.request.app, None, &|| control.checkpoint())?;
    control.checkpoint()?;
    let values = organization::load(root)?;
    for source in &mut sources { organization::apply(&mut source.meta, &values); }
    let snapshot = search_sources_observed(sources, issues, &filters, control)?;
    super::diagnostics::report(root, "会话搜索读取",
        snapshot.issues.iter().map(|issue| format!("{}: {}", issue.app.label(), issue.message)));
    Ok(snapshot)
}

#[cfg(test)]
fn search_sources(
    sources: Vec<SessionSource>, issues: Vec<SessionIssue>, filters: &Filters,
) -> SessionSearchPage {
    search_sources_observed(sources, issues, filters, &SearchControl::default()).unwrap().page(1).unwrap()
}

fn search_sources_observed(
    mut sources: Vec<SessionSource>,
    mut issues: Vec<SessionIssue>,
    filters: &Filters,
    control: &SearchControl,
) -> Result<SearchSnapshot, String> {
    let app = filters.request.app;
    sources.retain(|source| app.is_none_or(|app| source.meta.app == app));
    sources.retain(|source| source.archived == filters.request.archived);
    issues.retain(|issue| app.is_none_or(|app| issue.app == app));
    let (projects, tags) = catalog(&sources);
    sources.retain(|source| filters.matches(&source.meta));
    sources.sort_by(|left, right| {
        right.meta.pinned.cmp(&left.meta.pinned)
            .then_with(|| right.meta.last_active_at.cmp(&left.meta.last_active_at))
            .then_with(|| left.meta.app.cmp(&right.meta.app))
            .then_with(|| left.meta.session_id.cmp(&right.meta.session_id))
    });
    let query = TextQuery::new(&filters.request.query);
    let count = sources.len();
    control.sessions(0, count)?;
    SearchSnapshot::build(projects, tags, |rows| {
        for (completed, source) in sources.into_iter().enumerate() {
            control.sessions(completed, count)?;
            write_matches(rows, &source, &query, control, &mut issues)?;
        }
        control.sessions(count, count)?;
        let mut seen = HashSet::new();
        issues.retain(|issue| seen.insert((issue.app, issue.message.clone())));
        Ok(issues)
    })
}

fn write_matches(rows: &mut SearchRows<'_>, source: &SessionSource, query: &TextQuery, control: &SearchControl,
    issues: &mut Vec<SessionIssue>) -> Result<(), String> {
    if !query.is_empty() {
        match content::matches(source, query, control) {
            Ok(matches) => {
                issues.extend(matches.issues.into_iter().map(|message| SessionIssue::new(source.meta.app, message)));
                if !matches.hits.is_empty() {
                    let session = rows.insert_session(&source.meta)?;
                    for (ordinal, hit) in matches.hits.iter().enumerate() {
                        if ordinal % 64 == 0 { control.checkpoint()?; }
                        rows.insert_hit(session, Some(&hit.message_id), hit.content_offset, &hit.excerpt)?;
                    }
                    return Ok(());
                }
            }
            Err(message) => issues.push(SessionIssue::new(source.meta.app, message)),
        }
    }
    control.checkpoint()?;
    let text = metadata_fields(&source.meta)
        .find(|text| !query.is_empty() && query.find(text).is_some());
    if query.is_empty() || text.is_some() {
        let session = rows.insert_session(&source.meta)?;
        rows.insert_hit(session, None, 0, &query.excerpt(text.unwrap_or(&source.meta.summary)))?;
    }
    Ok(())
}

fn metadata_fields(meta: &SessionMeta) -> impl Iterator<Item = &str> {
    [meta.title.as_str(), &meta.summary, &meta.session_id,
        meta.project_dir.as_deref().unwrap_or(""), meta.alias.as_deref().unwrap_or("")]
        .into_iter().chain(meta.tags.iter().map(String::as_str))
}

#[cfg(test)]
mod tests;
