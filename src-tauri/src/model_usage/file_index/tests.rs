use super::*;
use serde_json::json;
use std::fs::{self, File};
use std::io::Write;

fn message(id: &str, input: u64) -> String {
    format!("{}\n", json!({"type":"assistant","timestamp":"2026-10-22T02:00:00Z",
        "message":{"id":id,"model":"model","stop_reason":"end_turn",
            "usage":{"input_tokens":input,"output_tokens":2}}}))
}

fn total(value: FileUsage) -> u64 {
    match value.state {
        UsageState::Claude(values) => values.values().map(|candidate| candidate.event.tokens.total).sum(),
        _ => panic!("expected Claude usage"),
    }
}

#[test]
fn a_growing_file_returns_only_its_captured_prefix_then_catches_up_once() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    let index = ReadIndex::open(&temp.path().join("state")).unwrap();
    fs::write(&path, message("first", 10)).unwrap();
    let revision = FileRevision::read(&path).unwrap();
    write!(File::options().append(true).open(&path).unwrap(), "{}", message("second", 20)).unwrap();
    let (snapshot, cacheable) = capture(&path, AppKind::Claude, "session", revision, None).unwrap();
    verify(&path, &snapshot).unwrap();
    assert!(cacheable);
    assert_eq!(total(snapshot), 12);
    assert_eq!(total(read(&index, &path, AppKind::Claude, "session").unwrap()), 34);
    assert_eq!(total(read(&index, &path, AppKind::Claude, "session").unwrap()), 34);
}

#[test]
fn rewriting_a_captured_prefix_is_rejected_and_the_next_read_rebuilds_it() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    let index = ReadIndex::open(&temp.path().join("state")).unwrap();
    fs::write(&path, message("first", 10)).unwrap();
    let previous = read(&index, &path, AppKind::Claude, "session").unwrap();
    let (snapshot, _) = capture(&path, AppKind::Claude, "session", FileRevision::read(&path).unwrap(), Some(previous)).unwrap();
    fs::write(&path, message("first", 90)).unwrap();
    assert!(matches!(verify(&path, &snapshot), Err(prefix::SnapshotError::Changed(_))));
    assert_eq!(total(read(&index, &path, AppKind::Claude, "session").unwrap()), 92);
    fs::write(&path, b"").unwrap();
    assert!(verify(&path, &snapshot).is_err());
}

#[test]
fn reopening_reuses_index_and_append_keeps_each_vendor_message_once() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("state");
    let path = temp.path().join("session.jsonl");
    fs::write(&path, message("first", 10)).unwrap();
    let index = ReadIndex::open(&root).unwrap();
    let first = read(&index, &path, AppKind::Claude, "session").unwrap();
    assert_eq!(first.ordinal, 1);
    assert_eq!(total(first), 12);
    drop(index);
    let index = ReadIndex::open(&root).unwrap();
    let before = records::decoded_record_count();
    assert_eq!(total(read(&index, &path, AppKind::Claude, "session").unwrap()), 12);
    assert_eq!(records::decoded_record_count(), before);
    let mut file = File::options().append(true).open(&path).unwrap();
    file.write_all(message("second", 20).as_bytes()).unwrap();
    let before = records::decoded_record_count();
    let next = read(&index, &path, AppKind::Claude, "session").unwrap();
    assert_eq!(records::decoded_record_count() - before, 1);
    assert_eq!(next.ordinal, 2);
    assert_eq!(total(next), 34);
}

#[test]
fn editing_the_old_prefix_before_append_rebuilds_its_usage() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    let index = ReadIndex::open(&temp.path().join("state")).unwrap();
    fs::write(&path, message("first", 10)).unwrap();
    read(&index, &path, AppKind::Claude, "session").unwrap();
    fs::write(&path, message("first", 90) + &message("second", 20)).unwrap();
    assert_eq!(total(read(&index, &path, AppKind::Claude, "session").unwrap()), 114);
}

#[test]
fn truncation_and_removal_never_return_the_previous_file_usage() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    let index = ReadIndex::open(&temp.path().join("state")).unwrap();
    fs::write(&path, message("first", 10) + &message("second", 20)).unwrap();
    read(&index, &path, AppKind::Claude, "session").unwrap();
    fs::write(&path, message("new", 3)).unwrap();
    assert_eq!(total(read(&index, &path, AppKind::Claude, "session").unwrap()), 5);
    fs::remove_file(&path).unwrap();
    assert!(read(&index, &path, AppKind::Claude, "session").is_err());
}

#[test]
fn completing_an_unfinished_tail_does_not_duplicate_earlier_calls() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    let index = ReadIndex::open(&temp.path().join("state")).unwrap();
    let next = message("second", 20);
    let split = next.len() / 2;
    fs::write(&path, message("first", 10) + &next[..split]).unwrap();
    let partial = read(&index, &path, AppKind::Claude, "session").unwrap();
    assert!(!partial.appendable);
    assert_eq!(partial.issues.len(), 1);
    assert_eq!(total(partial), 12);
    File::options().append(true).open(&path).unwrap().write_all(next[split..].as_bytes()).unwrap();
    let complete = read(&index, &path, AppKind::Claude, "session").unwrap();
    assert!(complete.issues.is_empty());
    assert_eq!(total(complete), 34);
}

#[test]
fn codex_append_preserves_turn_context_and_real_call_identity() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    let index = ReadIndex::open(&temp.path().join("state")).unwrap();
    fs::write(&path, format!("{}\n{}\n", json!({"type":"session_meta","payload":{"id":"session"}}),
        json!({"type":"turn_context","payload":{"turn_id":"turn","model":"current-model"}}))).unwrap();
    read(&index, &path, AppKind::Codex, "session").unwrap();
    let receipt = json!({"type":"token_usage_record","timestamp":"2026-10-22T02:00:00Z","payload":{
        "thread_id":"session","session_id":"run","response_id":"response","turn_id":"turn","root_turn_id":"turn",
        "usage":{"input_tokens":10,"cached_input_tokens":2,"cache_write_input_tokens":0,
            "output_tokens":3,"reasoning_output_tokens":1,"total_tokens":13}}});
    writeln!(File::options().append(true).open(&path).unwrap(), "{receipt}").unwrap();
    let value = read(&index, &path, AppKind::Codex, "session").unwrap();
    let UsageState::Codex(state) = value.state else { panic!("expected Codex usage") };
    let events: Vec<_> = state.events().collect();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].model.as_deref(), Some("current-model"));
    assert_eq!(events[0].tokens.total(), Some(13));
}
