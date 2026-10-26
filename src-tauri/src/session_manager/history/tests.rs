use super::*;
use crate::session_manager::{SessionMessage, records};
use base64::Engine;
use serde_json::{json, Value};
use std::fs;
use std::ops::ControlFlow;

const ID: &str = "c1a11111-2222-4333-8444-555555555555";
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aGN8AAAAASUVORK5CYII=";

#[test]
fn continuous_append_preserves_a_verified_prefix_and_catches_up_without_warnings() {
    use std::io::Write;
    let mut fixture = Fixture::new();
    fixture.write(&[user("active session"), assistant("receipt", 3, 5)]);
    let path = fixture.path();
    let checkpoint = || {
        writeln!(fs::OpenOptions::new().append(true).open(&path).unwrap(),
            "{}", json!({"type":"progress","sessionId":ID,"data":{"type":"test"}})).unwrap();
        Ok(())
    };
    let issues = capture::synchronize_observed(&mut fixture.connection, &fixture.root, &[], &checkpoint).unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    let sources = reading::sources(&fixture.connection, &store::path(&fixture.root)).unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].retained.len(), 1);
    let report = crate::model_usage::report(asb_core::contracts::ModelUsageRange::All, sources, issues, chrono::Local::now());
    assert_eq!(report.groups[0].total_tokens, 8);
    let (sources, issues) = fixture.sync();
    assert!(issues.is_empty(), "{issues:?}");
    let report = crate::model_usage::report(asb_core::contracts::ModelUsageRange::All, sources, issues, chrono::Local::now());
    assert_eq!(report.groups[0].total_tokens, 8);
    let length: u64 = fixture.connection.query_row("SELECT byte_length FROM transcripts", [], |row| row.get(0)).unwrap();
    assert_eq!(length, fs::metadata(path).unwrap().len());
}

struct Fixture {
    _temporary: tempfile::TempDir,
    root: PathBuf,
    client: PathBuf,
    connection: Connection,
}

impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("app-state");
        let client = temporary.path().join("Claude account A");
        fs::create_dir_all(client.join("projects/project")).unwrap();
        let connection = store::open(&root).unwrap();
        capture::remember_root(&connection, &client, &root, false).unwrap();
        Self { _temporary: temporary, root, client, connection }
    }

    fn path(&self) -> PathBuf { self.client.join("projects/project").join(format!("{ID}.jsonl")) }

    fn write(&self, records: &[Value]) {
        fs::write(self.path(), lines(records)).unwrap();
    }

    fn sync(&mut self) -> (Vec<SessionSource>, Vec<SessionIssue>) {
        let issues = capture::synchronize(&mut self.connection, &self.root, &[]).unwrap();
        (reading::sources(&self.connection, &store::path(&self.root)).unwrap(), issues)
    }
}

fn lines(values: &[Value]) -> String { values.iter().map(|value| format!("{value}\n")).collect() }

fn user(text: &str) -> Value {
    json!({"type":"user","sessionId":ID,"uuid":"11111111-2222-4333-8444-000000000001","timestamp":"2026-10-22T01:00:00Z",
        "message":{"role":"user","content":text}})
}

fn assistant(id: &str, input: u64, output: u64) -> Value {
    json!({"type":"assistant","sessionId":ID,"timestamp":"2026-10-22T01:01:00Z",
        "message":{"id":id,"role":"assistant","model":"test-model","stop_reason":"end_turn",
        "content":[{"type":"text","text":id}],"usage":{"input_tokens":input,"output_tokens":output}}})
}

fn messages(source: &SessionSource) -> Vec<SessionMessage> {
    let mut messages = Vec::new();
    source.visit(|message| { messages.push(message); Ok(ControlFlow::Continue(())) }).unwrap();
    messages
}

fn appended_fixture() -> Fixture {
    let mut fixture = Fixture::new();
    fixture.write(&[user("before")]);
    assert!(fixture.sync().1.is_empty());
    fixture.write(&[user("before"), assistant("new-complete-message", 1, 2)]);
    fixture
}

#[test]
fn cancellation_at_each_sync_checkpoint_preserves_complete_records_and_availability() {
    let mut probe = appended_fixture();
    let checkpoints = std::cell::Cell::new(0usize);
    capture::synchronize_observed(&mut probe.connection, &probe.root, &[], &|| {
        checkpoints.set(checkpoints.get() + 1); Ok(())
    }).unwrap();
    for stop in 1..=checkpoints.get() {
        let mut fixture = appended_fixture();
        let visited = std::cell::Cell::new(0usize);
        let result = capture::synchronize_observed(&mut fixture.connection, &fixture.root, &[], &|| {
            visited.set(visited.get() + 1);
            if visited.get() >= stop { Err("cancelled".into()) } else { Ok(()) }
        });
        assert!(result.is_err(), "checkpoint {stop} must cancel");
        let saved = reading::sources(&fixture.connection, &store::path(&fixture.root)).unwrap();
        assert_eq!(saved.len(), 1);
        assert!(saved[0].meta.preservation.as_ref().unwrap().source_available);
        let retained = messages(&saved[0]);
        assert!(retained.len() == 1 || retained.len() == 2);
        assert_eq!(retained[0].content, "before");
        if retained.len() == 2 { assert!(retained[1].content.contains("new-complete-message")); }
        let issues: i64 = fixture.connection.query_row("SELECT count(*) FROM transcripts WHERE capture_issue IS NOT NULL", [], |row| row.get(0)).unwrap();
        assert_eq!(issues, 0, "cancellation is not a preservation failure");
    }
}

#[test]
fn originals_can_disappear_without_losing_text_images_or_message_identity() {
    let mut fixture = Fixture::new();
    let image = fixture.client.join("picture.png");
    fs::write(&image, base64::engine::general_purpose::STANDARD.decode(PNG).unwrap()).unwrap();
    fixture.write(&[json!({"type":"user","sessionId":ID,"message":{"role":"user",
        "content":[{"type":"text","text":"keep the whole record"},{"type":"image","path":image}]}})]);
    let (before, issues) = fixture.sync();
    assert!(issues.is_empty());
    let initial = messages(&before[0]);
    fs::remove_dir_all(&fixture.client).unwrap();
    let (after, issues) = fixture.sync();
    assert!(issues.is_empty());
    assert_eq!(after.len(), 1);
    assert!(!after[0].meta.preservation.as_ref().unwrap().source_available);
    let kept = messages(&after[0]);
    assert_eq!(kept[0].id, initial[0].id);
    assert!(kept[0].content.contains("keep the whole record"));
    let (images, issues) = after[0].resolve_images(&kept[0].images);
    assert!(issues.is_empty());
    assert_eq!(images, vec![format!("data:image/png;base64,{PNG}")]);
}

#[test]
fn incomplete_appends_keep_the_previous_complete_snapshot_then_recover() {
    let mut fixture = Fixture::new();
    fixture.write(&[user("first")]);
    let (before, _) = fixture.sync();
    let initial = messages(&before[0]);
    fs::write(fixture.path(), format!("{}{{\"type\":", lines(&[user("first")]))).unwrap();
    let (failed, issues) = fixture.sync();
    assert!(!issues.is_empty());
    assert_eq!(messages(&failed[0]).len(), 1);
    assert!(failed[0].meta.preservation.as_ref().unwrap().needs_attention);
    assert_eq!(failed[0].retained[0].revision, before[0].retained[0].revision);
    fixture.write(&[user("first"),assistant("second", 3, 5)]);
    let (recovered, issues) = fixture.sync();
    assert!(issues.is_empty());
    assert_eq!(messages(&recovered[0])[0].id, initial[0].id);
    assert_eq!(messages(&recovered[0]).len(), 2);
    assert!(!recovered[0].meta.preservation.as_ref().unwrap().needs_attention);
}

#[test]
fn changing_a_referenced_file_keeps_the_previously_saved_attachment() {
    let mut fixture = Fixture::new();
    let image = fixture.client.join("picture.png");
    let mut bytes = base64::engine::general_purpose::STANDARD.decode(PNG).unwrap();
    fs::write(&image, &bytes).unwrap();
    fixture.write(&[json!({"type":"user","sessionId":ID,"uuid":"11111111-2222-4333-8444-000000000001",
        "message":{"role":"user","content":[{"type":"image","path":image}]}})]);
    fixture.sync();
    bytes.push(0);
    fs::write(&image, &bytes).unwrap();
    fixture.write(&[
        json!({"type":"user","sessionId":ID,"uuid":"11111111-2222-4333-8444-000000000001",
            "message":{"role":"user","content":[{"type":"image","path":image}]}}),
        json!({"type":"user","sessionId":ID,"uuid":"11111111-2222-4333-8444-000000000002",
            "message":{"role":"user","content":[{"type":"image","path":image}]}}),
    ]);
    let (saved, issues) = fixture.sync();
    assert!(issues.is_empty());
    assert_eq!(saved[0].retained.len(), 2);
    assert_eq!(fixture.connection.query_row("SELECT count(*) FROM assets", [], |row| row.get::<_, u64>(0)).unwrap(), 2);
    let (images, issues) = saved[0].resolve_images(&messages(&saved[0])[0].images);
    assert!(issues.is_empty());
    assert_eq!(images, vec![format!("data:image/png;base64,{PNG}")]);
    let (images, issues) = saved[0].resolve_images(&messages(&saved[0])[1].images);
    assert!(issues.is_empty());
    assert_eq!(images, vec![format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))]);
}

#[test]
fn appending_after_a_complete_unterminated_record_keeps_contiguous_bytes() {
    let mut fixture = Fixture::new();
    fs::write(fixture.path(), user("first").to_string()).unwrap();
    fixture.sync();
    fixture.write(&[user("first"), assistant("new answer", 4, 9)]);
    let (sources, issues) = fixture.sync();
    assert!(issues.is_empty());
    assert_eq!(sources[0].retained.len(), 1);
    let mut end = 0;
    visit_record_bytes(&sources[0].retained[0], 0, |_, position, raw| {
        assert_eq!(position, end);
        end += raw.len() as u64;
        Ok(ControlFlow::Continue(()))
    }).unwrap();
    assert_eq!(end, fs::metadata(fixture.path()).unwrap().len());
    let report = crate::model_usage::report(asb_core::contracts::ModelUsageRange::All, sources, issues, chrono::Local::now());
    assert_eq!(report.groups[0].total_tokens, 13);
}

#[test]
fn account_directories_share_one_logical_session_and_count_receipts_once() {
    let mut fixture = Fixture::new();
    fixture.write(&[user("first"),assistant("call-1", 3, 5)]);
    let other = fixture._temporary.path().join("Claude account B");
    fs::create_dir_all(other.join("projects/project")).unwrap();
    fs::write(other.join("projects/project").join(format!("{ID}.jsonl")),
        lines(&[user("first"),assistant("call-1", 3, 5),assistant("call-2", 7, 11)])).unwrap();
    capture::remember_root(&fixture.connection, &other, &fixture.root, false).unwrap();
    let (sources, issues) = fixture.sync();
    assert!(issues.is_empty());
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].retained.len(), 2);
    assert_eq!(messages(&sources[0]).len(), 3);
    let report = crate::model_usage::report(asb_core::contracts::ModelUsageRange::All, sources, issues, chrono::Local::now());
    assert_eq!(report.groups.len(), 1);
    assert_eq!(report.groups[0].total_tokens, 26);
}

#[test]
fn rewriting_a_native_log_never_discards_previously_preserved_messages() {
    let mut fixture = Fixture::new();
    fixture.write(&[user("early prompt"), assistant("early answer", 1, 2)]);
    fixture.sync();
    fixture.write(&[assistant("compacted checkpoint", 2, 3)]);
    let (saved, issues) = fixture.sync();
    assert!(issues.is_empty());
    assert_eq!(saved[0].retained.len(), 2);
    let messages = messages(&saved[0]);
    assert!(messages.iter().any(|message| message.content == "early prompt"));
    assert!(messages.iter().any(|message| message.content == "early answer"));
    assert!(messages.iter().any(|message| message.content == "compacted checkpoint"));
}

#[test]
fn deletion_prevents_an_old_offline_copy_from_being_imported_again() {
    let mut fixture = Fixture::new();
    fixture.write(&[user("delete once")]);
    fixture.sync();
    super::delete(&fixture.root, &[ID.into()]).unwrap();
    fixture.write(&[user("delete once")]);
    let (sources, issues) = fixture.sync();
    assert!(issues.is_empty());
    assert!(sources.is_empty());
    assert_eq!(fixture.connection.query_row("SELECT count(*) FROM deleted_sessions WHERE id=?1", [ID], |row| row.get::<_, u64>(0)).unwrap(), 1);
}

#[test]
fn resume_reconstructs_raw_records_subagents_and_tool_results_without_credentials() {
    let mut fixture = Fixture::new();
    fixture.write(&[user("complete context")]);
    let sidecar = fixture.path().with_extension("");
    fs::create_dir_all(sidecar.join("subagents")).unwrap();
    fs::create_dir_all(sidecar.join("tool-results")).unwrap();
    fs::write(sidecar.join("subagents/agent-child.jsonl"), lines(&[assistant("child-answer", 0, 1)])).unwrap();
    fs::write(sidecar.join("tool-results/output.txt"), "the complete tool output").unwrap();
    fs::write(fixture.client.join(".credentials.json"), "must never be copied").unwrap();
    let (sources, issues) = fixture.sync();
    assert!(issues.is_empty());
    let main = sources.iter().find(|source| source.meta.session_id == ID).unwrap();
    let output = super::prepare_resume(&fixture.root, main).unwrap();
    let directory = output.parent().unwrap();
    assert_eq!(fs::read_to_string(directory.join(format!("{ID}/tool-results/output.txt"))).unwrap(), "the complete tool output");
    assert!(directory.join(format!("{ID}/subagents/agent-child.jsonl")).is_file());
    assert!(!directory.join(".credentials.json").exists());
    let mut raw = Vec::new();
    records::visit_records(&output, 0, |_, _, value| { raw.push(value); Ok(ControlFlow::Continue(())) }).unwrap();
    assert_eq!(raw, vec![user("complete context")]);
}

#[test]
fn resumed_appends_are_preserved_and_deleting_cleans_all_resume_files() {
    let mut fixture = Fixture::new();
    fixture.write(&[user("preserved prompt")]);
    let (sources, _) = fixture.sync();
    let output = super::prepare_resume(&fixture.root, &sources[0]).unwrap();
    let package = output.parent().unwrap().parent().unwrap().parent().unwrap().to_path_buf();
    fs::write(&output, lines(&[user("preserved prompt"), assistant("continued answer", 4, 9)])).unwrap();
    let (sources, issues) = fixture.sync();
    assert!(issues.is_empty());
    assert_eq!(messages(&sources[0]).len(), 2);
    fs::remove_file(&output).unwrap();
    let (saved, _) = fixture.sync();
    assert!(messages(&saved[0]).iter().any(|message| message.content == "continued answer"));
    super::delete(&fixture.root, &[ID.into()]).unwrap();
    assert!(!package.exists());
    assert!(fixture.sync().0.is_empty());
}

#[test]
fn unknown_or_corrupt_databases_are_preserved_in_place() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("state");
    fs::create_dir_all(super::super::layout::directory(&root)).unwrap();
    let path = store::path(&root);
    fs::write(&path, b"unreadable database bytes").unwrap();
    assert!(store::open(&root).is_err());
    assert_eq!(fs::read(path).unwrap(), b"unreadable database bytes");
}
