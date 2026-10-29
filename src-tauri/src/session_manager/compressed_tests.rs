use super::*;
use serde_json::{json, Value};

fn fixture(root: &Path) -> (PathBuf, PathBuf) {
    let mut values = vec![json!({"type":"session_meta","payload":{"id":"compressed-session"}})];
    for number in 0..36 {
        values.push(json!({"type":"response_item","payload":{"type":"message","role":"user",
            "content":[{"type":"input_text","text":format!("message {number} 中文正文")}]}}));
    }
    values.push(json!({"type":"turn_context","payload":{"turn_id":"turn-1","model":"model-codex"}}));
    let counts = json!({"input_tokens":10,"cached_input_tokens":2,"cache_write_input_tokens":1,
        "output_tokens":3,"reasoning_output_tokens":1,"total_tokens":13});
    values.push(json!({"type":"token_usage_record","timestamp":"2026-10-29T01:00:00Z","payload":{
        "thread_id":"compressed-session","turn_id":"turn-1","root_turn_id":"root-1",
        "session_id":"runtime-1","response_id":"response-1",
        "usage":counts,"turn_token_usage":counts,"thread_token_usage":counts}}));
    let text = values.iter().map(|value| format!("{value}\n")).collect::<String>();
    let plain = root.join("source.jsonl");
    let compressed = root.join("source.jsonl.zst");
    fs::write(&plain, text.as_bytes()).unwrap();
    fs::write(&compressed, zstd::stream::encode_all(text.as_bytes(), 1).unwrap()).unwrap();
    (plain, compressed)
}

#[test]
fn discovery_and_decoded_identity_merge_plain_and_compressed_continuations() {
    let temp = tempfile::tempdir().unwrap();
    let (plain, compressed) = fixture(temp.path());
    assert_eq!(parser::session_identity(&compressed).unwrap().unwrap().id, "compressed-session");
    let mut indexed = HashMap::new();
    let mut issues = Vec::new();
    sources::scan_roots(&[(AppKind::Codex, temp.path().into())], &mut indexed, &mut issues);
    assert!(issues.is_empty());
    assert_eq!(indexed.len(), 1);
    let source = indexed.values().next().unwrap();
    assert!(source.paths.contains(&plain) && source.paths.contains(&compressed));
    let index = timeline::index(source).unwrap();
    assert_eq!(index.entries.len(), 36);
}

#[test]
fn compressed_pages_and_continuous_reads_keep_original_ids_and_unicode_offsets() {
    let temp = tempfile::tempdir().unwrap();
    let (plain, compressed) = fixture(temp.path());
    let original = timeline::index(&fixture_source(&plain, AppKind::Codex, "compressed-session")).unwrap();
    let packed = timeline::index(&fixture_source(&compressed, AppKind::Codex, "compressed-session")).unwrap();
    let selected = vec![packed.entries[31].clone(), packed.entries[2].clone(), packed.entries[17].clone()];
    let page = packed.load_batch(&selected, |message| (message.id, message.content));
    for (actual, expected) in page.into_iter().zip([31, 2, 17]) {
        let message = original.load(&original.entries[expected]).unwrap();
        assert_eq!(actual.unwrap(), (message.id, message.content));
    }
    let mut count = 0;
    assert!(packed.visit(|message| {
        assert!(message.content.contains("中文正文"));
        count += 1;
        Ok(std::ops::ControlFlow::Continue(()))
    }).unwrap().is_empty());
    assert_eq!(count, 36);
}

#[test]
fn compressed_usage_is_exact_and_duplicate_encodings_do_not_double_count_calls() {
    let temp = tempfile::tempdir().unwrap();
    let (plain, compressed) = fixture(temp.path());
    let mut source = fixture_source(&plain, AppKind::Codex, "compressed-session");
    source.paths.push(compressed);
    let report = crate::model_usage::report(asb_core::contracts::ModelUsageRange::All,
        vec![source], Vec::new(), chrono::Local::now());
    assert!(report.issues.is_empty());
    assert_eq!(report.coverage.scanned_files, 2);
    assert_eq!(report.groups.iter().map(|group| group.total_tokens).sum::<u64>(), 13);
}

#[test]
fn damaged_archives_and_replaced_cursor_sources_remain_explicit_read_errors() {
    let temp = tempfile::tempdir().unwrap();
    let (_, compressed) = fixture(temp.path());
    let index = timeline::index(&fixture_source(&compressed, AppKind::Codex, "compressed-session")).unwrap();
    index.load(&index.entries[0]).unwrap();
    let bytes = fs::read(&compressed).unwrap();
    fs::write(&compressed, &bytes[..bytes.len() / 2]).unwrap();
    assert!(index.load(&index.entries[1]).is_err());
    assert!(records::visit_records(&compressed, 0, |_, _, _| Ok(std::ops::ControlFlow::Continue(()))).is_err());
    fs::write(&compressed, b"not a Zstandard archive").unwrap();
    assert!(parser::session_identity(&compressed).is_err());
}

#[test]
fn compressed_only_sessions_support_actual_search_and_markdown_export() {
    let _clients = crate::test_client_paths::redirect_client_paths();
    let temp = tempfile::tempdir().unwrap();
    let client = PathBuf::from(std::env::var_os("CODEX_HOME").unwrap());
    let sessions = client.join("sessions");
    fs::create_dir_all(&sessions).unwrap();
    let (plain, compressed) = fixture(&sessions);
    fs::remove_file(plain).unwrap();
    let original = fs::read(&compressed).unwrap();
    let request = serde_json::from_value(json!({"query":"message 31","app":"codex","offset":0,
        "project":null,"tag":null,"pinnedOnly":false,"activeAfter":null,"activeBefore":null})).unwrap();
    let state = temp.path().join("state");
    let result = search_sessions(&state, request).unwrap();
    assert!(result.issues.is_empty());
    assert_eq!(result.total, 1);
    let output = temp.path().join("conversation.md");
    export_markdown(&state, AppKind::Codex, "compressed-session", &output).unwrap();
    let exported = fs::read_to_string(output).unwrap();
    assert!(exported.contains("message 31 中文正文"));
    assert!(exported.contains("message 0 中文正文"));
    assert_eq!(fs::read(compressed).unwrap(), original);
}
