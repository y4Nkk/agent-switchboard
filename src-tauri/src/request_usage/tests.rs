use super::*;
use asb_core::contracts::UpstreamProtocol;
use crate::codex_metering::{CodexRequestLedger, CodexRequestRecord, CodexUsageOrigin};
use crate::gateway::request_ledger::{ClaudeRequestLedger, ClaudeRequestRecord};

fn row() -> RequestUsageRow {
    RequestUsageRow { id: uuid::Uuid::new_v4().to_string(), at_ms: 1_700_000_000_000,
        app: AppKind::Codex, profile_id: "test-provider".into(), model: Some("test-model".into()),
        model_source: RequestModelSource::Response, request_model: Some("alias".into()),
        mapped_model: Some("mapped-model".into()), response_model: Some("test-model".into()),
        upstream_protocol: Some(UpstreamProtocol::Responses), outcome: RequestOutcome::Completed,
        status: Some(200), duration_ms: 1000, first_output_ms: Some(100),
        output_tokens_per_second: Some(10.0), input_tokens: Some(100), output_tokens: Some(10),
        cache_read_tokens: Some(10), cache_creation_tokens: None, reasoning_tokens: None,
        estimated_cost_usd: Some("0.000001".into()), pricing_error: None, attempts: Vec::new() }
}

fn query() -> RequestUsageQuery {
    RequestUsageQuery { range: RequestUsageRange::All, app: None, profile_id: None,
        model: None, page: 1, group_page: 1 }
}

fn codex(row: RequestUsageRow) -> CodexRequestRecord {
    CodexRequestRecord { id: row.id, origin: CodexUsageOrigin::Proxy, thread_id: None, at_ms: row.at_ms,
        billable: true, profile_id: Some(row.profile_id), route_revision: Some("revision".into()),
        upstream_protocol: row.upstream_protocol, request_model: row.request_model,
        mapped_model: row.mapped_model, response_model: row.response_model, status: row.status,
        duration_ms: row.duration_ms, first_byte_latency_ms: None, first_token_latency_ms: row.first_output_ms,
        input_tokens: row.input_tokens, output_tokens: row.output_tokens, cache_read_tokens: row.cache_read_tokens,
        cache_creation_tokens: row.cache_creation_tokens, reasoning_tokens: row.reasoning_tokens,
        billing: Default::default(), cost: None, pricing_error: None, attempts: Vec::new() }
}

fn claude(row: RequestUsageRow) -> ClaudeRequestRecord {
    ClaudeRequestRecord { at: chrono::DateTime::from_timestamp_millis((row.at_ms + row.duration_ms) as i64)
        .unwrap().to_rfc3339(), profile_id: Some(row.profile_id), route_revision: Some("revision".into()),
        client_protocol: UpstreamProtocol::AnthropicMessages, upstream_protocol: row.upstream_protocol,
        request_model: row.request_model, mapped_model: row.mapped_model, response_model: row.response_model,
        cost: None, pricing_error: None, first_token_latency_ms: row.first_output_ms,
        input_tokens: row.input_tokens, output_tokens: row.output_tokens, cache_read_tokens: row.cache_read_tokens,
        cache_creation_tokens: row.cache_creation_tokens, reasoning_tokens: row.reasoning_tokens,
        status: row.status, duration_ms: row.duration_ms, first_byte_latency_ms: None, failover_attempts: Vec::new() }
}

#[test]
fn percentiles_use_individual_successful_requests_and_cache_is_token_weighted() {
    let mut acc = Accumulator::default();
    acc.add(&row());
    let mut second = row();
    second.output_tokens_per_second = Some(100.0);
    second.first_output_ms = Some(900);
    second.input_tokens = Some(900);
    second.cache_read_tokens = Some(810);
    acc.add(&second);
    let mut failure = row();
    failure.outcome = RequestOutcome::Failed;
    failure.status = Some(429);
    failure.output_tokens_per_second = None;
    acc.add(&failure);
    let mut disconnected = row();
    disconnected.outcome = RequestOutcome::Disconnected;
    disconnected.status = Some(499);
    disconnected.output_tokens_per_second = None;
    acc.add(&disconnected);
    let result = acc.finish();
    assert_eq!(result.requests, 4);
    assert_eq!(result.speed_samples, 2);
    assert_eq!(result.output_rate_p50, Some(55.0));
    assert_eq!(result.output_rate_p10, Some(19.0));
    assert_eq!(result.first_output_p50_ms, Some(500.0));
    assert_eq!(result.first_output_p95_ms, Some(860.0));
    assert_eq!(result.cache_hit_rate, Some(82.0));
    assert_eq!(result.cache_samples, 2);
    assert_eq!(result.disconnected, 1);
    assert_eq!(result.rate_limited, 1);
    assert!((result.completion_rate.unwrap() - 200.0 / 3.0).abs() < 1e-10);
    assert_eq!(result.estimated_cost_usd.as_deref(), Some("0.000004"));
}

#[test]
fn missing_counters_stay_unknown_and_invalid_cache_pairs_are_excluded() {
    let mut acc = Accumulator::default();
    let mut missing = row();
    missing.cache_read_tokens = None;
    missing.cache_creation_tokens = None;
    missing.reasoning_tokens = None;
    missing.estimated_cost_usd = None;
    acc.add(&missing);
    let mut invalid = missing.clone();
    invalid.cache_read_tokens = Some(101);
    acc.add(&invalid);
    let result = acc.finish();
    assert_eq!(result.cache_samples, 0);
    assert_eq!(result.cache_hit_rate, None);
    assert_eq!(result.estimated_cost_usd, None);
    assert_eq!(result.cost_samples, 0);
    let reasoning = result.tokens.iter().find(|entry| entry.kind == "reasoning").unwrap();
    assert_eq!(reasoning.tokens, None);
    assert_eq!(reasoning.samples, 0);
    let empty = Accumulator::default().finish();
    assert_eq!(empty.completion_rate, None);
    assert_eq!(empty.output_rate_p50, None);
}

#[test]
fn merge_keeps_raw_samples_and_recovered_attempts_instead_of_averaging_percentiles() {
    let mut first = Accumulator::default();
    first.add(&row());
    let mut second = Accumulator::default();
    let mut recovered = row();
    recovered.output_tokens_per_second = Some(100.0);
    recovered.attempts = vec![RequestUsageAttempt { profile_id: "first".into(), status: Some(429) },
        RequestUsageAttempt { profile_id: "second".into(), status: Some(200) }];
    for _ in 0..3 { second.add(&recovered); }
    first.merge(second);
    let result = first.finish();
    assert_eq!(result.output_rate_p50, Some(100.0));
    assert_eq!(result.retries, 3);
    assert_eq!(result.recovered_requests, 3);
    assert_eq!(result.speed_samples, 4);
}

#[test]
fn both_ledgers_share_start_time_order_and_paging_does_not_change_aggregates() {
    let temp = tempfile::tempdir().unwrap();
    let claude_path = temp.path().join("claude.json");
    let ledger = CodexRequestLedger::new(temp.path());
    for index in 0..31 {
        let mut item = row();
        item.at_ms += index * 1000;
        ledger.append(&codex(item)).unwrap();
    }
    let mut item = row();
    item.at_ms += 30_500;
    let expected_start = item.at_ms;
    ClaudeRequestLedger::new(claude_path.clone()).append(claude(item)).unwrap();
    let first = read(temp.path(), &claude_path, query()).unwrap();
    assert_eq!(first.summary.requests, 32);
    assert_eq!(first.rows.len(), 25);
    assert_eq!(first.rows[0].app, AppKind::Claude);
    assert_eq!(first.rows[0].at_ms, expected_start);
    assert_eq!(first.groups.len(), 2);
    assert!(first.rows.windows(2).all(|pair| pair[0].at_ms >= pair[1].at_ms));
    let mut page = query();
    page.page = 2;
    let second = read(temp.path(), &claude_path, page).unwrap();
    assert_eq!(second.rows.len(), 7);
    assert_eq!(second.summary.requests, first.summary.requests);
    assert_eq!(second.summary.output_rate_p50, first.summary.output_rate_p50);
    assert!(second.rows.iter().all(|row| first.rows.iter().all(|other| row.id != other.id)));
    let mut page = query();
    page.page = 1000;
    let last = read(temp.path(), &claude_path, page).unwrap();
    assert_eq!(last.page, 2);
    assert_eq!(last.rows.len(), 7);
}

#[test]
fn session_receipts_are_not_a_second_performance_source_and_filters_apply_before_paging() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("claude.json");
    let ledger = CodexRequestLedger::new(temp.path());
    let mut session = codex(row());
    session.origin = CodexUsageOrigin::Session;
    session.id = format!("{}:{}:0", crate::codex_metering::CODEX_SESSION_ID_PREFIX, uuid::Uuid::new_v4());
    session.billable = false;
    session.profile_id = None;
    session.route_revision = None;
    session.upstream_protocol = None;
    session.request_model = None;
    session.response_model = None;
    session.duration_ms = 0;
    session.first_token_latency_ms = None;
    ledger.append(&session).unwrap();
    let mut measured = row();
    measured.first_output_ms = Some(1500);
    ledger.append(&codex(measured)).unwrap();
    let report = read(temp.path(), &path, query()).unwrap();
    assert_eq!(report.summary.requests, 1);
    assert_eq!(report.summary.first_output_samples, 0);
    assert_eq!(report.summary.output_rate_p50, Some(10.0));
    assert_eq!(report.rows[0].model.as_deref(), Some("test-model"));
    assert_eq!(report.rows[0].model_source, RequestModelSource::Response);
    let mut filter = query();
    filter.model = Some("missing-model".into());
    assert_eq!(read(temp.path(), &path, filter).unwrap().summary.requests, 0);
}

#[test]
fn unreadable_sources_are_reported_and_absent_sources_do_not_create_files() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("claude.json");
    let empty = read(temp.path(), &path, query()).unwrap();
    assert!(empty.sources.iter().all(|source| source.error.is_none()));
    assert!(!path.exists());
    assert!(!temp.path().join("codex").exists());
    std::fs::write(&path, "{invalid").unwrap();
    let partial = read(temp.path(), &path, query()).unwrap();
    assert!(partial.sources.iter().any(|source| source.app == AppKind::Claude && source.error.is_some()));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{invalid");
    assert_eq!(partial.summary.completion_rate, None);
}

#[test]
fn partial_output_and_disconnections_never_enter_speed_or_cache_samples() {
    let temp = tempfile::tempdir().unwrap();
    let ledger = CodexRequestLedger::new(temp.path());
    for status in [206, 499] {
        let mut item = row();
        item.status = Some(status);
        ledger.append(&codex(item)).unwrap();
    }
    let report = read(temp.path(), &temp.path().join("claude.json"), query()).unwrap();
    assert_eq!(report.summary.requests, 2);
    assert_eq!(report.summary.completed, 0);
    assert_eq!(report.summary.failed, 1);
    assert_eq!(report.summary.incomplete, 1);
    assert_eq!(report.summary.disconnected, 1);
    assert_eq!(report.summary.completion_rate, Some(0.0));
    assert_eq!(report.summary.output_rate_p50, None);
    assert_eq!(report.summary.cache_samples, 0);
    assert!(report.rows.iter().all(|row| row.output_tokens_per_second.is_none()));
}

#[test]
fn reported_output_preserves_unknown_models_but_auxiliary_operations_do_not_count() {
    let temp = tempfile::tempdir().unwrap();
    let ledger = CodexRequestLedger::new(temp.path());
    let mut measured = row();
    measured.model = None;
    measured.request_model = None;
    measured.mapped_model = None;
    measured.response_model = None;
    ledger.append(&codex(measured.clone())).unwrap();
    let mut auxiliary = measured;
    auxiliary.id = uuid::Uuid::new_v4().to_string();
    auxiliary.output_tokens = None;
    ledger.append(&codex(auxiliary)).unwrap();
    let report = read(temp.path(), &temp.path().join("claude.json"), query()).unwrap();
    assert_eq!(report.summary.requests, 1);
    assert_eq!(report.rows[0].model, None);
    assert_eq!(report.rows[0].model_source, RequestModelSource::Unknown);
}

#[test]
fn rolling_windows_exclude_old_sources_and_provider_identity_requires_a_client() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("claude.json");
    CodexRequestLedger::new(temp.path()).append(&codex(row())).unwrap();
    ClaudeRequestLedger::new(path.clone()).append(claude(row())).unwrap();
    let mut recent = query();
    recent.range = RequestUsageRange::Last15Minutes;
    let report = read(temp.path(), &path, recent).unwrap();
    assert_eq!(report.summary.requests, 0);
    assert!(report.sources.iter().all(|source| source.verified_requests == 0));
    let mut invalid = query();
    invalid.profile_id = Some("test-provider".into());
    assert!(read(temp.path(), &path, invalid).is_err());
}
