use super::*;
use asb_core::contracts::AppKind;
use serde_json::json;
use serde_json::Value;
use rusqlite::params;
use std::path::Path;

fn history(path: &Path) -> Connection {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch("CREATE TABLE thread_items(thread_id TEXT,turn_id TEXT,item_id TEXT,created_at_ms INTEGER,item_json TEXT,rollout_ordinal INTEGER,updated_at_ordinal INTEGER NOT NULL DEFAULT 0); \
        CREATE TABLE thread_realtime_items(thread_id TEXT,item_id TEXT,created_at_ms INTEGER,item_json TEXT,rollout_ordinal INTEGER);").unwrap();
    connection
}
fn insert(connection: &Connection, id: &str, at: i64, value: &Value) {
    connection.execute("INSERT INTO thread_items VALUES('native','turn',?1,?2,?3,?2,?2)",
        params![id,at,value.to_string()]).unwrap();
}
fn read(source: &SessionSource) -> (Vec<SessionMessage>, Vec<String>) {
    let mut messages = Vec::new();
    let issues = source.visit(|message| { messages.push(message); Ok(ControlFlow::Continue(())) }).unwrap();
    (messages, issues)
}

#[test]
fn native_history_remains_readable_without_any_original_rollout() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("history.sqlite");
    let connection = history(&path);
    insert(&connection, "u", 1, &json!({"type":"userMessage","id":"u","content":[{"type":"text","text":"question"}]}));
    insert(&connection, "a", 2, &json!({"type":"agentMessage","id":"a","text":"retained answer"}));
    drop(connection);
    let original = std::fs::read(&path).unwrap();
    let mut source = crate::session_manager::fixture_source(&temp.path().join("missing.jsonl"), AppKind::Codex, "native");
    source.paths.clear();
    source.native_history = Some(path.clone());
    let (messages, issues) = read(&source);
    assert!(issues.is_empty());
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content, "retained answer");
    assert_eq!(std::fs::read(&path).unwrap(), original, "native history is read-only");
}

#[test]
fn projected_mirrors_are_deduplicated_and_protocol_only_agent_content_survives() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.sqlite");
    let connection = history(&database);
    let answer = json!({"type":"agentMessage","id":"a","text":"answer"});
    insert(&connection, "a", 1_000, &answer);
    let rollout = temp.path().join("rollout.jsonl");
    let records = [json!({"type":"event_msg","timestamp":"1970-01-01T00:00:01Z","payload":{"type":"item_completed","item":answer}}),
        json!({"type":"response_item","timestamp":"1970-01-01T00:00:02Z","payload":{"type":"agent_message","author":"/root/worker",
            "content":[{"type":"input_text","text":"agent detail"},{"type":"encrypted_content","encrypted_content":"cipher"}]}})];
    std::fs::write(&rollout, records.iter().map(|record| format!("{record}\n")).collect::<String>()).unwrap();
    let mut source = crate::session_manager::fixture_source(&rollout, AppKind::Codex, "native");
    source.native_history = Some(database);
    let (messages, issues) = read(&source);
    assert!(issues.is_empty());
    assert_eq!(messages.len(), 2);
    assert!(messages[1].content.contains("agent detail"));
    assert!(messages[1].content.contains("cipher"));
}

#[test]
fn an_invalid_native_record_does_not_hide_valid_neighbors() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("history.sqlite");
    let connection = history(&path);
    insert(&connection, "a", 1, &json!({"type":"agentMessage","id":"a","text":"valid"}));
    connection.execute("INSERT INTO thread_items VALUES('native','turn','bad',2,'invalid',2,2)", []).unwrap();
    let mut source = crate::session_manager::fixture_source(&temp.path().join("unused"), AppKind::Codex, "native");
    source.paths.clear();
    source.native_history = Some(path);
    let (messages, issues) = read(&source);
    assert_eq!(messages.len(), 1);
    assert_eq!(issues.len(), 1);
}

#[test]
fn raw_messages_reasoning_and_repeated_content_survive_a_partial_native_projection() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.sqlite");
    let connection = history(&database);
    insert(&connection, "a", 1_000, &json!({"type":"agentMessage","id":"a","text":"retained answer"}));
    let rollout = temp.path().join("rollout.jsonl");
    let user = json!({"type":"response_item","timestamp":"1970-01-01T00:00:02Z","payload":{
        "type":"message","role":"user","content":[{"type":"input_text","text":"unprojected question"}]}});
    let records = [user.clone(), user, json!({"type":"response_item","timestamp":"1970-01-01T00:00:03Z",
        "payload":{"type":"reasoning","encrypted_content":"retained cipher"}})];
    let original = records.iter().map(|record| format!("{record}\n")).collect::<String>();
    std::fs::write(&rollout, &original).unwrap();
    let copy = temp.path().join("copy.jsonl");
    std::fs::write(&copy, &original).unwrap();
    let mut source = crate::session_manager::fixture_source(&rollout, AppKind::Codex, "native");
    source.native_history = Some(database);
    source.paths.push(copy);
    let (messages, issues) = read(&source);
    assert!(issues.is_empty());
    assert_eq!(messages.len(), 4);
    assert_eq!(messages.iter().filter(|message| message.content == "unprojected question").count(), 2);
    assert!(messages[3].content.contains("retained cipher"));
}

#[test]
fn unreadable_native_history_does_not_hide_an_independent_readable_rollout() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.sqlite");
    std::fs::write(&database, "invalid SQLite database").unwrap();
    let rollout = temp.path().join("rollout.jsonl");
    std::fs::write(&rollout, "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":\"still readable\"}}\n").unwrap();
    let mut source = crate::session_manager::fixture_source(&rollout, AppKind::Codex, "native");
    source.native_history = Some(database);
    let (messages, issues) = read(&source);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "still readable");
    assert!(!issues.is_empty());
}

#[test]
fn native_index_reuses_unchanged_items_and_decodes_only_updated_rows() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.sqlite");
    let connection = history(&database);
    for ordinal in 0..40 {
        insert(&connection, &format!("item-{ordinal}"), ordinal, &json!({
            "type":"agentMessage","id":format!("item-{ordinal}"),"text":"original"
        }));
    }
    let index = ReadIndex::open(&temp.path().join("state")).unwrap();
    native_index::positions(&index, &connection, &database, "native").unwrap();
    let before = native_index::source_read_count();
    native_index::positions(&index, &connection, &database, "native").unwrap();
    assert_eq!(native_index::source_read_count(), before);
    let updated = json!({"type":"agentMessage","id":"item-2","text":"updated"});
    connection.execute("UPDATE thread_items SET item_json=?1,updated_at_ordinal=100 WHERE item_id='item-2'", [updated.to_string()]).unwrap();
    let (positions, issues) = native_index::positions(&index, &connection, &database, "native").unwrap();
    assert!(issues.is_empty());
    assert_eq!(native_index::source_read_count() - before, 1);
    assert!(positions.iter().any(|position| position.id == parser::native_identity(&updated)));
    connection.execute("DELETE FROM thread_items WHERE item_id='item-3'", []).unwrap();
    assert_eq!(native_index::positions(&index, &connection, &database, "native").unwrap().0.len(), 39);
}
