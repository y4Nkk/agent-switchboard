use super::*;
use crate::session_manager::{fixture_source, SessionSource};
use chrono::TimeZone;
use serde_json::{json, Value};
use std::path::Path;

fn now() -> DateTime<Local> { Local.with_ymd_and_hms(2026, 10, 16, 12, 0, 0).single().unwrap() }
fn at(day: u32) -> String { Local.with_ymd_and_hms(2026, 10, day, 12, 0, 0).single().unwrap().to_rfc3339() }
fn source(root: &Path, name: &str, id: &str, app: AppKind, values: &[Value]) -> SessionSource {
    let path = root.join(name);
    std::fs::write(&path, values.iter().map(|value| format!("{value}\n")).collect::<String>()).unwrap();
    fixture_source(&path, app, id)
}
fn counts(input: u64) -> Value {
    json!({"input_tokens":input,"cached_input_tokens":2,"cache_write_input_tokens":1,
        "output_tokens":3,"reasoning_output_tokens":1,"total_tokens":input+3})
}
fn codex(id: &str, day: u32, input: u64) -> Vec<Value> {
    vec![json!({"type":"session_meta","payload":{"id":id}}),
        json!({"type":"turn_context","payload":{"turn_id":format!("{id}-turn"),"model":"model-codex"}}),
        json!({"type":"token_usage_record","timestamp":at(day),"payload":{
            "thread_id":id,"turn_id":format!("{id}-turn"),"root_turn_id":format!("{id}-root"),
            "session_id":format!("{id}-runtime"),"response_id":format!("{id}-response-{day}-{input}"),
            "usage":counts(input),"turn_token_usage":counts(input),"thread_token_usage":counts(input)}})]
}
fn claude(id: &str, day: u32, output: u64, stopped: bool) -> Value {
    json!({"type":"assistant","timestamp":at(day),"message":{"id":id,"model":"model-claude",
        "stop_reason":if stopped { Some("end_turn") } else { None },
        "usage":{"input_tokens":5,"output_tokens":output,"cache_read_input_tokens":7,"cache_creation_input_tokens":11}}})
}
fn total(report: &ModelUsageReport) -> u64 { report.groups.iter().map(|group| group.total_tokens).sum() }

#[test]
fn ranges_follow_actual_call_days_and_keep_cache_components_exact() {
    let temp = tempfile::tempdir().unwrap();
    let old = source(temp.path(), "old.jsonl", "old", AppKind::Codex, &codex("old", 1, 10));
    let today = source(temp.path(), "today.jsonl", "today", AppKind::Codex, &codex("today", 16, 20));
    let sources = vec![old, today];
    let daily = report(ModelUsageRange::Today, sources.clone(), vec![], now());
    assert_eq!(total(&daily), 23);
    assert_eq!(daily.groups[0].input_tokens, 17);
    assert_eq!(daily.groups[0].cache_read_input_tokens, 2);
    assert_eq!(daily.groups[0].cache_creation_input_tokens, 1);
    assert_eq!(daily.coverage.sessions_without_usage, 0);
    assert_eq!(total(&report(ModelUsageRange::Last7Days, sources.clone(), vec![], now())), 23);
    assert_eq!(total(&report(ModelUsageRange::Last30Days, sources.clone(), vec![], now())), 36);
    let all = report(ModelUsageRange::All, sources, vec![], now());
    assert_eq!(total(&all), 36);
    assert_eq!(all.days.len(), 2);
}

#[test]
fn continuations_deduplicate_request_identity_and_count_equal_usage_from_distinct_calls() {
    let temp = tempfile::tempdir().unwrap();
    let first = codex("same", 16, 20);
    let mut next = first.clone();
    next.push(first[2].clone());
    let mut second = first[2].clone();
    second["payload"]["response_id"] = json!("another-real-response");
    next.push(second);
    let mut source = source(temp.path(), "a.jsonl", "same", AppKind::Codex, &first);
    let continuation = source.paths[0].with_file_name("b.jsonl");
    std::fs::write(&continuation, next.iter().map(|value| format!("{value}\n")).collect::<String>()).unwrap();
    source.paths.push(continuation);
    let read = report(ModelUsageRange::All, vec![source], vec![], now());
    assert_eq!(total(&read), 46);
    assert_eq!(read.coverage.scanned_files, 2);
    assert_eq!(read.groups[0].session_count, 1);
}

#[test]
fn claude_stream_fragments_replays_and_subagents_keep_vendor_message_ownership() {
    let temp = tempfile::tempdir().unwrap();
    let main = source(temp.path(), "main.jsonl", "main", AppKind::Claude,
        &[claude("message-1", 16, 0, false), claude("message-1", 16, 3, true)]);
    let child = source(temp.path(), "child.jsonl", "main-agent-1", AppKind::Claude,
        &[claude("message-1", 16, 3, true), claude("message-2", 16, 4, true)]);
    let read = report(ModelUsageRange::Today, vec![main, child], vec![], now());
    assert!(read.issues.is_empty());
    assert_eq!(total(&read), 53);
    assert_eq!(read.groups[0].session_count, 2);
}

#[test]
fn partial_and_invalid_records_preserve_observations_and_explain_coverage() {
    let temp = tempfile::tempdir().unwrap();
    let source = source(temp.path(), "broken.jsonl", "one", AppKind::Codex, &codex("one", 16, 10));
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().append(true).open(&source.paths[0]).unwrap();
    file.write_all(b"{invalid\n{\"type\":").unwrap();
    let read = report(ModelUsageRange::Today, vec![source], vec![], now());
    assert_eq!(total(&read), 13);
    assert_eq!(read.issues.len(), 2);
}

#[test]
fn native_history_without_call_records_reports_coverage_without_fabricating_consumption() {
    let mut native = fixture_source(Path::new("unused"), AppKind::Codex, "native-only");
    native.paths.clear();
    for range in [ModelUsageRange::All, ModelUsageRange::Today] {
        let read = report(range, vec![native.clone()], vec![], now());
        assert_eq!(total(&read), 0);
        assert!(read.groups.is_empty() && read.days.is_empty());
        assert_eq!(read.coverage.sessions_without_usage, 1);
    }
}

#[test]
fn every_range_reuses_the_same_indexed_events_and_removed_sources_stop_contributing() {
    let temp = tempfile::tempdir().unwrap();
    let source = source(temp.path(), "shared.jsonl", "same", AppKind::Codex, &codex("same", 16, 10));
    let root = temp.path().join("state");
    let first = reports(&root, vec![source.clone()], vec![], now()).unwrap();
    assert_eq!(first.len(), 4);
    let before = crate::session_manager::records::decoded_record_count();
    let second = reports(&root, vec![source], vec![], now()).unwrap();
    assert_eq!(crate::session_manager::records::decoded_record_count(), before);
    assert!(second.iter().all(|report| total(report) == 13));
    let empty = reports(&root, vec![], vec![], now()).unwrap();
    assert!(empty.iter().all(|report| total(report) == 0 && report.coverage.source_sessions == 0));
}

#[test]
fn forked_prefix_receipts_retain_the_original_owner_without_counting_inherited_totals() {
    let temp = tempfile::tempdir().unwrap();
    let parent_records = codex("parent", 15, 10);
    let parent = source(temp.path(), "parent.jsonl", "parent", AppKind::Codex, &parent_records);
    let mut records = codex("child", 16, 10);
    records[0]["payload"]["forked_from_id"] = json!("parent");
    records[2]["payload"]["thread_token_usage"] = counts(99997);
    records.insert(1, parent_records[2].clone());
    records.insert(1, parent_records[1].clone());
    let child = source(temp.path(), "child.jsonl", "child", AppKind::Codex, &records);
    let all = report(ModelUsageRange::All, vec![child.clone(), parent.clone()], vec![], now());
    assert!(all.issues.is_empty());
    assert_eq!(total(&all), 26);
    assert_eq!(all.groups[0].session_count, 2);
    let today = report(ModelUsageRange::Today, vec![child, parent], vec![], now());
    assert_eq!(total(&today), 13);
}

#[test]
fn context_snapshots_rate_updates_and_counter_resets_never_create_calls() {
    let temp = tempfile::tempdir().unwrap();
    let mut records = codex("one", 16, 10);
    for limit in ["codex", "other", "codex"] {
        records.push(json!({"type":"event_msg","timestamp":at(16),"payload":{"type":"token_count",
            "rate_limits":{"limit_id":limit},"info":{"last_token_usage":counts(9997),"total_token_usage":counts(99997)}}}));
    }
    records.push(json!({"type":"event_msg","timestamp":at(16),"payload":{"type":"token_count",
        "info":{"last_token_usage":{"total_tokens":500},"total_token_usage":{"total_tokens":500}}}}));
    records.push(codex("one", 16, 5)[2].clone());
    let source = source(temp.path(), "one.jsonl", "one", AppKind::Codex, &records);
    let read = report(ModelUsageRange::All, vec![source], vec![], now());
    assert!(read.issues.is_empty());
    assert_eq!(total(&read), 21);
}

#[test]
fn compaction_receipt_is_undated_until_its_original_call_is_found() {
    let temp = tempfile::tempdir().unwrap();
    let original = codex("one", 16, 10);
    let checkpoint = vec![original[0].clone(), json!({"type":"compacted","timestamp":at(16),
        "payload":{"latest_token_usage_record":original[2]["payload"]}})];
    let compact = source(temp.path(), "a.jsonl", "one", AppKind::Codex, &checkpoint);
    let all = report(ModelUsageRange::All, vec![compact.clone()], vec![], now());
    assert_eq!(total(&all), 13);
    assert_eq!(all.unassigned_tokens.total_tokens, 13);
    let today = report(ModelUsageRange::Today, vec![compact.clone()], vec![], now());
    assert_eq!(total(&today), 0);
    assert_eq!(today.coverage.undated_event_tokens, 13);
    let raw = source(temp.path(), "b.jsonl", "one", AppKind::Codex, &original);
    let mut merged = compact;
    merged.paths.extend(raw.paths);
    let read = report(ModelUsageRange::Today, vec![merged], vec![], now());
    assert_eq!(total(&read), 13);
    assert_eq!(read.groups[0].model.as_deref(), Some("model-codex"));
    assert_eq!(read.coverage.undated_event_tokens, 0);
}

#[test]
fn missing_dates_and_models_are_explicit_and_invalid_components_are_not_accepted() {
    let temp = tempfile::tempdir().unwrap();
    let mut records = codex("one", 16, 10);
    records[2].as_object_mut().unwrap().remove("timestamp");
    records[1]["payload"]["turn_id"] = json!("unrelated-turn");
    let mut invalid = codex("one", 16, 20)[2].clone();
    invalid["payload"]["usage"]["cache_write_input_tokens"] = json!(21);
    records.push(invalid);
    let source = source(temp.path(), "one.jsonl", "one", AppKind::Codex, &records);
    let today = report(ModelUsageRange::Today, vec![source.clone()], vec![], now());
    assert_eq!(total(&today), 0);
    assert_eq!(today.coverage.undated_event_tokens, 13);
    assert_eq!(today.issues.len(), 1);
    let all = report(ModelUsageRange::All, vec![source], vec![], now());
    assert_eq!(total(&all), 13);
    assert_eq!(all.unassigned_tokens.total_tokens, 13);
    assert!(all.groups[0].model.is_none());
}
