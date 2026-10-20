use super::*;
use asb_core::contracts::AppKind;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn search(sources: Vec<SessionSource>, issues: Vec<SessionIssue>, query: &str, app: Option<AppKind>, offset: usize) -> SessionSearchPage {
    let filters = Filters::parse(SessionSearchRequest { query: query.into(), app, offset, ..Default::default() }).unwrap();
    super::search_sources(sources, issues, &filters)
}

fn source(path: &Path, id: &str) -> SessionSource {
    SessionSource { paths: vec![path.to_path_buf()], native_history: None, converted: None, meta: SessionMeta {
        app: AppKind::Codex, session_id: id.into(), title: "metadata title".into(),
        summary: "summary".into(), project_dir: Some("/projects/example".into()),
        created_at: None, last_active_at: Some("2026-09-22T00:00:00Z".into()),
        resume_command: String::new(),
        alias: None, pinned: false, tags: Vec::new(), parent_session_id: None, can_delete: true,
    } }
}

fn message(text: &str) -> String {
    format!("{}\n", serde_json::json!({"role":"assistant", "content":text}))
}

#[test]
fn searches_complete_transcripts_and_locates_unicode_cross_line_hits() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("source.jsonl");
    let text = format!("{}\nÄPFEL   连接\n  Error42 tail", "before ".repeat(90));
    fs::write(&path, message(&text) + &message("second Äpfel 连接 Error42")).unwrap();
    let page = search(vec![source(&path, "s")], vec![], "äpfel 连接 error42", None, 0);
    assert_eq!(page.total, 2);
    assert!(page.issues.is_empty());
    assert!(page.results[0].excerpt.contains("ÄPFEL 连接 Error42"));
    assert!(page.results[0].excerpt.starts_with('…'));
    let messages = collect_messages(&path).unwrap();
    assert_eq!(page.results[0].message_id.as_deref(), Some(messages[0].id.as_str()));
    assert_ne!(messages[0].id, messages[1].id);
}

#[test]
fn deep_unicode_match_opens_its_original_content_page() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("source.jsonl");
    let text = format!("  {}\n\t  İ目标  跨行\n命中", "🙂 \t".repeat(20_000));
    fs::write(&path, message(&text)).unwrap();
    let page = search(vec![source(&path, "s")], vec![], "İ目标 跨行 命中", None, 0);
    assert!(page.issues.is_empty());
    let offset = page.results[0].content_offset;
    assert!(offset > 0);
    let part = super::super::messages::content_page(&text, offset).unwrap();
    assert!(part.content.contains("İ目标  跨行\n命中"));
}

#[test]
fn metadata_matches_only_without_message_hits_and_failures_are_visible() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("source.jsonl");
    fs::write(&path, message("metadata title in message")).unwrap();
    let page = search(vec![source(&path, "s")], vec![], "metadata", None, 0);
    assert_eq!(page.total, 1);
    assert!(page.results[0].message_id.is_some());
    let page = search(vec![source(&path, "s")], vec![], "projects/example", None, 0);
    assert_eq!(page.total, 1);
    assert!(page.results[0].message_id.is_none());
    fs::write(&path, message("metadata") + "{invalid\n").unwrap();
    let page = search(vec![source(&path, "s")], vec![], "metadata", None, 0);
    assert_eq!(page.total, 1);
    assert!(page.results[0].message_id.is_some());
    assert_eq!(page.issues.len(), 1);
    assert!(page.issues[0].message.contains("第 2 条记录"));
}

#[test]
fn pagination_ties_and_app_filter_are_deterministic() {
    let temp = tempdir().unwrap();
    let sources = (0..55).rev().map(|index| source(temp.path(), &format!("s{index:02}")))
        .collect::<Vec<_>>();
    let page = search(sources.clone(), vec![], "", None, 0);
    assert_eq!(page.total, 55);
    assert_eq!(page.results.len(), 50);
    assert_eq!(page.results[0].session.session_id, "s00");
    let page = search(sources.clone(), vec![], "", None, 50);
    assert_eq!(page.results.len(), 5);
    assert_eq!(page.results[0].session.session_id, "s50");
    assert_eq!(search(sources, vec![], "", Some(AppKind::Claude), 0).total, 0);
}

#[test]
fn message_ids_survive_append_but_reject_changed_prefix() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("source.jsonl");
    let first = message("same text");
    fs::write(&path, &first).unwrap();
    let old = collect_messages(&path).unwrap()[0].id.clone();
    fs::write(&path, first.clone() + &first).unwrap();
    let appended = collect_messages(&path).unwrap();
    assert_eq!(old, appended[0].id);
    assert_ne!(old, appended[1].id);
    fs::write(&path, message("edited") + &first).unwrap();
    assert!(collect_messages(&path).unwrap().iter().all(|message| message.id != old));
}

#[test]
fn message_pagination_counts_unretained_hits_without_metadata_duplicates() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("source.jsonl");
    let transcript = (0..123).map(|index| message(&format!("metadata match {index}")))
        .collect::<String>();
    fs::write(&path, transcript).unwrap();
    let page = search(vec![source(&path, "s")], vec![], "metadata", None, 50);
    assert_eq!(page.total, 123);
    assert_eq!(page.results.len(), PAGE_SIZE);
    assert_eq!(page.results[0].excerpt, "metadata match 50");
    assert_eq!(page.results[49].excerpt, "metadata match 99");
    let past = search(vec![source(&path, "s")], vec![], "metadata", None, 123);
    assert_eq!(past.total, 123);
    assert!(past.results.is_empty());
}

#[test]
fn searches_codex_tool_outputs_once_without_event_mirror_duplicates() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("source.jsonl");
    let records = [
        serde_json::json!({"type":"response_item", "payload": {
            "type":"function_call_output", "output":"Error: database is locked"}}),
        serde_json::json!({"type":"response_item", "payload": {
            "type":"custom_tool_call_output", "output":"Error: patch rejected"}}),
        serde_json::json!({"type":"event_msg", "payload": {
            "type":"tool_output", "output":"Error: database is locked"}}),
    ];
    fs::write(&path, records.iter().map(|record| format!("{record}\n")).collect::<String>()).unwrap();
    let messages = collect_messages(&path).unwrap();
    assert_eq!(messages.len(), 2);
    assert!(messages.iter().all(|message| message.role == "tool"));
    let page = search(vec![source(&path, "s")], vec![], "error:", None, 0);
    assert_eq!(page.total, 2);
    assert_eq!(page.results[0].message_id.as_deref(), Some(messages[0].id.as_str()));
}

#[test]
fn organization_filters_run_before_pagination_and_pins_sort_first() {
    let temp = tempdir().unwrap();
    let mut sources = (0..65).map(|index| {
        let mut source = source(temp.path(), &format!("s{index:02}"));
        source.meta.pinned = index == 64;
        source.meta.tags = vec![if index >= 10 { "keep".into() } else { "other".into() }];
        source
    }).collect::<Vec<_>>();
    sources[64].meta.last_active_at = Some("2026-09-21T00:00:00Z".into());
    let filters = Filters::parse(SessionSearchRequest { tag: Some("keep".into()), ..Default::default() }).unwrap();
    let page = super::search_sources(sources, vec![], &filters);
    assert_eq!(page.total, 55);
    assert_eq!(page.results[0].session.session_id, "s64");
    assert_eq!(page.results.len(), 50);
    assert_eq!(page.projects[0].count, 65);
    assert_eq!(page.tags, ["keep", "other"]);
}

#[test]
fn exact_project_and_time_filters_distinguish_clients_and_ungrouped_sessions() {
    let temp = tempdir().unwrap();
    let mut codex = source(temp.path(), "codex");
    codex.meta.project_dir = None;
    let mut claude = source(temp.path(), "claude");
    claude.meta.app = AppKind::Claude;
    claude.meta.project_dir = None;
    let mut earlier = codex.clone();
    earlier.meta.session_id = "earlier".into();
    earlier.meta.last_active_at = Some("2026-09-21T23:59:59Z".into());
    let filters = Filters::parse(SessionSearchRequest {
        project: Some(filters::SessionProjectFilter { app: AppKind::Codex, project_dir: None }),
        active_after: Some("2026-09-22T00:00:00Z".into()),
        active_before: Some("2026-09-23T00:00:00Z".into()), ..Default::default()
    }).unwrap();
    let page = super::search_sources(vec![codex, claude, earlier], vec![], &filters);
    assert_eq!(page.total, 1);
    assert_eq!(page.results[0].session.session_id, "codex");
    assert_eq!(page.projects.len(), 2);
}

#[test]
fn local_alias_and_tags_are_searchable_without_overwriting_source_title() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("source.jsonl");
    fs::write(&path, message("unrelated body")).unwrap();
    let mut source = source(&path, "s1");
    source.meta.alias = Some("Local Release".into());
    source.meta.tags = vec!["Ticket-42".into()];
    for query in ["local release", "ticket-42"] {
        let page = search(vec![source.clone()], vec![], query, None, 0);
        assert_eq!(page.total, 1);
        assert_eq!(page.results[0].session.title, "metadata title");
    }
    assert!(Filters::parse(SessionSearchRequest { active_after: Some("yesterday".into()), ..Default::default() }).is_err());
}
