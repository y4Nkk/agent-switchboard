use super::{bookmarks, resolve_session, SessionMessage, SessionSource};
use asb_core::contracts::{AppKind, LocalizedMessage};
use serde::{Deserialize, Serialize};
use std::path::Path;

const PAGE_SIZE: usize = 25;
pub(super) const PREVIEW_CHARS: usize = 1200;
pub(super) const CONTENT_PAGE_CHARS: usize = 16_384;

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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagePreview {
    pub id: String,
    pub role: String,
    pub content: String,
    pub content_length: usize,
    pub at: Option<String>,
    pub bookmarked: Option<bool>,
    pub image_count: usize,
}

impl From<SessionMessage> for MessagePreview {
    fn from(message: SessionMessage) -> Self {
        Self { id: message.id, role: message.role, at: message.at, bookmarked: None, image_count: message.images.len(),
            content_length: message.content.chars().count(),
            content: message.content.chars().take(PREVIEW_CHARS).collect() }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagePage {
    pub session: super::SessionMeta,
    pub items: Vec<MessagePreview>,
    pub total: usize,
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
            return Err("正文页码无效；请重新打开消息".into());
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
    let message = source.find(&request.message_id)?;
    let mut page = content_page(&message.content, request.offset)?;
    if request.offset == 0 { (page.images, page.issues) = source.resolve_images(&message.images); }
    Ok(page)
}

pub fn load_page(root: &Path, request: MessagePageRequest) -> Result<MessagePage, String> {
    if request.offset % PAGE_SIZE != 0 { return Err("消息页码无效".into()); }
    let source = resolve_session(root, request.app, &request.session_id)?;
    let mut page = scan_page(&source, request.offset, request.anchor_id.as_deref())?;
    super::organization::apply(&mut page.session, &super::organization::load(root)?);
    if let Err(detail) = bookmarks::mark_messages(root, request.app, &request.session_id, &mut page.items) {
        for item in &mut page.items { item.bookmarked = None; }
        page.bookmark_issue = Some(LocalizedMessage::new("sessions.bookmarks.readUnavailable",
            serde_json::json!({"detail":detail}), format!("会话可继续阅读，收藏状态不可用：{detail}")));
    }
    Ok(page)
}

fn scan_page(source: &SessionSource, requested_offset: usize, anchor: Option<&str>) -> Result<MessagePage, String> {
    let index = super::timeline::index(source)?;
    let total = index.entries.len();
    let offset = if let Some(anchor) = anchor {
        index.entries.iter().position(|entry| entry.id == anchor)
            .ok_or("原消息已改变或不存在；请重新搜索会话")? / PAGE_SIZE * PAGE_SIZE
    } else { requested_offset.min(total.saturating_sub(1) / PAGE_SIZE * PAGE_SIZE) };
    let mut page = MessagePage { session: source.meta.clone(), items: Vec::new(), total, offset,
        page_size: PAGE_SIZE, bookmark_issue: None, issues: Vec::new() };
    page.issues = index.issues.clone();
    for entry in index.entries.iter().skip(offset).take(PAGE_SIZE) {
        match index.load(entry) {
            Ok(message) => page.items.push(message.into()),
            Err(error) => page.issues.push(error),
        }
    }
    Ok(page)
}

#[cfg(test)]
mod tests;
