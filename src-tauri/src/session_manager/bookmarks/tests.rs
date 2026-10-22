use super::*;
use std::fs;
use tempfile::tempdir;

fn snapshot() -> (SessionMeta, SessionMessage) {
    (SessionMeta { app: AppKind::Codex, session_id: "s1".into(), title: "会话标题".into(),
        summary: "摘要".into(), project_dir: Some("/project".into()),
        created_at: None, last_active_at: None, resume_command: None,
        alias: None, pinned: false, tags: Vec::new(), parent_session_id: None, can_delete: true, preservation: None },
    SessionMessage { id: "message-hash".into(), role: "assistant".into(),
        content: "完整消息\n```rust\nlet x = 1;\n```".into(), at: None, images: Vec::new() })
}

#[test]
fn bookmarks_are_idempotent_persistent_snapshots() {
    let temp = tempdir().unwrap();
    assert!(search_bookmarks(temp.path(), BookmarkSearchRequest::default()).unwrap().items.is_empty());
    assert!(!database_path(temp.path()).exists());
    let (meta, message) = snapshot();
    let first = save_snapshot(temp.path(), &meta, &message).unwrap();
    let mut changed = meta.clone();
    changed.title = "新标题".into();
    let second = save_snapshot(temp.path(), &changed, &message).unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(first.saved_at, second.saved_at);
    assert_eq!(second.session_title, meta.title);
    let rows = search_bookmarks(temp.path(), BookmarkSearchRequest::default()).unwrap().items;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].excerpt, message.content);
    delete_bookmark(temp.path(), &first.id).unwrap();
    assert!(search_bookmarks(temp.path(), BookmarkSearchRequest::default()).unwrap().items.is_empty());
}

#[test]
fn rejects_system_messages_and_does_not_replace_invalid_database() {
    let temp = tempdir().unwrap();
    let (meta, mut message) = snapshot();
    message.role = "system".into();
    assert!(save_snapshot(temp.path(), &meta, &message).is_err());
    let path = database_path(temp.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"corrupted database").unwrap();
    assert!(search_bookmarks(temp.path(), BookmarkSearchRequest::default()).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"corrupted database");
}

#[test]
fn rejects_foreign_schema_without_migration() {
    let temp = tempdir().unwrap();
    let path = database_path(temp.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE previous(content TEXT); PRAGMA user_version=9;").unwrap();
    drop(connection);
    assert!(search_bookmarks(temp.path(), BookmarkSearchRequest::default()).unwrap_err().contains("结构无效"));
    let connection = Connection::open(&path).unwrap();
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap();
    assert_eq!(version, 9);
}

#[test]
fn concurrent_duplicate_saves_keep_one_row() {
    let temp = tempdir().unwrap();
    let (meta, message) = snapshot();
    let root = temp.path().to_path_buf();
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let first = scope.spawn(|| {
            barrier.wait();
            save_snapshot(&root, &meta, &message).unwrap()
        });
        let second = scope.spawn(|| {
            barrier.wait();
            save_snapshot(&root, &meta, &message).unwrap()
        });
        assert_eq!(first.join().unwrap().id, second.join().unwrap().id);
    });
    assert_eq!(search_bookmarks(temp.path(), BookmarkSearchRequest::default()).unwrap().items.len(), 1);
}

#[test]
fn rejects_existing_empty_database_without_initializing_it() {
    let temp = tempdir().unwrap();
    let path = database_path(temp.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"").unwrap();
    let (meta, message) = snapshot();
    assert!(save_snapshot(temp.path(), &meta, &message).unwrap_err().contains("结构无效"));
    assert_eq!(fs::metadata(path).unwrap().len(), 0);
}

#[test]
fn stored_schema_matches_the_single_current_definition_after_reopening() {
    let temp = tempdir().unwrap();
    let connection = open_store(temp.path()).unwrap();
    let stored: String = connection.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='bookmarks'",
        [], |row| row.get(0),
    ).unwrap();
    assert_eq!(stored, SCHEMA);
    drop(connection);
    open_store(temp.path()).unwrap();
}

#[test]
fn parsed_tool_output_cannot_be_bookmarked() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    fs::write(&path, concat!(
        "{\"type\":\"response_item\",\"payload\":{\"type\":\"function_call_output\",\"output\":\"error text\"}}\n",
        "{\"type\":\"response_item\",\"payload\":{\"type\":\"custom_tool_call_output\",\"output\":\"patch error\"}}\n",
    )).unwrap();
    let (meta, _) = snapshot();
    for message in collect_messages(&path).unwrap() {
        assert_eq!(message.role, "tool");
        assert!(save_snapshot(temp.path(), &meta, &message).unwrap_err().contains("只能收藏"));
    }
    assert!(!database_path(temp.path()).exists());
}

#[test]
fn bookmark_queries_are_paged_and_content_reads_preserve_nul_and_unicode() {
    let temp = tempdir().unwrap();
    let (meta, mut message) = snapshot();
    message.content = format!("head\0{}\nÄPFEL  连接", "🙂中".repeat(9000));
    let saved = save_snapshot(temp.path(), &meta, &message).unwrap();
    for index in 0..55 {
        let mut next = message.clone(); next.id = format!("message-{index}");
        save_snapshot(temp.path(), &meta, &next).unwrap();
    }
    let page = search_bookmarks(temp.path(), BookmarkSearchRequest {
        query: "äpfel 连接".into(), app: Some(AppKind::Codex), offset: 0,
    }).unwrap();
    assert_eq!(page.total, 56);
    assert_eq!(page.items.len(), PAGE_SIZE);
    assert_eq!(page.items[0].excerpt.chars().count(), PREVIEW_CHARS);
    assert_eq!(page.items[0].content_length, message.content.chars().count());
    assert!(page.items[0].content_offset > 0);
    let matched = load_bookmark_content(temp.path(), &page.items[0].id, page.items[0].content_offset).unwrap();
    assert!(matched.content.contains("ÄPFEL  连接"));
    let first = load_bookmark_content(temp.path(), &saved.id, 0).unwrap();
    let second = load_bookmark_content(temp.path(), &saved.id, first.page_size).unwrap();
    assert_eq!(first.content + &second.content, message.content);
    assert_eq!(search_bookmarks(temp.path(), BookmarkSearchRequest {
        query: "äpfel 连接".into(), app: None, offset: PAGE_SIZE,
    }).unwrap().items.len(), 6);
}

#[test]
fn saved_images_are_independent_of_the_original_and_do_not_distort_text_pages() {
    let temp = tempdir().unwrap();
    let (meta, mut message) = snapshot();
    message.images = vec!["data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aGN8AAAAASUVORK5CYII=".into()];
    let saved = save_snapshot(temp.path(), &meta, &message).unwrap();
    let page = load_bookmark_content(temp.path(), &saved.id, 0).unwrap();
    assert_eq!(page.content, message.content);
    assert_eq!(page.images, message.images);
    assert_eq!(page.total, message.content.chars().count());
    let summary = search_bookmarks(temp.path(), BookmarkSearchRequest::default()).unwrap();
    assert_eq!(summary.items[0].image_count, 1);
    assert_eq!(summary.items[0].excerpt, message.content);
    assert!(load_copy_text(temp.path(), &saved.id).unwrap().contains("![会话图片](data:image/png;"));
}

#[test]
fn leading_nul_is_nonempty_content_and_survives_a_snapshot() {
    let temp = tempdir().unwrap();
    let (meta, mut message) = snapshot();
    message.content = "\0original".into();
    let saved = save_snapshot(temp.path(), &meta, &message).unwrap();
    assert_eq!(load_bookmark_content(temp.path(), &saved.id, 0).unwrap().content, message.content);
}

#[test]
fn v1_snapshot_upgrade_preserves_ids_and_text_and_installs_one_current_schema() {
    let temp = tempdir().unwrap();
    let path = database_path(temp.path());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch(store::SCHEMA_V1).unwrap();
    connection.execute_batch("PRAGMA user_version=1").unwrap();
    connection.execute("INSERT INTO bookmarks VALUES('kept','codex','session','title',NULL,'message','user','original text',NULL,'2026-10-16T00:00:00Z')", []).unwrap();
    drop(connection);
    let page = load_bookmark_content(temp.path(), "kept", 0).unwrap();
    assert_eq!(page.content, "original text");
    assert!(page.images.is_empty());
    let connection = open_store(temp.path()).unwrap();
    assert_eq!(connection.query_row::<i64,_,_>("PRAGMA user_version", [], |row| row.get(0)).unwrap(), 2);
    assert_eq!(connection.query_row::<String,_,_>("SELECT sql FROM sqlite_master WHERE name='bookmarks'", [], |row| row.get(0)).unwrap(), SCHEMA);
    assert_eq!(connection.query_row::<i64,_,_>("SELECT COUNT(*) FROM sqlite_master WHERE name='bookmarks_previous'", [], |row| row.get(0)).unwrap(), 0);
}
