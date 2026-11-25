use super::SearchControl;
use crate::session_manager::{
    messages::aligned_content_offset, read_index::key, resolve_session_observed,
    text::{validate_query, TextQuery}, timeline, SessionSource,
};
use asb_core::contracts::AppKind;
use serde::{Deserialize, Serialize};
use std::{ops::ControlFlow, path::Path};

const PAGE_SIZE: usize = 100;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TranscriptSearchRequest {
    app: AppKind,
    session_id: String,
    query: String,
    offset: usize,
    revision: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptMatch {
    message_id: String,
    content_revision: String,
    content_offset: usize,
    start: usize,
    end: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSearchPage {
    hits: Vec<TranscriptMatch>,
    total: usize,
    offset: usize,
    page_size: usize,
    revision: String,
    issues: Vec<String>,
}

pub(in crate::session_manager) fn search(root: &Path, request: TranscriptSearchRequest, control: &SearchControl)
    -> Result<TranscriptSearchPage, String> {
    validate_query(&request.query)?;
    let query = TextQuery::new(&request.query);
    if query.is_empty() || request.offset % PAGE_SIZE != 0
        || (request.offset > 0 && request.revision.is_none()) {
        return Err("会话内查找条件无效".into());
    }
    let source = resolve_session_observed(root, request.app, &request.session_id, &|| control.checkpoint())?;
    let page = search_source(&source, &query, request.offset, request.revision.as_deref(), control)?;
    super::super::diagnostics::report(root, "会话内查找读取", page.issues.iter().cloned());
    Ok(page)
}

fn search_source(source: &SessionSource, query: &TextQuery, offset: usize, expected_revision: Option<&str>,
    control: &SearchControl,
) -> Result<TranscriptSearchPage, String> {
    let checkpoint = || control.checkpoint();
    control.indexing()?;
    let index = timeline::index_observed(source, &checkpoint)?;
    let mut total = 0;
    let mut hits = Vec::with_capacity(PAGE_SIZE);
    let issues = index.visit_observed(|completed, count| control.messages(completed, count), &checkpoint, |message| {
        let mut content_revision = None;
        let _ = query.scan_matches(&message.content, &checkpoint, |found| {
            if total >= offset && hits.len() < PAGE_SIZE {
                hits.push(TranscriptMatch {
                    message_id: message.id.clone(),
                    content_revision: content_revision.get_or_insert_with(|| asb_switch::sha256_hex(&message.content)).clone(),
                    content_offset: aligned_content_offset(found.range.start),
                    start: found.range.start,
                    end: found.range.end,
                });
            }
            total += 1;
            ControlFlow::Continue(())
        })?;
        Ok(ControlFlow::Continue(()))
    })?;
    control.checkpoint()?;
    let revision = asb_switch::sha256_hex(&key(&(source.meta.app, &source.meta.session_id,
        query.identity(), &index.entries, &issues, total))?);
    if expected_revision.is_some_and(|expected| expected != revision.as_str()) {
        return Err("会话内容已更新，请重新查找".into());
    }
    Ok(TranscriptSearchPage { hits, total, offset, page_size: PAGE_SIZE, revision, issues })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_manager::{fixture_source, messages::CONTENT_PAGE_CHARS};

    #[test]
    fn pages_occurrences_and_rejects_windows_from_a_previous_revision() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let text = format!("{}{}", "😀".repeat(CONTENT_PAGE_CHARS - 1), "Hit ".repeat(PAGE_SIZE + 1));
        let record = serde_json::json!({ "role": "assistant", "content": text });
        std::fs::write(&path, format!("{record}\n")).unwrap();
        let source = fixture_source(&path, AppKind::Codex, "session");
        let control = SearchControl::default();
        let query = TextQuery::new("hit");
        let first = search_source(&source, &query, 0, None, &control).unwrap();
        assert_eq!(first.total, PAGE_SIZE + 1);
        assert_eq!(first.hits.len(), PAGE_SIZE);
        assert_eq!(first.hits[0].start, CONTENT_PAGE_CHARS - 1);
        assert_eq!(first.hits[0].end, CONTENT_PAGE_CHARS + 2);
        assert_eq!(first.hits[0].content_offset, 0);
        assert_eq!(first.hits[1].content_offset, CONTENT_PAGE_CHARS);
        let last = search_source(&source, &query, PAGE_SIZE, Some(&first.revision), &control).unwrap();
        assert_eq!(last.hits.len(), 1);
        assert_eq!(last.hits[0].message_id, first.hits[0].message_id);
        assert_eq!(last.hits[0].content_revision, first.hits[0].content_revision);
        assert_eq!(last.hits[0].start, CONTENT_PAGE_CHARS - 1 + PAGE_SIZE * 4);
        std::fs::write(&path, format!("{record}\n{{\"role\":\"assistant\",\"content\":\"hit\"}}\n")).unwrap();
        assert!(search_source(&source, &query, PAGE_SIZE, Some(&first.revision), &control).is_err());
    }
}
