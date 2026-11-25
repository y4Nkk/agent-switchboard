use super::{ContentPage, MessageContentRequest, SessionMessage, CONTENT_PAGE_CHARS};
use asb_core::contracts::AppKind;
use std::{path::{Path, PathBuf}, sync::{Arc, Mutex}};

const MAX_CACHED_BYTES: usize = 32 * 1024 * 1024;

#[derive(PartialEq, Eq)]
struct Key {
    root: PathBuf,
    app: AppKind,
    session_id: String,
    message_id: String,
    record_revision: String,
    content_revision: String,
}

static CACHE: Mutex<Option<(Key, Arc<ChunkedContent>)>> = Mutex::new(None);

pub(super) struct ChunkedContent {
    text: String,
    starts: Vec<usize>,
    chars: usize,
    pub images: Vec<String>,
}

impl ChunkedContent {
    fn new(message: SessionMessage) -> Self {
        let mut starts = vec![0];
        let mut chars = 0;
        for (byte, _) in message.content.char_indices() {
            if chars > 0 && chars % CONTENT_PAGE_CHARS == 0 { starts.push(byte); }
            chars += 1;
        }
        Self { text: message.content, starts, chars, images: message.images }
    }

    pub(super) fn part(&self, offset: usize) -> Result<ContentPage, String> {
        if offset % CONTENT_PAGE_CHARS != 0 { return Err("正文位置无效；请重新打开消息".into()); }
        let index = offset / CONTENT_PAGE_CHARS;
        let start = self.starts.get(index).copied().ok_or("正文位置无效；请重新打开消息")?;
        let end = self.starts.get(index + 1).copied().unwrap_or(self.text.len());
        ContentPage::from_part(self.text[start..end].to_owned(), offset, self.chars)
    }

    fn fits_cache(&self) -> bool {
        let bytes = self.text.capacity() + self.starts.capacity() * std::mem::size_of::<usize>()
            + self.images.capacity() * std::mem::size_of::<String>()
            + self.images.iter().map(String::capacity).sum::<usize>();
        self.starts.len() > 1 && bytes <= MAX_CACHED_BYTES
    }
}

pub(super) fn read(root: &Path, request: &MessageContentRequest, record_revision: &str,
    load: impl FnOnce() -> Result<SessionMessage, String>,
) -> Result<Arc<ChunkedContent>, String> {
    let key = Key { root: root.into(), app: request.app, session_id: request.session_id.clone(),
        message_id: request.message_id.clone(), record_revision: record_revision.into(),
        content_revision: request.content_revision.clone() };
    if let Ok(cache) = CACHE.lock() {
        if let Some((_, text)) = cache.as_ref().filter(|(stored, _)| stored == &key) {
            return Ok(Arc::clone(text));
        }
    }
    let message = load()?;
    if asb_switch::sha256_hex(&message.content) != request.content_revision {
        return Err("原消息正文已改变，请刷新会话后重试".into());
    }
    let content = Arc::new(ChunkedContent::new(message));
    if content.fits_cache() {
        if let Ok(mut cache) = CACHE.lock() { *cache = Some((key, Arc::clone(&content))); }
    }
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_index_preserves_multibyte_text_and_validates_boundaries() {
        let text = format!("{}末尾", "😀".repeat(CONTENT_PAGE_CHARS));
        let content = ChunkedContent::new(SessionMessage {
            id: "message".into(), role: "assistant".into(), content: text.clone(),
            at: None, images: Vec::new(),
        });
        let first = content.part(0).unwrap();
        let end = content.part(CONTENT_PAGE_CHARS).unwrap();
        assert_eq!(first.content + &end.content, text);
        assert_eq!(end.total, CONTENT_PAGE_CHARS + 2);
        assert!(content.part(1).is_err());
        assert!(content.part(2 * CONTENT_PAGE_CHARS).is_err());
    }
}
