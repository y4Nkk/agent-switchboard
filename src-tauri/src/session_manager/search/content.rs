use super::super::{messages::aligned_content_offset, read_index::{key, ReadIndex},
    text::{raw_offset, TextQuery}, timeline, SessionSource};
use serde::{Deserialize, Serialize};
use std::ops::ControlFlow;

#[derive(Serialize, Deserialize)]
pub(super) struct ContentMatch {
    pub message_id: String,
    pub content_offset: usize,
    pub excerpt: String,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Matches { pub hits: Vec<ContentMatch>, pub issues: Vec<String> }

pub(super) fn matches(source: &SessionSource, query: &TextQuery, control: &super::SearchControl) -> Result<Matches, String> {
    control.indexing()?;
    let messages = timeline::index_observed(source, &|| control.checkpoint())?;
    control.messages(0, messages.entries.len())?;
    let index = ReadIndex::open_observed(&source.index_root, &|| control.checkpoint())?;
    let cache_key = key(&(source.meta.app, &source.meta.session_id))?;
    let revision = asb_switch::sha256_hex(&key(&(query.identity(), &messages.entries, &messages.issues))?);
    if let Some(matches) = index.get("message-search", &cache_key, &revision)? {
        control.messages(messages.entries.len(), messages.entries.len())?;
        return Ok(matches);
    }
    let mut hits = Vec::new();
    let issues = messages.visit_observed(|completed, total| control.messages(completed, total), |message| {
        control.checkpoint()?;
        if let Some(position) = query.find(&message.content) {
            let raw = raw_offset(&message.content, position).ok_or("搜索正文位置无效")?;
            hits.push(ContentMatch { message_id: message.id, content_offset: aligned_content_offset(raw),
                excerpt: query.excerpt(&message.content) });
        }
        Ok(ControlFlow::Continue(()))
    })?;
    let mut result = Matches { hits, issues };
    if let Err(error) = index.put("message-search", &cache_key, &revision, &result) { result.issues.push(error); }
    Ok(result)
}
