use super::*;
use chrono::NaiveDate;
use serde_json::json;
use std::fs;

fn record() -> ClaudeRequestRecord {
    ClaudeRequestRecord {
        at: "2026-11-04T01:00:00.000Z".into(),
        profile_id: Some("test-profile".into()),
        route_revision: None,
        client_protocol: UpstreamProtocol::AnthropicMessages,
        upstream_protocol: None,
        request_model: None,
        mapped_model: None,
        response_model: None,
        cost: Some(ClaudeRequestCost { model: "model".into(), source: "fixture".into(),
            multiplier: "1".into(), total_usd: "1.25".into() }),
        pricing_error: None,
        first_token_latency_ms: None,
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cache_creation_tokens: None,
        reasoning_tokens: None,
        status: Some(200),
        duration_ms: 1,
        first_byte_latency_ms: None,
        failover_attempts: Vec::new(),
    }
}

#[test]
fn migration_restores_spend_older_than_retained_requests_and_imports_once() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("claude-request-ledger.json");
    let history = json!({
        "version": 2, "entries": [record()],
        "spendByDayUtc": {
            "2026-11-03": { "test-profile": 5_000_000 },
            "2026-11-04": { "test-profile": 10_000_000 },
            "2026-10-31": { "test-profile": 100_000_000 }
        }
    }).to_string();
    fs::write(&source, &history).unwrap();
    let ledger = ClaudeRequestLedger::new(root.path().to_path_buf());
    ledger.append(record()).unwrap();
    drop(ledger);

    let database = store::open(root.path()).unwrap();
    let count: usize = database.query_row("SELECT count(*) FROM requests", [], |row| row.get(0)).unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
    assert_eq!(count, 2);
    assert_eq!(store::totals(&database, "test-profile", date).unwrap(), (11_250_000, 16_250_000));
    assert_eq!(fs::read_to_string(source).unwrap(), history);
}

#[test]
fn spend_failure_rolls_back_the_inserted_request() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("claude-request-ledger.json");
    fs::write(source, json!({
        "version": 2,
        "spendByDayUtc": { "2026-11-04": { "test-profile": u64::MAX } }
    }).to_string()).unwrap();
    let ledger = ClaudeRequestLedger::new(root.path().to_path_buf());
    assert!(ledger.append(record()).unwrap_err().contains("费用累计"));
    let database = store::open(root.path()).unwrap();
    let count: usize = database.query_row("SELECT count(*) FROM requests", [], |row| row.get(0)).unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
    assert_eq!(count, 0);
    assert_eq!(store::totals(&database, "test-profile", date).unwrap(), (u64::MAX, u64::MAX));
}

#[test]
fn damaged_source_is_preserved_and_failed_initialization_can_be_retried() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("claude-request-ledger.json");
    fs::write(&source, "{bad").unwrap();
    assert!(store::open(root.path()).unwrap_err().contains("格式无效"));
    assert_eq!(fs::read_to_string(&source).unwrap(), "{bad");

    fs::write(source, json!({ "version": 1, "entries": [record()] }).to_string()).unwrap();
    let database = store::open(root.path()).unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
    assert_eq!(store::totals(&database, "test-profile", date).unwrap(), (1_250_000, 1_250_000));
}
