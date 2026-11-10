use super::*;
use crate::session_manager::{self as sessions, SessionSearchRequest};
use serde_json::json;
use std::fs;

fn request(app: Option<AppKind>, archived: bool) -> Request {
    Request { app, archived, active_after: "2026-11-05T00:00:00+08:00".into(),
        active_before: "2026-11-06T00:00:00+08:00".into() }
}

fn id(index: usize) -> String { format!("44444444-5555-4666-8777-{index:012}") }

fn write(claude: &Path, index: usize, at: Option<&str>, text: &str) -> PathBuf {
    let path = claude.join("projects/range").join(format!("{}.jsonl", id(index)));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut value = json!({"type":"user","sessionId":id(index),"uuid":id(index+1000),
        "cwd":"/range-project","message":{"role":"user","content":text}});
    if let Some(at) = at { value["timestamp"] = json!(at); }
    fs::write(&path, format!("{value}\n")).unwrap();
    path
}

fn fixture(count: usize, run: impl FnOnce(&Path, &Path)) {
    let _clients = crate::test_client_paths::redirect_client_paths();
    let root = tempfile::tempdir().unwrap();
    let claude = PathBuf::from(std::env::var_os("CLAUDE_CONFIG_DIR").unwrap());
    for index in 0..count { write(&claude, index, Some("2026-11-05T04:00:00Z"), "range fixture"); }
    run(root.path(), &claude);
}

#[test]
fn range_contract_requires_current_fields_and_valid_ordered_dates() {
    assert!(snapshot::dates(&request(None, false)).is_ok());
    let mut invalid = request(None, false); invalid.active_before = invalid.active_after.clone();
    assert!(snapshot::dates(&invalid).is_err());
    invalid.active_after = "yesterday".into(); assert!(snapshot::dates(&invalid).is_err());
    assert!(serde_json::from_value::<Request>(json!({"app":null,"activeAfter":"2026-11-05T00:00:00Z","activeBefore":"2026-11-06T00:00:00Z"})).is_err());
    assert!(serde_json::from_value::<Request>(json!({"app":null,"archived":false,"activeAfter":"x","activeBefore":"y","cutoff":"x"})).is_err());
}

#[test]
fn previews_complete_range_across_pages_and_respects_local_day_boundaries() {
    fixture(75, |root, claude| {
        write(claude, 80, Some("2026-11-04T15:59:59Z"), "before");
        write(claude, 81, Some("2026-11-04T16:00:00Z"), "start inclusive");
        write(claude, 82, Some("2026-11-05T15:59:59Z"), "last second");
        write(claude, 83, Some("2026-11-05T16:00:00Z"), "end exclusive");
        write(claude, 84, None, "unknown activity");
        let operations = Operations::default();
        let preview = operations.preview(root, request(Some(AppKind::Claude), false), Action::Archive).unwrap();
        assert!(preview.issues.is_empty(), "{:?}", preview.issues);
        assert_eq!(preview.matched, 77); assert_eq!(preview.items.len(), 77);
        assert!(preview.items.iter().all(|item| item.session.app == AppKind::Claude));
        assert!(!preview.items.iter().any(|item| [id(80), id(83), id(84)].contains(&item.session.session_id)));
    });
}

#[test]
fn archive_restore_and_archived_search_preserve_originals_and_organization() {
    fixture(55, |root, claude| {
        let original = fs::read(claude.join("projects/range").join(format!("{}.jsonl", id(0)))).unwrap();
        sessions::update_session_organization(root, &[sessions::SessionDeleteRequest { app: AppKind::Claude, session_id: id(0) }],
            &sessions::SessionOrganizationChange::Pin { pinned: true }).unwrap();
        let operations = Operations::default();
        let preview = operations.preview(root, request(None, false), Action::Archive).unwrap();
        assert!(operations.execute(root, &preview.id, false).is_err());
        let result = operations.execute(root, &preview.id, true).unwrap();
        assert_eq!(result.id.as_deref(), Some(preview.id.as_str()));
        assert_eq!(result.completed, 55); assert!(result.outcomes.iter().all(|item| item.completed && item.error.is_none()));
        let current = sessions::search_sessions(root, SessionSearchRequest::default()).unwrap(); assert_eq!(current.total, 0);
        let archived = sessions::search_sessions(root, SessionSearchRequest { archived: true, ..Default::default() }).unwrap();
        assert_eq!(archived.total, 55); assert_eq!(archived.results.len(), 50);
        let location = sessions::load_metadata(root, AppKind::Claude, &id(0)).unwrap(); assert!(location.archived && location.session.pinned);
        let preview = operations.preview(root, request(None, true), Action::Unarchive).unwrap();
        assert_eq!(operations.execute(root, &preview.id, true).unwrap().completed, 55);
        assert_eq!(sessions::search_sessions(root, SessionSearchRequest::default()).unwrap().total, 55);
        assert_eq!(fs::read(claude.join("projects/range").join(format!("{}.jsonl", id(0)))).unwrap(), original);
    });
}

#[test]
fn cancelling_or_replacing_preview_revokes_its_only_execution_id() {
    fixture(1, |root, _| {
        let operations = Operations::default();
        let first = operations.preview(root, request(None, false), Action::Archive).unwrap();
        let second = operations.preview(root, request(None, false), Action::Archive).unwrap();
        assert!(operations.execute(root, &first.id, true).is_err());
        operations.discard(&second.id).unwrap(); assert!(operations.execute(root, &second.id, true).is_err());
        assert_eq!(sessions::search_sessions(root, SessionSearchRequest::default()).unwrap().total, 1);
    });
}

#[test]
fn newly_matching_independent_sessions_are_not_added_after_confirmation() {
    fixture(1, |root, claude| {
        let operations = Operations::default();
        let preview = operations.preview(root, request(None, false), Action::Archive).unwrap();
        write(claude, 1, Some("2026-11-05T05:00:00Z"), "new matching session");
        let result = operations.execute(root, &preview.id, true).unwrap(); assert_eq!(result.completed, 1);
        let current = sessions::search_sessions(root, SessionSearchRequest::default()).unwrap(); assert_eq!(current.total, 1);
        assert_eq!(current.results[0].session.session_id, id(1));
    });
}

#[test]
fn changed_content_rejects_frozen_preview_and_preserves_the_original() {
    fixture(1, |root, claude| {
        let operations = Operations::default();
        let preview = operations.preview(root, request(None, false), Action::Delete).unwrap();
        let path = write(claude, 0, Some("2026-11-05T04:00:00Z"), "other fixture");
        let error = match operations.execute(root, &preview.id, true) { Err(error) => error, Ok(_) => panic!("changed source accepted") };
        assert!(error.contains("变化"), "{error}"); assert!(path.exists()); assert!(!operations.progress().unwrap().running);
        assert!(operations.execute(root, &preview.id, true).is_err());
    });
}

#[test]
fn partial_deletion_reports_failures_and_removes_only_verified_sessions() {
    fixture(1, |root, claude| {
        let codex = PathBuf::from(std::env::var_os("CODEX_HOME").unwrap());
        fs::create_dir_all(codex.join("sessions")).unwrap();
        let path = codex.join("sessions/invalid.jsonl");
        fs::write(&path, format!("{}\n{}\n", json!({"type":"session_meta","payload":{"id":"invalid-uuid"}}),
            json!({"type":"event_msg","timestamp":"2026-11-05T04:00:00Z","payload":{"type":"user_message","message":"keep"}}))).unwrap();
        let operations = Operations::default();
        let preview = operations.preview(root, request(None, false), Action::Delete).unwrap(); assert_eq!(preview.items.len(), 2);
        let result = operations.execute(root, &preview.id, true).unwrap();
        assert_eq!(result.outcomes.iter().filter(|item| item.completed).count(), 1);
        assert_eq!(result.outcomes.iter().filter(|item| !item.completed && item.error.is_some()).count(), 1);
        assert!(path.exists()); assert!(!claude.join("projects/range").join(format!("{}.jsonl", id(0))).exists());
    });
}

#[test]
fn deletion_includes_and_reports_related_claude_sessions_outside_the_dates() {
    fixture(1, |root, claude| {
        let directory = claude.join("projects/range").join(id(0)).join("subagents");
        fs::create_dir_all(&directory).unwrap();
        let child = directory.join("agent-child.jsonl");
        fs::write(&child, format!("{}\n", json!({"type":"user","sessionId":id(0),"uuid":id(2000),
            "timestamp":"2026-11-07T00:00:00Z","message":{"role":"user","content":"outside range child"}}))).unwrap();
        let operations = Operations::default();
        let preview = operations.preview(root, request(Some(AppKind::Claude), false), Action::Delete).unwrap();
        assert_eq!(preview.matched, 1); assert_eq!(preview.items.len(), 2); assert_eq!(preview.items.iter().filter(|item| item.related).count(), 1);
        let result = operations.execute(root, &preview.id, true).unwrap();
        assert_eq!(result.outcomes.len(), 2); assert!(result.outcomes.iter().all(|item| item.completed && item.error.is_none()));
        assert!(!child.exists());
    });
}

#[test]
fn newly_discovered_children_reject_the_original_preview() {
    fixture(1, |root, claude| {
        let operations = Operations::default();
        let preview = operations.preview(root, request(None, false), Action::Delete).unwrap();
        let directory = claude.join("projects/range").join(id(0)).join("subagents"); fs::create_dir_all(&directory).unwrap();
        let child = directory.join("agent-new.jsonl");
        fs::write(&child, format!("{}\n", json!({"type":"user","sessionId":id(0),"uuid":id(3000),
            "timestamp":"2026-11-07T00:00:00Z","message":{"role":"user","content":"new child"}}))).unwrap();
        assert!(operations.execute(root, &preview.id, true).is_err()); assert!(child.exists());
    });
}

#[test]
fn archived_delete_cleans_markers_and_keeps_bookmarked_content() {
    fixture(1, |root, _| {
        let page = sessions::load_page(root, sessions::MessagePageRequest { app: AppKind::Claude, session_id: id(0), offset: 0, anchor_id: None }).unwrap();
        sessions::save_bookmark(root, AppKind::Claude, &id(0), &page.items[0].id).unwrap();
        let operations = Operations::default();
        let preview = operations.preview(root, request(None, false), Action::Archive).unwrap(); operations.execute(root, &preview.id, true).unwrap();
        let preview = operations.preview(root, request(None, true), Action::Delete).unwrap();
        assert!(operations.execute(root, &preview.id, true).unwrap().outcomes[0].completed);
        let mut markers = sessions::fixture_source(Path::new("missing"), AppKind::Claude, &id(0));
        sessions::archive::apply(root, std::slice::from_mut(&mut markers)).unwrap(); assert!(!markers.archived);
        let bookmarks = sessions::search_bookmarks(root, sessions::BookmarkSearchRequest { query: String::new(), app: None, offset: 0 }).unwrap();
        assert_eq!(bookmarks.total, 1);
    });
}

#[test]
fn only_native_spawned_children_expand_the_range_and_independent_forks_stay_outside() {
    fixture(0, |root, _| {
        let codex = PathBuf::from(std::env::var_os("CODEX_HOME").unwrap()).join("sessions");
        fs::create_dir_all(&codex).unwrap();
        let parent = id(100);
        for (index, name) in ["parent", "child", "fork"].iter().enumerate() {
            let source = if *name == "child" { json!({"subagent":{"thread_spawn":{"parent_thread_id":parent}}}) } else { json!("cli") };
            let at = if *name == "parent" { "2026-11-05T04:00:00Z" } else { "2026-11-07T04:00:00Z" };
            fs::write(codex.join(format!("{}.jsonl", id(100 + index))), format!("{}\n{}\n",
                json!({"type":"session_meta","payload":{"id":id(100+index),"source":source,
                    "forked_from_id":if *name=="fork" {Some(&parent)} else {None}}}),
                json!({"type":"event_msg","timestamp":at,"payload":{"type":"user_message","message":name}}))).unwrap();
        }
        for action in [Action::Archive, Action::Delete] {
            let plan = snapshot::prepare(root, request(Some(AppKind::Codex), false), action).unwrap();
            assert_eq!(plan.preview.matched, 1); assert_eq!(plan.preview.items.len(), 2);
            assert!(plan.preview.items.iter().any(|item| item.related && item.session.session_id == id(101)));
            assert!(!plan.preview.items.iter().any(|item| item.session.session_id == id(102)));
        }
    });
}
