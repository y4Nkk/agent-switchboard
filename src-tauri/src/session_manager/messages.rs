use super::{bookmarks, resolve_session, timeline::Position, SessionMessage, SessionSource};
use asb_core::contracts::{AppKind, LocalizedMessage};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::ops::ControlFlow;

mod content_cache;

const PAGE_SIZE: usize = 25;
pub(super) const PREVIEW_CHARS: usize = 1200;
pub(super) const CONTENT_PAGE_CHARS: usize = 16_384;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstMessage {
    pub id: String,
    pub role: String,
    pub at: Option<String>,
    pub content: String,
    pub truncated: bool,
}

impl FirstMessage {
    pub(super) fn from_message(id: String, message: SessionMessage) -> Self {
        let mut chars = message.content.chars();
        let content = chars.by_ref().take(PREVIEW_CHARS).collect();
        Self { id, role: message.role, at: message.at, content, truncated: chars.next().is_some() }
    }
}

pub fn load_first_messages(root: &Path, app: AppKind, id: &str) -> Result<Vec<FirstMessage>, String> {
    if !super::parser::valid_session_id(id) { return Err("会话 ID 无效".into()); }
    let records = match app {
        AppKind::Codex => {
            let (thread, paths) = super::sources::codex_candidates(root, id)?;
            if let Some(thread) = thread.filter(|thread| thread.history.is_file()) {
                match super::timeline::first_native_messages(&thread.history, id, PAGE_SIZE) {
                    Ok(messages) if !messages.is_empty() => return Ok(messages),
                    Err(error) => log::warn!("无法预读会话 {id} 的原生记录: {error}"),
                    _ => {}
                }
            }
            paths.into_iter().map(super::records::Source::File).collect()
        }
        AppKind::Claude if id.starts_with("workflow-") => {
            let mut messages = Vec::new();
            if let Some(path) = super::conversion::committed_source(root, id)?.and_then(|source| source.converted) {
                super::conversion::visit_messages(&path, |message| {
                    messages.push(FirstMessage::from_message(message.id.clone(), message));
                    Ok(if messages.len() == PAGE_SIZE { ControlFlow::Break(()) } else { ControlFlow::Continue(()) })
                })?;
            }
            return Ok(messages);
        }
        AppKind::Claude => super::history::committed_session(root, id)?
            .map(|source| source.record_sources()).unwrap_or_default(),
    };
    let mut messages = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for record in records {
        match super::jsonl::first(&record, PAGE_SIZE) {
            Ok(first) => for message in first {
                if ids.insert(message.id.clone()) { messages.push(message); }
                if messages.len() == PAGE_SIZE { return Ok(messages); }
            },
            Err(error) => log::warn!("无法预读会话 {id} 的文件记录: {error}"),
        }
    }
    Ok(messages)
}

pub(super) fn aligned_content_offset(offset: usize) -> usize {
    offset / CONTENT_PAGE_CHARS * CONTENT_PAGE_CHARS
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessagePageRequest {
    pub app: AppKind,
    pub session_id: String,
    pub offset: usize,
    pub anchor_id: Option<String>,
    pub snapshot: Option<MessageSnapshot>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageSnapshot {
    pub total: usize,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagePreview {
    pub id: String,
    pub role: String,
    pub content: String,
    pub content_revision: String,
    pub content_length: usize,
    pub content_page_size: usize,
    pub at: Option<String>,
    pub bookmarked: Option<bool>,
    pub image_count: usize,
}

impl From<SessionMessage> for MessagePreview {
    fn from(message: SessionMessage) -> Self {
        Self { id: message.id, role: message.role, at: message.at, bookmarked: None, image_count: message.images.len(),
            content_length: message.content.chars().count(),
            content_page_size: CONTENT_PAGE_CHARS,
            content_revision: asb_switch::sha256_hex(&message.content),
            content: message.content.chars().take(PREVIEW_CHARS).collect() }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagePage {
    pub session: super::SessionMeta,
    pub items: Vec<MessagePreview>,
    pub snapshot: MessageSnapshot,
    pub offset: usize,
    pub page_size: usize,
    pub bookmark_issue: Option<LocalizedMessage>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MessageContentRequest {
    pub app: AppKind,
    pub session_id: String,
    pub message_id: String,
    pub offset: usize,
    pub content_revision: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentPage {
    pub content: String,
    pub offset: usize,
    pub total: usize,
    pub page_size: usize,
    pub images: Vec<String>,
    pub issues: Vec<String>,
}

impl ContentPage {
    pub(super) fn from_part(content: String, offset: usize, total: usize) -> Result<Self, String> {
        if offset % CONTENT_PAGE_CHARS != 0 || (offset != 0 && offset >= total)
            || content.chars().count() != total.saturating_sub(offset).min(CONTENT_PAGE_CHARS) {
            return Err("正文位置无效；请重新打开消息".into());
        }
        Ok(Self { content, offset, total, page_size: CONTENT_PAGE_CHARS, images: Vec::new(), issues: Vec::new() })
    }
}

pub(super) fn content_page(content: &str, offset: usize) -> Result<ContentPage, String> {
    let total = content.chars().count();
    ContentPage::from_part(content.chars().skip(offset).take(CONTENT_PAGE_CHARS).collect(), offset, total)
}

pub fn load_content(root: &Path, request: MessageContentRequest) -> Result<ContentPage, String> {
    let source = resolve_session(root, request.app, &request.session_id)?;
    let index = super::timeline::index(&source)?;
    let entry = index.entry(&request.message_id)?;
    let content = content_cache::read(root, &request, entry.record_revision(), || index.load(entry))?;
    let mut page = content.part(request.offset)?;
    if request.offset == 0 { (page.images, page.issues) = source.resolve_images(&content.images); }
    Ok(page)
}

pub fn load_page(root: &Path, request: MessagePageRequest) -> Result<MessagePage, String> {
    if request.offset % PAGE_SIZE != 0 { return Err("会话消息位置无效".into()); }
    let source = if request.snapshot.is_some() { super::sources::for_snapshot(root, request.app, &request.session_id)? }
        else { resolve_session(root, request.app, &request.session_id)? };
    let mut page = scan_page(&source, request.offset, request.anchor_id.as_deref(), request.snapshot.as_ref())?;
    super::organization::apply(&mut page.session, &super::organization::load(root)?);
    if let Err(detail) = bookmarks::mark_messages(root, request.app, &request.session_id, &mut page.items) {
        for item in &mut page.items { item.bookmarked = None; }
        page.bookmark_issue = Some(LocalizedMessage::new("sessions.bookmarks.readUnavailable",
            serde_json::json!({"detail":detail}), format!("会话可继续阅读，收藏状态不可用：{detail}")));
    }
    Ok(page)
}

fn message_snapshot(entries: &[Position], expected: Option<&MessageSnapshot>) -> Result<MessageSnapshot, String> {
    let total = expected.map_or(entries.len(), |snapshot| snapshot.total);
    let prefix = entries.get(..total).ok_or("会话记录已更新，请刷新会话后重试")?;
    let mut hash = Sha256::new();
    for entry in prefix {
        for value in [entry.id.as_str(), entry.record_revision()] {
            hash.update((value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        }
    }
    let revision = format!("{:x}", hash.finalize());
    if expected.is_some_and(|snapshot| snapshot.revision != revision) {
        return Err("会话记录已更新，请刷新会话后重试".into());
    }
    Ok(MessageSnapshot { total, revision })
}

fn scan_page(source: &SessionSource, requested_offset: usize, anchor: Option<&str>, expected: Option<&MessageSnapshot>) -> Result<MessagePage, String> {
    let index = super::timeline::index(source)?;
    let snapshot = message_snapshot(&index.entries, expected)?;
    let total = snapshot.total;
    let offset = if let Some(anchor) = anchor {
        index.entries[..total].iter().position(|entry| entry.id == anchor)
            .ok_or("原消息已改变或不存在；请重新搜索会话")? / PAGE_SIZE * PAGE_SIZE
    } else { requested_offset.min(total.saturating_sub(1) / PAGE_SIZE * PAGE_SIZE) };
    let mut page = MessagePage { session: source.meta.clone(), items: Vec::new(), snapshot, offset,
        page_size: PAGE_SIZE, bookmark_issue: None, issues: Vec::new() };
    page.issues = index.issues.clone();
    let end = (offset + PAGE_SIZE).min(total);
    let entries = &index.entries[offset..end];
    for (entry, result) in entries.iter().zip(index.load_batch(entries, Into::into)) {
        match result {
            Ok(message) => page.items.push(message),
            Err(error) if anchor == Some(entry.id.as_str()) => return Err(error),
            Err(error) => page.issues.push(error),
        }
    }
    Ok(page)
}

#[cfg(test)]
mod tests;
