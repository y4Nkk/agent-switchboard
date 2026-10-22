use super::{bookmarks, resolve_session};
use asb_core::contracts::AppKind;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum SessionCopySource {
    Session { app: AppKind, session_id: String, message_id: String },
    Bookmark { id: String },
}

pub fn load_copy_text(root: &Path, source: SessionCopySource) -> Result<String, String> {
    match source {
        SessionCopySource::Session { app, session_id, message_id } => {
            let source = resolve_session(root, app, &session_id)?;
            source.copy_text(&source.find(&message_id)?)
        }
        SessionCopySource::Bookmark { id } => bookmarks::load_copy_text(root, &id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_current_typed_clipboard_source() {
        assert!(serde_json::from_value::<SessionCopySource>(serde_json::json!({
            "kind":"session","app":"codex","sessionId":"s-1","messageId":"m-1"
        })).is_ok());
        assert!(serde_json::from_value::<SessionCopySource>(serde_json::json!({
            "kind":"bookmark","id":"id","content":"injected text"
        })).is_err());
    }
}
