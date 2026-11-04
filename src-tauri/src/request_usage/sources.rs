use super::contracts::*;
use crate::codex_metering::CodexRequestLedger;
use crate::gateway::request_ledger::{ClaudeRequestLedger, MAX_ENTRIES};
use asb_core::contracts::AppKind;
use std::path::Path;

pub(super) fn visit(
    root: &Path, claude_path: &Path, app: AppKind, start: u64, end: u64,
    mut consume: impl FnMut(RequestUsageRow),
) -> RequestUsageSource {
    let mut source = RequestUsageSource { app, verified_requests: 0, earliest_at_ms: None,
        latest_at_ms: None, retention_limit: if app == AppKind::Claude { Some(MAX_ENTRIES) } else { None },
        error: None };
    let mut invalid_times = 0_usize;
    let mut accept = |mut row: RequestUsageRow| {
        // Session receipts and auxiliary/unauthenticated gateway operations do not prove model requests.
        if row.at_ms < start || row.at_ms > end || row.status == Some(101)
            || (row.model.is_none() && row.output_tokens.is_none())
            || row.profile_id.is_empty() { return; }
        normalize(&mut row);
        source.verified_requests += 1;
        source.earliest_at_ms = Some(source.earliest_at_ms.unwrap_or(row.at_ms).min(row.at_ms));
        source.latest_at_ms = Some(source.latest_at_ms.unwrap_or(row.at_ms).max(row.at_ms));
        consume(row);
    };
    let result = match app {
        AppKind::Codex => CodexRequestLedger::new(root).visit_gateway_requests(start, end, |r| {
            let (model, model_source) = model(&r.response_model, &r.mapped_model, &r.request_model);
            accept(RequestUsageRow { id: r.id, at_ms: r.at_ms, app, profile_id: r.profile_id.unwrap_or_default(),
                model, model_source, request_model: r.request_model, mapped_model: r.mapped_model,
                response_model: r.response_model, upstream_protocol: r.upstream_protocol,
                outcome: outcome(r.status), status: r.status, duration_ms: r.duration_ms,
                first_output_ms: r.first_token_latency_ms, output_tokens_per_second: None,
                input_tokens: r.input_tokens, output_tokens: r.output_tokens, cache_read_tokens: r.cache_read_tokens,
                cache_creation_tokens: r.cache_creation_tokens, reasoning_tokens: r.reasoning_tokens,
                estimated_cost_usd: r.cost.map(|cost| cost.total_usd), pricing_error: r.pricing_error,
                attempts: r.attempts.into_iter().map(|a| RequestUsageAttempt {
                    profile_id: a.profile_id, status: a.status }).collect() });
        }),
        AppKind::Claude => ClaudeRequestLedger::new(claude_path.to_path_buf()).visit_requests(|index, r| {
            let (model, model_source) = model(&r.response_model, &r.mapped_model, &r.request_model);
            let at_ms = chrono::DateTime::parse_from_rfc3339(&r.at)
                .ok().and_then(|at| u64::try_from(at.timestamp_millis()).ok());
            // Claude persists completion time; Codex persists start time. Project both onto request start.
            let Some(at_ms) = at_ms.and_then(|finished| finished.checked_sub(r.duration_ms)) else {
                invalid_times += 1;
                return;
            };
            accept(RequestUsageRow { id: format!("claude:{at_ms}:{index}"), at_ms, app,
                profile_id: r.profile_id.unwrap_or_default(), model, model_source,
                request_model: r.request_model, mapped_model: r.mapped_model, response_model: r.response_model,
                upstream_protocol: r.upstream_protocol, outcome: outcome(r.status), status: r.status,
                duration_ms: r.duration_ms, first_output_ms: r.first_token_latency_ms,
                output_tokens_per_second: None, input_tokens: r.input_tokens, output_tokens: r.output_tokens,
                cache_read_tokens: r.cache_read_tokens, cache_creation_tokens: r.cache_creation_tokens,
                reasoning_tokens: r.reasoning_tokens, estimated_cost_usd: r.cost.map(|cost| cost.total_usd),
                pricing_error: r.pricing_error, attempts: r.failover_attempts.into_iter().map(|a| {
                    RequestUsageAttempt { profile_id: a.profile_id, status: a.status } }).collect() });
        }),
    };
    source.error = result.err().map(asb_core::adapter::scrub_message).or_else(|| {
        (invalid_times > 0).then(|| format!("{invalid_times} 条 Claude 请求记录的时间与耗时无效，未纳入性能统计"))
    });
    source
}

fn model(response: &Option<String>, mapped: &Option<String>, request: &Option<String>)
    -> (Option<String>, RequestModelSource)
{
    for (value, source) in [(response, RequestModelSource::Response),
        (mapped, RequestModelSource::Mapped), (request, RequestModelSource::Request)]
    {
        if let Some(value) = value.as_ref().filter(|value| !value.trim().is_empty()) {
            return (Some(value.clone()), source);
        }
    }
    (None, RequestModelSource::Unknown)
}

fn outcome(status: Option<u16>) -> RequestOutcome {
    match status {
        Some(206) => RequestOutcome::Failed,
        Some(200..=299) => RequestOutcome::Completed,
        Some(499) => RequestOutcome::Disconnected,
        _ => RequestOutcome::Failed,
    }
}

fn normalize(row: &mut RequestUsageRow) {
    row.first_output_ms = row.first_output_ms.filter(|first| *first <= row.duration_ms);
    if row.outcome == RequestOutcome::Completed && row.duration_ms > 0 {
        row.output_tokens_per_second = row.output_tokens.filter(|tokens| *tokens > 0)
            .map(|tokens| tokens as f64 * 1000.0 / row.duration_ms as f64);
    }
}
