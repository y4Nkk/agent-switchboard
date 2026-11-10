use super::*;

fn row() -> RequestUsageRow {
    RequestUsageRow { id: "native-test-record".into(), app: AppKind::Claude,
        scope: MeasurementScope::Generation, at_ms: 1_700_000_000_000,
        time_source: crate::native_telemetry::NativeTimeSource::SpanEnd,
        event_name: "claude_code.llm_request".into(), event_kind: None, model: Some("test-model".into()),
        session_id: None, request_id: None, outcome: RequestOutcome::Succeeded,
        status: None, error_type: None, duration_ms: Some(1000.0), first_token_ms: Some(100.0), first_content_ms: Some(150.0),
        output_tokens_per_second: Some(10.0), input_tokens: Some(100), output_tokens: Some(10),
        cache_read_tokens: Some(10), cache_creation_tokens: None, reasoning_tokens: None,
        estimated_cost_usd: Some("0.000001".into()), attempt_count: None }
}

fn query() -> RequestUsageQuery {
    RequestUsageQuery { range: RequestUsageRange::All, app: None, scope: MeasurementScope::Generation,
        model: None, page: 1, group_page: 1 }
}

#[test]
fn native_sample_percentiles_and_cache_weights_are_independent() {
    let mut acc = Accumulator::default();
    acc.add(&row());
    let mut second = row();
    second.output_tokens_per_second = Some(100.0);
    second.output_tokens = Some(100);
    second.first_token_ms = Some(900.0);
    second.input_tokens = Some(900);
    second.cache_read_tokens = Some(810);
    acc.add(&second);
    let mut failure = row();
    failure.outcome = RequestOutcome::Failed;
    failure.status = Some(429);
    failure.output_tokens_per_second = None;
    acc.add(&failure);
    let mut unknown = row();
    unknown.outcome = RequestOutcome::Unknown;
    unknown.output_tokens_per_second = None;
    acc.add(&unknown);
    let result = acc.finish();
    assert_eq!(result.records, 4);
    assert_eq!(result.unknown, 1);
    assert_eq!(result.speed_samples, 2);
    assert_eq!(result.output_rate_p50, Some(55.0));
    assert_eq!(result.output_rate_p10, Some(19.0));
    assert_eq!(result.first_token_p50_ms, Some(500.0));
    assert_eq!(result.cache_hit_rate, Some(82.0));
    assert!((result.success_ratio.unwrap() - 200.0 / 3.0).abs() < 0.00001);
}

#[test]
fn missing_fields_do_not_create_zero_measurements() {
    let mut source = row();
    source.duration_ms = None;
    source.output_tokens_per_second = None;
    source.first_token_ms = None;
    source.first_content_ms = None;
    source.cache_read_tokens = None;
    source.estimated_cost_usd = None;
    let mut accumulator = Accumulator::default();
    accumulator.add(&source);
    let result = accumulator.finish();
    assert_eq!(result.speed_samples, 0);
    assert_eq!(result.duration_samples, 0);
    assert_eq!(result.first_token_samples, 0);
    assert_eq!(result.cache_hit_rate, None);
    assert_eq!(result.estimated_cost_usd, None);
    assert_eq!(result.attempt_samples, 0);
}

#[test]
fn different_native_measurement_boundaries_do_not_share_ratios_or_latencies() {
    let mut accumulator = Accumulator::default();
    accumulator.add(&row());
    let mut codex = row();
    codex.app = AppKind::Codex;
    codex.time_source = crate::native_telemetry::NativeTimeSource::Event;
    codex.event_name = "codex.sse_event".into();
    codex.duration_ms = None;
    codex.output_tokens_per_second = None;
    accumulator.add(&codex);
    let result = accumulator.finish();
    assert!(result.mixed_measurements);
    assert_eq!(result.success_ratio, None);
    assert_eq!(result.first_token_p50_ms, None);
    assert_eq!(result.output_rate_p50, Some(10.0));
    assert_eq!(result.speed_samples, 1);
}

#[test]
fn pagination_does_not_change_aggregation_and_groups_keep_native_operation() {
    let mut projection = Projection::new(None);
    for index in 0..32 {
        let mut record = row();
        record.id = format!("record-{index}");
        record.at_ms -= index;
        projection.add(record, &query(), 60_000);
    }
    assert_eq!(projection.matched, 32);
    assert_eq!(projection.rows.len(), PAGE_SIZE);
    assert_eq!(projection.tail.len(), PAGE_SIZE);
    assert_eq!(projection.groups.len(), 1);
    assert_eq!(projection.summary.finish().records, 32);
}

#[test]
fn obsolete_gateway_query_contract_is_rejected() {
    let previous = serde_json::json!({"range":"all", "app":null, "scope":"generation", "profileId":null,
        "model":null, "page":1, "groupPage":1});
    assert!(serde_json::from_value::<RequestUsageQuery>(previous).is_err());
    let mut current = query();
    current.page = 0;
    assert!(current.validate().is_err());
    current.page = 1;
    current.model = Some(" ".into());
    assert!(current.validate().is_err());
}
