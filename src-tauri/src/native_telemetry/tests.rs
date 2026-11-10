use super::*;
use asb_core::contracts::AppKind;
use serde_json::{json, Value};

fn attributes(properties: Value) -> Vec<Value> {
    properties.as_object().unwrap().iter().map(|(key, value)| {
        let wrapped = match value {
            Value::Bool(value) => json!({"boolValue":value}),
            Value::Number(value) if value.is_u64() => json!({"intValue":value.to_string()}),
            Value::Number(value) => json!({"doubleValue":value}),
            _ => json!({"stringValue":value}),
        };
        json!({"key":key,"value":wrapped})
    }).collect()
}

pub(super) fn logs(name: &str, mut properties: Value) -> Value {
    properties["event.name"] = json!(name);
    json!({"resourceLogs":[{"scopeLogs":[{"logRecords":[{
        "timeUnixNano":"1700000000000000000", "attributes":attributes(properties),
        "body":{"stringValue":"DO_NOT_STORE_RAW_CONTENT"}
    }]}]}]})
}

pub(super) fn span() -> Value {
    json!({"resourceSpans":[{"scopeSpans":[{"spans":[{
        "name":"claude_code.llm_request", "traceId":"1234567890abcdef1234567890abcdef",
        "spanId":"1234567890abcdef", "startTimeUnixNano":"1700000000000000000",
        "endTimeUnixNano":"1700000001000000000", "attributes":attributes(json!({
            "model":"test-model", "success":true, "duration_ms":1000, "ttft_ms":100.5, "first_content_ms":200,
            "input_tokens":10,"cache_read_tokens":20,"cache_creation_tokens":5,"output_tokens":50,"attempt":2,
            "user.email":"DO_NOT_STORE_ACCOUNT", "request_id":"request-native-1"
        }))
    }]}]}]})
}

pub(super) fn query(scope: MeasurementScope) -> crate::request_usage::RequestUsageQuery {
    serde_json::from_value(json!({"range":"all", "app":null,"scope":scope,"model":null,"page":1,"groupPage":1})).unwrap()
}

#[test]
fn codex_terminal_usage_never_reuses_event_duration_as_generation_duration() {
    let value = logs("codex.sse_event", json!({"event.kind":"response.completed",
        "model":"test-model","duration_ms":10,"input_token_count":"100",
        "output_token_count":"40","cached_token_count":20,"ttft_ms":150}));
    let batch = decode::logs(AppKind::Codex, &value).unwrap();
    assert_eq!(batch.rows.len(), 1);
    let row = &batch.rows[0];
    assert_eq!(row.scope, MeasurementScope::Generation);
    assert_eq!(row.duration_ms, None);
    assert_eq!(row.output_tokens_per_second, None);
    assert_eq!(row.first_token_ms, Some(150.0));
    assert_eq!(row.input_tokens, Some(100));
    let mut invented = row.clone();
    invented.duration_ms = Some(1000.0);
    invented.normalize();
    assert!(invented.validate().is_err());
    let generic = logs("codex.sse_event", json!({"event.kind":"response.completed","duration_ms":10}));
    assert_eq!(decode::logs(AppKind::Codex, &generic).unwrap().ignored, 1);
    let partial = logs("codex.sse_event", json!({"event.kind":"response.incomplete","duration_ms":10}));
    let partial = decode::logs(AppKind::Codex, &partial).unwrap();
    assert_eq!(partial.rows[0].outcome, RequestOutcome::Failed);
    assert_eq!(partial.rows[0].event_kind.as_deref(), Some("response.incomplete"));
    assert_eq!(partial.rows[0].status, None);
    assert_eq!(partial.rows[0].duration_ms, None);
}

#[test]
fn transport_success_and_generation_results_are_separate() {
    let value = logs("codex.api_request", json!({"http.response.status_code":200,"duration_ms":20}));
    let batch = decode::logs(AppKind::Codex, &value).unwrap();
    let row = &batch.rows[0];
    assert_eq!(row.scope, MeasurementScope::Transport);
    assert_eq!(row.outcome, RequestOutcome::Succeeded);
    assert_eq!(row.duration_ms, Some(20.0));
    assert_eq!(row.output_tokens_per_second, None);
    assert_eq!(row.output_tokens, None);
}

#[test]
fn claude_span_uses_native_request_identity_full_duration_and_separate_cache_fields() {
    let batch = decode::traces(&span()).unwrap();
    let row = &batch.rows[0];
    assert_eq!(row.duration_ms, Some(1000.0));
    assert_eq!(row.output_tokens_per_second, Some(50.0));
    assert_eq!(row.input_tokens, Some(35));
    assert_eq!(row.first_token_ms, Some(100.5));
    assert_eq!(row.first_content_ms, Some(200.0));
    assert_eq!(row.attempt_count, Some(2));
    assert_eq!(row.request_id.as_deref(), Some("request-native-1"));
}

#[test]
fn span_bounds_do_not_fill_missing_native_duration_and_unrelated_spans_do_not_count() {
    let mut value = span();
    let record = &mut value["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
    record["attributes"].as_array_mut().unwrap().retain(|entry| entry["key"] != "duration_ms");
    let batch = decode::traces(&value).unwrap();
    assert_eq!(batch.rows[0].duration_ms, None);
    assert_eq!(batch.rows[0].output_tokens_per_second, None);
    value["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["name"] = json!("claude_code.tool");
    assert_eq!(decode::traces(&value).unwrap().ignored, 1);
}

#[test]
fn replay_is_idempotent_conflicts_rollback_and_private_content_is_not_persisted() {
    let temp = tempfile::tempdir().unwrap();
    store::initialize(temp.path()).unwrap();
    let batch = decode::traces(&span()).unwrap();
    assert_eq!(store::append(temp.path(), &batch.rows).unwrap(), 1);
    assert_eq!(store::append(temp.path(), &batch.rows).unwrap(), 0);
    let raw = logs("codex.api_request", json!({"http.response.status_code":200,"duration_ms":20}));
    store::append(temp.path(), &decode::logs(AppKind::Codex, &raw).unwrap().rows).unwrap();
    let mut changed = batch.rows[0].clone();
    changed.model = Some("changed-model".into());
    assert!(store::append(temp.path(), &[changed]).is_err());
    let report = crate::request_usage::read(temp.path(), query(MeasurementScope::Generation)).unwrap();
    assert_eq!(report.summary.records, 1);
    assert_eq!(report.rows[0].model.as_deref(), Some("test-model"));
    assert_eq!(crate::request_usage::read(temp.path(), query(MeasurementScope::Transport)).unwrap().summary.records, 1);
    let connection = rusqlite::Connection::open(temp.path().join("state/request-performance.sqlite3")).unwrap();
    let body: String = connection.query_row("SELECT group_concat(body) FROM observations", [], |r| r.get(0)).unwrap();
    assert!(!body.contains("DO_NOT_STORE"));
    assert!(!body.contains("user.email"));
}

#[test]
fn empty_exports_are_valid_and_invalid_native_span_identity_is_rejected() {
    assert_eq!(decode::logs(AppKind::Codex, &json!({})).unwrap().rows.len(), 0);
    let mut value = span();
    value["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["traceId"] = json!("bad");
    assert!(decode::traces(&value).is_err());
    assert!(decode::logs(AppKind::Codex, &json!({"resourceLogs":"not-an-array"})).is_err());
}

#[test]
fn native_observed_timestamps_are_distinguished_from_event_timestamps() {
    let mut value = logs("codex.api_request", json!({"http.response.status_code":200,"duration_ms":20}));
    let record = &mut value["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0];
    record["timeUnixNano"] = json!("0");
    record["observedTimeUnixNano"] = json!("1700000000000000000");
    let batch = decode::logs(AppKind::Codex, &value).unwrap();
    assert_eq!(batch.rows[0].time_source, NativeTimeSource::Observed);
    value["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["timeUnixNano"] = json!("invalid");
    assert!(decode::logs(AppKind::Codex, &value).is_err());
}

#[test]
fn out_of_order_native_spans_have_stable_pages_and_full_aggregates() {
    let temp = tempfile::tempdir().unwrap();
    store::initialize(temp.path()).unwrap();
    for index in 1..=32_u64 {
        let mut value = span();
        let record = &mut value["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
        record["spanId"] = json!(format!("{index:016x}"));
        record["startTimeUnixNano"] = json!((1_700_000_000_000_000_000 + index * 1_000_000_000).to_string());
        record["endTimeUnixNano"] = json!((1_700_000_001_000_000_000 + index * 1_000_000_000).to_string());
        store::append(temp.path(), &decode::traces(&value).unwrap().rows).unwrap();
    }
    let first = crate::request_usage::read(temp.path(), query(MeasurementScope::Generation)).unwrap();
    assert_eq!(first.summary.records, 32);
    assert_eq!(first.rows.len(), 25);
    assert!(first.rows[0].at_ms > first.rows[24].at_ms);
    let mut next = query(MeasurementScope::Generation);
    next.page = 2;
    let second = crate::request_usage::read(temp.path(), next).unwrap();
    assert_eq!(second.rows.len(), 7);
    assert_eq!(second.summary.records, 32);
    assert!(second.rows[0].at_ms < first.rows[24].at_ms);
}

#[test]
fn loopback_http_gzip_export_uses_the_real_decoder_and_store() {
    use std::io::Write;
    let temp = tempfile::tempdir().unwrap();
    store::initialize(temp.path()).unwrap();
    let listener = tiny_http::Server::http(("127.0.0.1", 0)).unwrap();
    let address = listener.server_addr().to_ip().unwrap();
    let root = temp.path().to_path_buf();
    let status = Arc::new(Mutex::new(ReceiverStatus::default()));
    let observed = status.clone();
    let worker = std::thread::spawn(move || {
        let request = listener.recv_timeout(std::time::Duration::from_secs(3)).unwrap().unwrap();
        server::receive(request, &root, &observed);
    });
    let value = logs("codex.api_request", json!({"http.response.status_code":200,"duration_ms":20}));
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(value.to_string().as_bytes()).unwrap();
    let response = reqwest::blocking::Client::builder().no_proxy().timeout(std::time::Duration::from_secs(3))
        .build().unwrap().post(format!("http://{address}/codex/v1/logs"))
        .header("Content-Type", "application/json").header("Content-Encoding", "gzip")
        .body(gzip.finish().unwrap()).send().unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(response.text().unwrap(), "{}");
    worker.join().unwrap();
    assert_eq!(status.lock().unwrap().accepted_records, 1);
    assert_eq!(crate::request_usage::read(temp.path(), query(MeasurementScope::Transport)).unwrap().summary.records, 1);
}

#[test]
fn foreign_schema_is_preserved_and_interrupted_empty_initialization_is_recoverable() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("state")).unwrap();
    let path = temp.path().join("state/request-performance.sqlite3");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE business(value TEXT); INSERT INTO business VALUES ('keep');").unwrap();
    assert!(store::initialize(temp.path()).is_err());
    let value: String = connection.query_row("SELECT value FROM business", [], |row| row.get(0)).unwrap();
    assert_eq!(value, "keep");
    let empty = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(empty.path().join("state")).unwrap();
    drop(rusqlite::Connection::open(empty.path().join("state/request-performance.sqlite3")).unwrap());
    store::initialize(empty.path()).unwrap();
    store::initialize(empty.path()).unwrap();
}

#[test]
fn exporter_candidates_preserve_model_connections_and_shared_intentions() {
    let mut codex_settings = asb_core::ownership::default_client_settings(AppKind::Codex);
    let original = "model = 'test-model'\nmodel_provider = 'test-provider'\n[otel]\ntrace_exporter = 'none'\n";
    let rendered = configuration::render(AppKind::Codex, original, true, &mut codex_settings).unwrap();
    assert!(configuration::configured(AppKind::Codex, &rendered).unwrap());
    let document: toml_edit::DocumentMut = rendered.parse().unwrap();
    assert_eq!(document["model_provider"].as_str(), Some("test-provider"));
    assert_eq!(document["otel"]["trace_exporter"].as_str(), Some("none"));
    let disabled = configuration::render(AppKind::Codex, &rendered, false, &mut codex_settings).unwrap();
    assert!(!configuration::configured(AppKind::Codex, &disabled).unwrap());
    let mut claude_settings = asb_core::ownership::default_client_settings(AppKind::Claude);
    claude_settings.claude_extra.insert("env".into(), json!({"OTEL_LOGS_EXPORTER":"console","KEEP":"unchanged"}));
    let rendered = configuration::render(AppKind::Claude,
        r#"{"env":{"ANTHROPIC_BASE_URL":"https://test.example","KEEP":"unchanged"}}"#,
        true, &mut claude_settings).unwrap();
    let document: Value = serde_json::from_str(&rendered).unwrap();
    assert!(configuration::configured(AppKind::Claude, &rendered).unwrap());
    assert_eq!(document["env"]["ANTHROPIC_BASE_URL"], "https://test.example");
    assert_eq!(claude_settings.claude_extra["env"]["OTEL_LOGS_EXPORTER"], "otlp");
    assert_eq!(claude_settings.claude_extra["env"]["KEEP"], "unchanged");
}
