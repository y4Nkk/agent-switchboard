use super::*;
use crate::native_telemetry::{tests::{logs, span, query}, MeasurementScope, RequestOutcome};
use serde_json::json;
use std::io::{Cursor, Write};

const CODEX: &str = r#"{"resourceLogs":[{"scopeLogs":[{"logRecords":[{
    "timeUnixNano":"1700000000000000000","attributes":[
        {"key":"event.name","value":{"stringValue":"codex.sse_event"}},
        {"key":"event.kind","value":{"stringValue":"response.completed"}},
        {"key":"model","value":{"stringValue":"sandbox-codex"}},
        {"key":"output_token_count","value":{"intValue":"40"}},
        {"key":"ttft_ms","value":{"doubleValue":150.5}}
    ]}]}]}]}"#;
const CLAUDE: &str = r#"{"resourceLogs":[{"scopeLogs":[{"logRecords":[{
    "timeUnixNano":"1700000000000000000","attributes":[
        {"key":"event.name","value":{"stringValue":"api_request"}},
        {"key":"model","value":{"stringValue":"sandbox-claude"}},
        {"key":"duration_ms","value":{"intValue":"1200"}},
        {"key":"input_tokens","value":{"intValue":"10"}},
        {"key":"cache_read_tokens","value":{"intValue":"20"}},
        {"key":"cache_creation_tokens","value":{"intValue":"5"}},
        {"key":"cost_usd_micros","value":{"intValue":"123"}}
    ]}]}]}]}"#;
const CLAUDE_SPAN: &str = r#"{"resourceSpans":[{"scopeSpans":[{"spans":[{
    "name":"claude_code.llm_request","traceId":"1234567890abcdef1234567890abcdef",
    "spanId":"1234567890abcdef","startTimeUnixNano":"1700000000000000000",
    "endTimeUnixNano":"1700000001000000000","attributes":[
        {"key":"model","value":{"stringValue":"sandbox-claude"}},
        {"key":"success","value":{"boolValue":true}},
        {"key":"duration_ms","value":{"intValue":"1000"}},
        {"key":"output_tokens","value":{"intValue":"50"}}
    ]}]}]}]}"#;

fn request(path: &str, body: &'static str) -> Request {
    tiny_http::TestRequest::new().with_method(Method::Post).with_path(path).with_body(body)
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap()).into()
}

#[test]
fn both_client_http_routes_persist_filter_and_deduplicate_native_records() {
    let temp = tempfile::tempdir().unwrap();
    store::initialize(temp.path()).unwrap();
    let status = Arc::new(Mutex::new(ReceiverStatus::default()));
    for _ in 0..2 {
        for (path, body) in [("/codex/v1/logs", CODEX), ("/claude/v1/logs", CLAUDE),
            ("/claude/v1/traces", CLAUDE_SPAN)] {
            receive(request(path, body), temp.path(), &status);
            assert_eq!(status.lock().unwrap().error, None);
        }
    }
    assert_eq!(status.lock().unwrap().accepted_records, 3);
    assert!(status.lock().unwrap().last_export_at_ms.is_some());
    let report = crate::request_usage::read(temp.path(), query(MeasurementScope::Generation)).unwrap();
    assert_eq!(report.summary.records, 2);
    assert!(report.summary.mixed_measurements);
    for (app, model, rate) in [(AppKind::Codex, "sandbox-codex", None),
        (AppKind::Claude, "sandbox-claude", Some(50.0))] {
        let mut selected = query(MeasurementScope::Generation);
        selected.app = Some(app);
        selected.model = Some(model.into());
        let report = crate::request_usage::read(temp.path(), selected).unwrap();
        assert_eq!(report.summary.records, 1);
        assert_eq!(report.rows[0].outcome, RequestOutcome::Succeeded);
        assert_eq!(report.rows[0].output_tokens_per_second, rate);
    }
    let report = crate::request_usage::read(temp.path(), query(MeasurementScope::Transport)).unwrap();
    assert_eq!(report.summary.records, 1);
    assert_eq!(report.rows[0].app, AppKind::Claude);
    assert_eq!(report.rows[0].duration_ms, Some(1200.0));
    assert_eq!(report.rows[0].input_tokens, Some(35));
    assert_eq!(report.rows[0].estimated_cost_usd.as_deref(), Some("0.000123"));
    let connection = rusqlite::Connection::open(temp.path().join("state/request-performance.sqlite3")).unwrap();
    assert_eq!(connection.query_row("SELECT count(*) FROM observations", [], |r| r.get::<_, u64>(0)).unwrap(), 3);
}

#[test]
fn gzip_exports_decode_both_clients_and_enforce_both_size_limits() {
    for (path, value) in [("/codex/v1/logs", logs("codex.api_request", json!({"duration_ms":20}))),
        ("/claude/v1/traces", span())] {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(value.to_string().as_bytes()).unwrap();
        let body = encoder.finish().unwrap();
        let batch = decode_export(&Method::Post, path, "application/json; charset=utf-8", "gzip",
            &mut Cursor::new(body)).unwrap();
        assert_eq!(batch.rows.len(), 1);
    }
    let expanded = vec![b' '; MAX_BODY as usize + 1];
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&expanded).unwrap();
    for (body, encoding) in [(expanded, "identity"), (encoder.finish().unwrap(), "gzip")] {
        let result = decode_export(&Method::Post, "/codex/v1/logs", "application/json", encoding,
            &mut Cursor::new(body));
        assert_eq!(result.err().unwrap().0, 413);
    }
}

#[test]
fn invalid_http_inputs_return_explicit_errors_without_mutating_saved_facts() {
    for (method, path, content_type, encoding, body, expected) in [
        (Method::Get, "/codex/v1/logs", "application/json", "", "{}", 405),
        (Method::Post, "/v1/logs", "application/json", "", "{}", 404),
        (Method::Post, "/codex/v1/logs", "application/x-protobuf", "", "{}", 415),
        (Method::Post, "/codex/v1/logs", "application/json", "br", "{}", 415),
        (Method::Post, "/codex/v1/logs", "application/json", "gzip", "broken", 400),
        (Method::Post, "/claude/v1/traces", "application/json", "", "invalid", 400),
    ] {
        let result = decode_export(&method, path, content_type, encoding, &mut Cursor::new(body));
        assert_eq!(result.err().unwrap().0, expected);
    }
    let temp = tempfile::tempdir().unwrap();
    store::initialize(temp.path()).unwrap();
    let status = Arc::new(Mutex::new(ReceiverStatus::default()));
    receive(request("/codex/v1/logs", CODEX), temp.path(), &status);
    receive(request("/claude/v1/traces", "invalid"), temp.path(), &status);
    assert!(status.lock().unwrap().error.is_some());
    assert_eq!(status.lock().unwrap().accepted_records, 1);
    let report = crate::request_usage::read(temp.path(), query(MeasurementScope::Generation)).unwrap();
    assert_eq!(report.summary.records, 1);
    receive(request("/claude/v1/logs", "{}"), temp.path(), &status);
    assert_eq!(status.lock().unwrap().error, None);
}

#[test]
fn later_identity_conflict_rolls_back_earlier_new_record_in_the_same_batch() {
    let temp = tempfile::tempdir().unwrap();
    store::initialize(temp.path()).unwrap();
    let mut original = decode::traces(&span()).unwrap().rows.remove(0);
    store::append(temp.path(), &[original.clone()]).unwrap();
    let mut new = original.clone();
    new.id = "claude:distinct-native-span".into();
    original.model = Some("conflicting-model".into());
    assert!(store::append(temp.path(), &[new, original]).is_err());
    let report = crate::request_usage::read(temp.path(), query(MeasurementScope::Generation)).unwrap();
    assert_eq!(report.summary.records, 1);
    assert_eq!(report.rows[0].model.as_deref(), Some("test-model"));
}
