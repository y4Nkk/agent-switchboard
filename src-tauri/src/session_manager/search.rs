mod filters;
mod content;
pub use filters::{SessionProject, SessionSearchRequest};
use filters::{catalog, Filters};
use super::{organization, scan_filtered_sources,
    text::TextQuery, SessionIssue, SessionMeta, SessionSource};
#[cfg(test)]
use super::parser::collect_messages;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

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
    pub issues: Vec<SessionIssue>,
    pub projects: Vec<SessionProject>,
    pub tags: Vec<String>,
}

pub fn search_sessions(
    root: &Path,
    request: SessionSearchRequest,
) -> Result<SessionSearchPage, String> {
    let filters = Filters::parse(request)?;
    let (mut sources, issues) = scan_filtered_sources(root, filters.request.app, None)?;
    let values = organization::load(root)?;
    for source in &mut sources { organization::apply(&mut source.meta, &values); }
    Ok(search_sources(sources, issues, &filters))
}

fn search_sources(
    mut sources: Vec<SessionSource>,
    mut issues: Vec<SessionIssue>,
    filters: &Filters,
) -> SessionSearchPage {
    let app = filters.request.app;
    let offset = filters.request.offset;
    sources.retain(|source| app.is_none_or(|app| source.meta.app == app));
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
    let mut results = Vec::with_capacity(PAGE_SIZE);
    let mut total = 0;
    for source in sources {
        if query.is_empty() {
            if total >= offset && results.len() < PAGE_SIZE {
                results.push(metadata_hit(&source.meta, &query));
            }
            total += 1;
            continue;
        }
        let before = total;
        match content::matches(&source, &query) {
            Ok(matches) => {
                for hit in matches.hits {
                    if total >= offset && results.len() < PAGE_SIZE {
                        results.push(SessionSearchHit { session: source.meta.clone(), message_id: Some(hit.message_id),
                            content_offset: hit.content_offset, excerpt: hit.excerpt });
                    }
                    total += 1;
                }
                issues.extend(matches.issues.into_iter().map(|message| SessionIssue::new(source.meta.app, message)));
            }
            Err(message) => issues.push(SessionIssue::new(source.meta.app, message)),
        }
        if total == before && metadata_fields(&source.meta)
            .iter().any(|field| query.find(field).is_some()) {
            if total >= offset && results.len() < PAGE_SIZE {
                results.push(metadata_hit(&source.meta, &query));
            }
            total += 1;
        }
    }
    let mut seen = HashSet::new();
    issues.retain(|issue| seen.insert((issue.app, issue.message.clone())));
    SessionSearchPage { results, total, issues, projects, tags }
}

fn metadata_fields(meta: &SessionMeta) -> Vec<&str> {
    let mut fields = vec![meta.title.as_str(), &meta.summary, &meta.session_id,
        meta.project_dir.as_deref().unwrap_or(""), meta.alias.as_deref().unwrap_or("")];
    fields.extend(meta.tags.iter().map(String::as_str));
    fields
}

fn metadata_hit(meta: &SessionMeta, query: &TextQuery) -> SessionSearchHit {
    let text = metadata_fields(meta).into_iter()
        .find(|text| !query.is_empty() && query.find(text).is_some())
        .unwrap_or(&meta.summary);
    SessionSearchHit { session: meta.clone(), message_id: None, content_offset: 0, excerpt: query.excerpt(text) }
}

#[cfg(test)]
mod tests;
