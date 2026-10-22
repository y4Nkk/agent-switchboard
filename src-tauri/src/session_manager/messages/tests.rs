use super::*;
use std::fs;

fn transcript(count: usize) -> String {
    (0..count).map(|index| format!("{}\n", serde_json::json!({"role":"user","content":format!("message {index}")}))).collect()
}

#[test]
fn keeps_only_one_page_and_locates_an_exact_message_anchor() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    fs::write(&path, transcript(63)).unwrap();
    let first = scan_page(&crate::session_manager::fixture_source(&path, AppKind::Codex, "test"), 0, None).unwrap();
    assert_eq!(first.total, 63);
    assert_eq!(first.items.len(), PAGE_SIZE);
    let last = scan_page(&crate::session_manager::fixture_source(&path, AppKind::Codex, "test"), 50, None).unwrap();
    assert_eq!(last.items.len(), 13);
    let anchored = scan_page(&crate::session_manager::fixture_source(&path, AppKind::Codex, "test"), 0, Some(&last.items[5].id)).unwrap();
    assert_eq!(anchored.offset, 50);
    assert_eq!(anchored.items[5].content, "message 55");
    assert!(scan_page(&crate::session_manager::fixture_source(&path, AppKind::Codex, "test"), 0, Some("changed-id")).is_err());
    assert_eq!(scan_page(&crate::session_manager::fixture_source(&path, AppKind::Codex, "test"), 100, None).unwrap().offset, 50);
}

#[test]
fn preview_and_content_pages_preserve_unicode_and_nul_boundaries() {
    let content = "🙂中\0".repeat(9000);
    let preview = MessagePreview::from(SessionMessage { id: "id".into(), role: "assistant".into(),
        content: content.clone(), at: None, images: Vec::new() });
    assert_eq!(preview.content.chars().count(), PREVIEW_CHARS);
    assert_eq!(preview.content_length, 27_000);
    let first = content_page(&content, 0).unwrap();
    let second = content_page(&content, CONTENT_PAGE_CHARS).unwrap();
    assert_eq!(first.content.chars().count(), CONTENT_PAGE_CHARS);
    assert_eq!(first.content + &second.content, content);
    assert!(content_page("short", CONTENT_PAGE_CHARS).is_err());
    assert!(content_page("short", 1).is_err());
}

#[test]
fn appended_records_preserve_anchors_and_prefix_edits_invalidate_them() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    fs::write(&path, transcript(30)).unwrap();
    let anchor = scan_page(&crate::session_manager::fixture_source(&path, AppKind::Codex, "test"), 25, None).unwrap().items[0].id.clone();
    fs::write(&path, transcript(35)).unwrap();
    assert_eq!(scan_page(&crate::session_manager::fixture_source(&path, AppKind::Codex, "test"), 0, Some(&anchor)).unwrap().offset, 25);
    fs::write(&path, transcript(35).replacen("message 0", "edited", 1)).unwrap();
    assert!(scan_page(&crate::session_manager::fixture_source(&path, AppKind::Codex, "test"), 0, Some(&anchor)).is_err());
}

#[test]
fn a_warm_message_page_decodes_only_that_page_and_single_message_reads_seek_directly() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    fs::write(&path, transcript(63)).unwrap();
    let source = crate::session_manager::fixture_source(&path, AppKind::Codex, "test");
    scan_page(&source, 0, None).unwrap();
    let before = crate::session_manager::records::decoded_record_count();
    let page = scan_page(&source, 25, None).unwrap();
    assert_eq!(crate::session_manager::records::decoded_record_count() - before, 25);
    let before = crate::session_manager::records::decoded_record_count();
    let message = source.find(&page.items[24].id).unwrap();
    assert_eq!(message.content, "message 49");
    assert_eq!(crate::session_manager::records::decoded_record_count() - before, 1);
}
