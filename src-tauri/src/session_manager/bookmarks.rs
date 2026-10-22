use super::{messages::{aligned_content_offset, content_page, ContentPage, MessagePreview, PREVIEW_CHARS},
    organization, resolve_session, text::{raw_offset, TextQuery}, SessionMessage, SessionMeta};
#[cfg(test)]
use super::parser::collect_messages;
use asb_core::contracts::AppKind;
use chrono::Utc;
use rusqlite::{params, TransactionBehavior};
#[cfg(test)]
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

mod store;
use store::{database_path, open_store, database_error, COLUMNS};
#[cfg(test)]
use store::SCHEMA;
#[derive(Debug, Clone)]
struct SessionBookmark {
    pub id: String,
    pub app: AppKind,
    pub session_id: String,
    pub session_title: String,
    pub project_dir: Option<String>,
    pub message_id: String,
    pub role: String,
    pub content: String,
    pub at: Option<String>,
    pub saved_at: String,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkSummary {
    pub image_count: usize,
    pub id: String,
    pub app: AppKind,
    pub session_id: String,
    pub session_title: String,
    pub project_dir: Option<String>,
    pub message_id: String,
    pub role: String,
    pub excerpt: String,
    pub content_length: usize,
    pub content_offset: usize,
    pub at: Option<String>,
    pub saved_at: String,
}

impl From<SessionBookmark> for BookmarkSummary {
    fn from(bookmark: SessionBookmark) -> Self {
        Self { image_count: bookmark.images.len(), id: bookmark.id, app: bookmark.app, session_id: bookmark.session_id,
            session_title: bookmark.session_title, project_dir: bookmark.project_dir,
            message_id: bookmark.message_id, role: bookmark.role, at: bookmark.at,
            saved_at: bookmark.saved_at, content_length: bookmark.content.chars().count(), content_offset: 0,
            excerpt: bookmark.content.chars().take(PREVIEW_CHARS).collect() }
    }
}

const PAGE_SIZE: usize = 50;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BookmarkSearchRequest {
    pub query: String,
    pub app: Option<AppKind>,
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkPage {
    pub items: Vec<BookmarkSummary>,
    pub total: usize,
    pub offset: usize,
    pub page_size: usize,
}

pub fn search_bookmarks(root: &Path, request: BookmarkSearchRequest) -> Result<BookmarkPage, String> {
    if request.offset % PAGE_SIZE != 0 || request.query.chars().count() > 4000 {
        return Err("收藏查询页码或搜索内容无效".into());
    }
    let mut page = BookmarkPage { items: Vec::new(), total: 0, offset: request.offset, page_size: PAGE_SIZE };
    if !database_path(root).exists() { return Ok(page); }
    let connection = open_store(root)?;
    let mut statement = connection.prepare(&format!(
        "SELECT {COLUMNS},json_array_length(images) FROM bookmarks INDEXED BY bookmarks_saved_at WHERE (?1 IS NULL OR app=?1) ORDER BY saved_at DESC, id ASC"
    )).map_err(database_error)?;
    let query = TextQuery::new(&request.query);
    let mut rows = statement.query(params![request.app.map(|app| app.dir_name())]).map_err(database_error)?;
    while let Some(row) = rows.next().map_err(database_error)? {
        if !bookmark_matches(row, &query).map_err(database_error)? { continue; }
        if page.total >= page.offset && page.items.len() < PAGE_SIZE {
            let mut item = summary_from_row(row).map_err(database_error)?;
            let content = borrowed_text(row, 7).map_err(database_error)?;
            if !query.is_empty() {
                if let Some(position) = query.find(content) {
                    item.content_offset = aligned_content_offset(raw_offset(content, position).ok_or("搜索收藏位置无效")?);
                }
            }
            page.items.push(item);
        }
        page.total += 1;
    }
    Ok(page)
}

fn bookmark_matches(row: &rusqlite::Row<'_>, query: &TextQuery) -> rusqlite::Result<bool> {
    if query.is_empty() { return Ok(true); }
    for index in [3, 4, 7] {
        if let rusqlite::types::ValueRef::Text(text) = row.get_ref(index)? {
            let text = std::str::from_utf8(text).map_err(|_| rusqlite::Error::InvalidQuery)?;
            if query.find(text).is_some() { return Ok(true); }
        }
    }
    Ok(false)
}

pub fn load_bookmark_content(root: &Path, id: &str, offset: usize) -> Result<ContentPage, String> {
    if !database_path(root).exists() { return Err("收藏已不存在".into()); }
    let connection = open_store(root)?;
    connection.query_row("SELECT content,images FROM bookmarks WHERE id=?1", [id], |row| {
        let content = borrowed_text(row, 0)?;
        let images = if offset == 0 { snapshot_images(row, 1)? } else { Vec::new() };
        Ok(content_page(content, offset).map(|mut page| { page.images = images; page }))
    }).map_err(database_error)?
}

pub(super) fn load_copy_text(root: &Path, id: &str) -> Result<String, String> {
    if !database_path(root).exists() { return Err("收藏已不存在".into()); }
    let message = open_store(root)?.query_row("SELECT content,images FROM bookmarks WHERE id=?1", [id], |row|
        Ok(SessionMessage { id: id.to_string(), role: String::new(), at: None,
            content: row.get(0)?, images: snapshot_images(row, 1)? })).map_err(database_error)?;
    super::images::copy_text(&message)
}

pub(super) fn mark_messages(root: &Path, app: AppKind, session: &str, items: &mut [MessagePreview]) -> Result<(), String> {
    if !database_path(root).exists() { for item in items { item.bookmarked = Some(false); } return Ok(()); }
    let connection = open_store(root)?;
    let mut query = connection.prepare(
        "SELECT EXISTS(SELECT 1 FROM bookmarks WHERE app=?1 AND session_id=?2 AND message_id=?3)"
    ).map_err(database_error)?;
    for item in items {
        item.bookmarked = Some(query.query_row(params![app.dir_name(), session, item.id], |row| row.get(0)).map_err(database_error)?);
    }
    Ok(())
}

pub fn save_bookmark(
    root: &Path,
    app: AppKind,
    session_id: &str,
    message_id: &str,
) -> Result<BookmarkSummary, String> {
    let mut source = resolve_session(root, app, session_id)?;
    organization::apply(&mut source.meta, &organization::load(root)?);
    let mut message = source.find(message_id)?;
    let (images, issues) = source.resolve_images(&message.images);
    if !issues.is_empty() { return Err(issues.join("；")); }
    message.images = images;
    save_snapshot(root, &source.meta, &message).map(Into::into)
}

fn save_snapshot(root: &Path, meta: &SessionMeta, message: &SessionMessage) -> Result<SessionBookmark, String> {
    if !matches!(message.role.as_str(), "user" | "assistant") {
        return Err("只能收藏用户或助手消息".into());
    }
    let (images, issues) = super::images::resolve(&message.images);
    if !issues.is_empty() { return Err(issues.join("；")); }
    let images = serde_json::to_string(&images).map_err(|error| error.to_string())?;
    if message.content.len() + images.len() > super::records::MAX_RECORD_BYTES as usize {
        return Err("收藏正文与图片超过 32 MiB 读取上限".into());
    }
    let identity = format!("{}\0{}\0{}", meta.app.dir_name(), meta.session_id, message.id);
    let id = format!("{:x}", Sha256::digest(identity.as_bytes()));
    let mut connection = open_store(root)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(database_error)?;
    transaction.execute(
        "INSERT INTO bookmarks (id, app, session_id, session_title, project_dir, message_id, role, content, at, saved_at, images)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(app, session_id, message_id) DO NOTHING",
        params![id, meta.app.dir_name(), meta.session_id, meta.alias.as_deref().unwrap_or(&meta.title), meta.project_dir,
            message.id, message.role, message.content, message.at, Utc::now().to_rfc3339(), images],
    ).map_err(database_error)?;
    let bookmark = transaction.query_row(
        &format!("SELECT {COLUMNS},images FROM bookmarks WHERE id = ?1"), [&id], from_row,
    ).map_err(database_error)?;
    transaction.commit().map_err(database_error)?;
    Ok(bookmark)
}

pub fn delete_bookmark(root: &Path, id: &str) -> Result<crate::storage::ReclaimOutcome, String> {
    if !database_path(root).exists() { return Ok(crate::storage::ReclaimOutcome::default()); }
    let connection = open_store(root)?;
    let removed = connection.execute("DELETE FROM bookmarks WHERE id = ?1", [id]).map_err(database_error)?;
    Ok(crate::storage::ReclaimOutcome { removed: removed as u64, warning: crate::storage::sqlite::compact(&connection).err() })
}

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionBookmark> {
    let app: String = row.get(1)?;
    let app = match app.as_str() {
        "codex" => AppKind::Codex,
        "claude" => AppKind::Claude,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(SessionBookmark { id: row.get(0)?, app, session_id: row.get(2)?,
        session_title: row.get(3)?, project_dir: row.get(4)?, message_id: row.get(5)?,
        role: row.get(6)?, content: row.get(7)?, at: row.get(8)?, saved_at: row.get(9)?, images: snapshot_images(row, 10)? })
}

fn snapshot_images(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Vec<String>> {
    let encoded: String = row.get(index)?;
    let images: Vec<String> = serde_json::from_str(&encoded).map_err(|error|
        rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(error)))?;
    if images.iter().any(|image| !image.starts_with("data:image/")) { return Err(rusqlite::Error::InvalidQuery); }
    Ok(images)
}

fn borrowed_text<'a>(row: &'a rusqlite::Row<'_>, index: usize) -> rusqlite::Result<&'a str> {
    let bytes = match row.get_ref(index)? {
        rusqlite::types::ValueRef::Text(bytes) => bytes,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    std::str::from_utf8(bytes).map_err(|error| rusqlite::Error::FromSqlConversionFailure(
        index, rusqlite::types::Type::Text, Box::new(error)))
}

fn summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<BookmarkSummary> {
    let app = match borrowed_text(row, 1)? {
        "codex" => AppKind::Codex, "claude" => AppKind::Claude,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let content = borrowed_text(row, 7)?;
    Ok(BookmarkSummary { image_count: row.get(10)?, id: row.get(0)?, app, session_id: row.get(2)?, session_title: row.get(3)?,
        project_dir: row.get(4)?, message_id: row.get(5)?, role: row.get(6)?,
        excerpt: content.chars().take(PREVIEW_CHARS).collect(), content_length: content.chars().count(),
        content_offset: 0, at: row.get(8)?, saved_at: row.get(9)? })
}

#[cfg(test)]
mod tests;
