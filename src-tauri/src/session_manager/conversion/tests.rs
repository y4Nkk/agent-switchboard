use super::*;
use serde_json::{json, Value};
use std::io::Write;

fn fixture(root: &Path, name: &str, values: &[Value]) -> PathBuf {
    let project = root.join("projects/encoded-project");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("parent-session.jsonl"), format!("{}\n", json!({
        "type":"user", "sessionId":"parent-session", "cwd":"C:/actual-project",
        "message":{"role":"user","content":"main request"}
    }))).unwrap();
    let path = project.join("parent-session/subagents/workflows").join(name).join("journal.jsonl");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    write(&path, values);
    path
}

fn write(path: &Path, values: &[Value]) {
    fs::write(path, values.iter().map(|value| format!("{value}\n")).collect::<String>()).unwrap();
}

fn run(root: &Path, ids: &[String]) -> SessionConversionOutcome {
    convert_from_roots(&root.join("state"), ids, &[(AppKind::Claude, root.join("projects"))]).unwrap()
}

fn converted(root: &Path, path: &Path) -> (SessionSource, Vec<SessionIssue>) {
    let mut sources = Vec::new();
    let mut issues = vec![pending_issue(path).unwrap()];
    include(&root.join("state"), &mut sources, &mut issues);
    (sources.into_iter().next().unwrap(), issues)
}

fn messages(source: &SessionSource) -> Vec<SessionMessage> {
    let mut result = Vec::new();
    assert!(source.visit(|message| { result.push(message); Ok(ControlFlow::Continue(())) }).unwrap().is_empty());
    result
}

#[test]
fn source_identity_is_shared_by_path_components_discovery_and_saved_copies() {
    let temp = tempfile::tempdir().unwrap();
    let path = fixture(temp.path(), "wf_example", &[json!({"type":"result","text":"retained"})]);
    let root = temp.path().join("projects");
    let discovered = sources::collect(&root).paths.into_iter().find(|path| is_workflow(path)).unwrap();
    let equivalent = root.join("encoded-project/./parent-session//subagents/workflows/wf_example/journal.jsonl");
    assert_eq!(source_key(&path), source_key(&discovered));
    assert_eq!(source_key(&equivalent), source_key(&discovered));
    assert_eq!(candidate(&path).unwrap().id(), candidate(&discovered).unwrap().id());
    let outcome = run(temp.path(), &[candidate(&equivalent).unwrap().id()]);
    assert_eq!((outcome.converted, outcome.records, outcome.failures.len()), (1, 1, 0));
    fs::remove_file(&path).unwrap();
    let (copies, failures) = store::list(&temp.path().join("state"));
    assert_eq!(copies.len(), 1);
    assert!(failures.is_empty());
    assert_eq!(copies[0].0.source_key, source_key(&equivalent));
}

#[test]
fn preserves_every_event_field_occurrence_and_time_without_rewriting_the_original() {
    let temp = tempfile::tempdir().unwrap();
    let values = vec![json!({"type":"launched"}), json!({"type":"result", "key":"result-1",
        "timestamp":"2026-10-20T01:02:03Z", "result":{"unknown":[1, {"text":"完整正文```"}], "cost":null}}),
        json!({"type":"launched"})];
    let path = fixture(temp.path(), "wf_example", &values);
    let original = fs::read(&path).unwrap();
    let id = candidate(&path).unwrap().id();
    let outcome = run(temp.path(), &[id.clone(), id]);
    assert_eq!((outcome.converted, outcome.records), (1, 3));
    assert!(outcome.failures.is_empty());
    assert_eq!(fs::read(&path).unwrap(), original);
    let (source, issues) = converted(temp.path(), &path);
    assert!(issues.is_empty());
    assert_eq!(source.meta.parent_session_id.as_deref(), Some("parent-session"));
    assert_eq!(source.meta.project_dir.as_deref(), Some("C:/actual-project"));
    assert_eq!(source.meta.resume_command, "claude --resume parent-session");
    let messages = messages(&source);
    assert_eq!(messages.len(), values.len());
    for (message, value) in messages.iter().zip(&values) { assert_eq!(message.content, content::json(value)); }
    assert_ne!(messages[0].id, messages[2].id);
    assert!(messages[0].at.is_none());
    assert_eq!(messages[1].at.as_deref(), Some("2026-10-20T01:02:03+00:00"));
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(source.find(&messages[1].id).unwrap().content, messages[1].content);
}

#[test]
fn refresh_replaces_one_record_and_keeps_existing_message_ids_stable() {
    let temp = tempfile::tempdir().unwrap();
    let path = fixture(temp.path(), "wf_example", &[json!({"type":"launched"})]);
    run(temp.path(), &[candidate(&path).unwrap().id()]);
    let (first, _) = converted(temp.path(), &path);
    let first_id = messages(&first)[0].id.clone();
    let id = candidate(&path).unwrap().id();
    assert_eq!(run(temp.path(), &[id]).converted, 1);
    File::options().append(true).open(&path).unwrap().write_all(b"{\"type\":\"result\",\"value\":\"new\"}\n").unwrap();
    let (_, issues) = converted(temp.path(), &path);
    assert_eq!(issues.len(), 1);
    let outcome = run(temp.path(), &[issues[0].conversion_id.clone().unwrap()]);
    assert_eq!((outcome.converted, outcome.records), (1, 2));
    let (next, issues) = converted(temp.path(), &path);
    assert!(issues.is_empty());
    assert_eq!(first.meta.session_id, next.meta.session_id);
    assert_eq!(messages(&next)[0].id, first_id);
    assert_eq!(store::list(&temp.path().join("state")).0.len(), 1);
}

#[test]
fn stale_requests_and_broken_records_never_replace_the_previous_complete_copy() {
    let temp = tempfile::tempdir().unwrap();
    let path = fixture(temp.path(), "wf_example", &[json!({"type":"launched"})]);
    let stale = candidate(&path).unwrap().id();
    run(temp.path(), std::slice::from_ref(&stale));
    let (source, _) = converted(temp.path(), &path);
    let target = source.converted.as_ref().unwrap();
    let previous = fs::read(target).unwrap();
    File::options().append(true).open(&path).unwrap().write_all(b"{\"type\":\"result\"}\n").unwrap();
    let rejected = run(temp.path(), &[stale]);
    assert_eq!(rejected.converted, 0);
    assert_eq!(rejected.failures.len(), 1);
    assert!(rejected.failures[0].message.contains("来源已经变化"));
    let during = candidate(&path).unwrap();
    let (meta, count) = metadata(&during).unwrap();
    File::options().append(true).open(&path).unwrap().write_all(b"{\"partial\":").unwrap();
    assert!(store::write(&temp.path().join("state"), &during, meta, count).is_err());
    let broken = run(temp.path(), &[candidate(&path).unwrap().id()]);
    assert_eq!(broken.converted, 0);
    assert_eq!(broken.failures.len(), 1);
    assert_eq!(fs::read(target).unwrap(), previous);
}

#[test]
fn batch_results_are_per_requested_source_and_never_accept_renderer_paths() {
    let temp = tempfile::tempdir().unwrap();
    let good = fixture(temp.path(), "wf_good", &[json!({"type":"result","text":"valid"})]);
    let bad = fixture(temp.path(), "wf_bad", &[json!({"type":"started"})]);
    File::options().append(true).open(&bad).unwrap().write_all(b"invalid\n").unwrap();
    let outcome = run(temp.path(), &[candidate(&good).unwrap().id(), candidate(&bad).unwrap().id(), "C:/unapproved/journal.jsonl".into()]);
    assert_eq!((outcome.converted, outcome.records, outcome.failures.len()), (1, 1, 2));
    assert_eq!(store::list(&temp.path().join("state")).0.len(), 1);
    assert!(is_workflow(&good));
    assert!(!is_workflow(&temp.path().join("projects/project/journal.jsonl")));
}

#[test]
fn converted_material_is_excluded_from_usage_and_deletion_only_removes_the_copy() {
    let temp = tempfile::tempdir().unwrap();
    let path = fixture(temp.path(), "wf_example", &[json!({"type":"result", "usage":{
        "input_tokens":100, "output_tokens":200}, "model":"not-a-call", "messageId":"not-a-call"})]);
    run(temp.path(), &[candidate(&path).unwrap().id()]);
    let (source, _) = converted(temp.path(), &path);
    let pending = pending_issue(&path).unwrap();
    let report = crate::model_usage::report(asb_core::contracts::ModelUsageRange::All,
        vec![source.clone()], vec![pending], chrono::Local::now());
    assert_eq!(report.coverage.source_sessions, 0);
    assert_eq!(report.coverage.scanned_files, 0);
    assert!(report.groups.is_empty());
    assert!(report.issues.is_empty());
    super::super::remove_source(&source).unwrap();
    assert!(path.is_file());
    assert!(!source.converted.unwrap().exists());
}

#[test]
fn corrupt_converted_material_is_preserved_and_can_be_converted_again() {
    let temp = tempfile::tempdir().unwrap();
    let path = fixture(temp.path(), "wf_example", &[json!({"type":"launched"})]);
    run(temp.path(), &[candidate(&path).unwrap().id()]);
    let (source, _) = converted(temp.path(), &path);
    let target = source.converted.unwrap();
    File::options().append(true).open(&target).unwrap().write_all(b"invalid\n").unwrap();
    let before = fs::read(&target).unwrap();
    let (copies, failures) = store::list(&temp.path().join("state"));
    assert!(copies.is_empty());
    assert_eq!(failures.len(), 1);
    assert_eq!(fs::read(&target).unwrap(), before);
    assert_eq!(run(temp.path(), &[candidate(&path).unwrap().id()]).converted, 1);
    assert_eq!(store::list(&temp.path().join("state")).0.len(), 1);
}
