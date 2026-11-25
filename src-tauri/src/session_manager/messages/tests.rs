use super::*;
use std::fs;

fn transcript(count: usize) -> String {
    (0..count).map(|index| format!("{}\n", serde_json::json!({"role":"user","content":format!("message {index}")}))).collect()
}

#[test]
fn early_messages_use_the_known_identity_and_complete_read_discovers_new_fragments() {
    let _guard = crate::test_client_paths::redirect_client_paths();
    let temp = tempfile::tempdir().unwrap();
    let directory = std::path::PathBuf::from(std::env::var_os("CODEX_HOME").unwrap()).join("sessions");
    fs::create_dir_all(&directory).unwrap();
    let app = AppKind::Codex;
    let id = "sample-session";
    let initial = directory.join("one.jsonl");
    let header = format!("{}\n", serde_json::json!({"type":"session_meta","payload":{"id":id}}));
    fs::write(&initial, format!("{header}{}", transcript(32))).unwrap();
    super::super::resolve_session(temp.path(), app, id).unwrap();
    let continuation = directory.join("arbitrary-name.jsonl");
    fs::write(continuation, format!("{header}{{\"role\":\"assistant\",\"content\":\"continuation\"}}\n")).unwrap();
    let first = load_first_messages(temp.path(), app, id).unwrap();
    assert_eq!(first.len(), PAGE_SIZE);
    assert_eq!(first[0].content, "message 0");
    assert!(!first[0].truncated);
    let page = load_page(temp.path(), MessagePageRequest {
        app, session_id: id.into(), offset: 0, anchor_id: None, snapshot: None,
    }).unwrap();
    assert_eq!(page.snapshot.total, 33);
    assert!(page.items.iter().any(|item| item.id == first[0].id));
    let tail = load_page(temp.path(), MessagePageRequest {
        app, session_id: id.into(), offset: 25, anchor_id: None, snapshot: Some(page.snapshot),
    }).unwrap();
    assert_eq!(tail.offset, 25);
    assert_eq!(tail.items.len(), 8);
    fs::write(initial, format!("{header}{}", transcript(32).replacen("message 0", "changed", 1))).unwrap();
    assert!(load_page(temp.path(), MessagePageRequest {
        app, session_id: id.into(), offset: 25, anchor_id: None, snapshot: Some(tail.snapshot),
    }).is_err());
}

#[test]
fn keeps_only_one_page_and_locates_an_exact_message_anchor() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    fs::write(&path, transcript(63)).unwrap();
    let first = scan_page(&crate::session_manager::fixture_source(&path, AppKind::Codex, "test"), 0, None, None).unwrap();
    assert_eq!(first.snapshot.total, 63);
    assert_eq!(first.items.len(), PAGE_SIZE);
    let last = scan_page(&crate::session_manager::fixture_source(&path, AppKind::Codex, "test"), 50, None, None).unwrap();
    assert_eq!(last.items.len(), 13);
    let anchored = scan_page(&crate::session_manager::fixture_source(&path, AppKind::Codex, "test"), 0, Some(&last.items[5].id), None).unwrap();
    assert_eq!(anchored.offset, 50);
    assert_eq!(anchored.items[5].content, "message 55");
    assert!(scan_page(&crate::session_manager::fixture_source(&path, AppKind::Codex, "test"), 0, Some("changed-id"), None).is_err());
    assert_eq!(scan_page(&crate::session_manager::fixture_source(&path, AppKind::Codex, "test"), 100, None, None).unwrap().offset, 50);
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
    let source = crate::session_manager::fixture_source(&path, AppKind::Codex, "test");
    let original = scan_page(&source, 25, None, None).unwrap();
    let anchor = &original.items[0].id;
    fs::write(&path, transcript(35)).unwrap();
    assert_eq!(scan_page(&source, 0, Some(anchor), None).unwrap().offset, 25);
    let continued = scan_page(&source, 25, None, Some(&original.snapshot)).unwrap();
    assert_eq!(continued.snapshot.total, 30);
    assert_eq!(continued.items.len(), 5);
    fs::write(&path, transcript(35).replacen("message 0", "edited", 1)).unwrap();
    assert!(scan_page(&source, 0, Some(anchor), None).is_err());
    assert!(scan_page(&source, 25, None, Some(&original.snapshot)).is_err());
}

#[test]
fn a_warm_message_page_decodes_only_that_page_and_single_message_reads_seek_directly() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    fs::write(&path, transcript(63)).unwrap();
    let source = crate::session_manager::fixture_source(&path, AppKind::Codex, "test");
    scan_page(&source, 0, None, None).unwrap();
    let before = crate::session_manager::records::decoded_record_count();
    let page = scan_page(&source, 25, None, None).unwrap();
    assert_eq!(crate::session_manager::records::decoded_record_count() - before, 25);
    let before = crate::session_manager::records::decoded_record_count();
    let message = source.find(&page.items[24].id).unwrap();
    assert_eq!(message.content, "message 49");
    assert_eq!(crate::session_manager::records::decoded_record_count() - before, 1);
}
