use super::{contracts::*, otlp::{self, Attributes}};
use asb_core::contracts::AppKind;
use serde_json::Value;

pub(super) struct Batch { pub rows: Vec<RequestUsageRow>, pub ignored: u64 }

pub(super) fn logs(app: AppKind, value: &Value) -> Result<Batch, String> {
    let mut batch = Batch { rows: Vec::new(), ignored: 0 };
    for resource in otlp::array(value, "resourceLogs")? {
        for scope in otlp::array(resource, "scopeLogs")? {
            for log in otlp::array(scope, "logRecords")? {
                let fields = otlp::attributes(log.get("attributes"))?;
                let name = otlp::text(&fields, "event.name").unwrap_or_default();
                let row = match app {
                    AppKind::Codex => codex_log(log, &fields, &name)?,
                    AppKind::Claude => claude_log(log, &fields, &name)?,
                };
                match row { Some(row) => batch.rows.push(row), None => batch.ignored += 1 }
            }
        }
    }
    Ok(batch)
}

pub(super) fn traces(value: &Value) -> Result<Batch, String> {
    let mut batch = Batch { rows: Vec::new(), ignored: 0 };
    for resource in otlp::array(value, "resourceSpans")? {
        for scope in otlp::array(resource, "scopeSpans")? {
            for span in otlp::array(scope, "spans")? {
                if span.get("name").and_then(Value::as_str) != Some("claude_code.llm_request") {
                    batch.ignored += 1; continue;
                }
                batch.rows.push(claude_span(span)?);
            }
        }
    }
    Ok(batch)
}

fn base(app: AppKind, scope: MeasurementScope, name: &str, fields: &Attributes) -> RequestUsageRow {
    RequestUsageRow { id: String::new(), app, scope, at_ms: 0, time_source: NativeTimeSource::Event, event_name: name.into(),
        event_kind: otlp::text(fields, "event.kind"),
        model: otlp::text(fields, "model"),
        session_id: otlp::text(fields, if app == AppKind::Codex { "conversation.id" } else { "session.id" }),
        request_id: otlp::text(fields, "request_id"), outcome: RequestOutcome::Unknown,
        status: otlp::integer(fields, "status_code").and_then(|n| u16::try_from(n).ok()),
        error_type: otlp::text(fields, if app == AppKind::Claude { "error_class" } else { "auth.error_code" }),
        duration_ms: None, first_token_ms: None, first_content_ms: None,
        output_tokens_per_second: None, input_tokens: None, output_tokens: None,
        cache_read_tokens: None, cache_creation_tokens: None, reasoning_tokens: None,
        estimated_cost_usd: None, attempt_count: None }
}

fn codex_log(log: &Value, fields: &Attributes, name: &str) -> Result<Option<RequestUsageRow>, String> {
    let generation = name == "codex.sse_event";
    let kind = otlp::text(fields, "event.kind");
    let has_error = fields.contains_key("error.message");
    let unsuccessful = matches!(kind.as_deref(), Some("response.failed" | "response.incomplete"));
    if generation && !(unsuccessful || has_error
        || (kind.as_deref() == Some("response.completed") && fields.contains_key("output_token_count"))) {
        return Ok(None);
    }
    if !generation && !matches!(name, "codex.api_request" | "codex.websocket_request") { return Ok(None); }
    let scope = if generation { MeasurementScope::Generation } else { MeasurementScope::Transport };
    let mut row = base(AppKind::Codex, scope, name, fields);
    row.status = otlp::integer(fields, "http.response.status_code").and_then(|n| u16::try_from(n).ok());
    row.request_id = otlp::text(fields, "auth.request_id");
    row.outcome = if has_error || unsuccessful {
        RequestOutcome::Failed
    } else if generation || otlp::boolean(fields, "success") == Some(true)
        || row.status.is_some_and(|status| (200..300).contains(&status)) {
        RequestOutcome::Succeeded
    } else if row.status.is_some_and(|status| status >= 400)
        || otlp::boolean(fields, "success") == Some(false) { RequestOutcome::Failed }
    else { RequestOutcome::Unknown };
    if generation {
        row.input_tokens = otlp::integer(fields, "input_token_count");
        row.output_tokens = otlp::integer(fields, "output_token_count");
        row.cache_read_tokens = otlp::integer(fields, "cached_token_count");
        row.cache_creation_tokens = otlp::integer(fields, "cache_write_token_count");
        row.reasoning_tokens = otlp::integer(fields, "reasoning_token_count");
        row.first_token_ms = otlp::number(fields, "ttft_ms");
    } else { row.duration_ms = otlp::number(fields, "duration_ms"); }
    finish_log(log, row).map(Some)
}

fn claude_log(log: &Value, fields: &Attributes, name: &str) -> Result<Option<RequestUsageRow>, String> {
    if !matches!(name, "api_request" | "api_error") { return Ok(None); }
    let mut row = base(AppKind::Claude, MeasurementScope::Transport, name, fields);
    row.outcome = if name == "api_request" { RequestOutcome::Succeeded } else { RequestOutcome::Failed };
    row.duration_ms = otlp::number(fields, "duration_ms");
    claude_tokens(&mut row, fields);
    if let Some(micros) = otlp::integer(fields, "cost_usd_micros") {
        row.estimated_cost_usd = Some(format!("{}.{:06}", micros / 1_000_000, micros % 1_000_000));
    }
    finish_log(log, row).map(Some)
}

fn claude_span(span: &Value) -> Result<RequestUsageRow, String> {
    let fields = otlp::attributes(span.get("attributes"))?;
    let start = otlp::nanos(span.get("startTimeUnixNano"))?;
    let end = otlp::nanos(span.get("endTimeUnixNano"))?;
    if end < start { return Err("OTLP span 结束时间早于开始时间".into()); }
    let trace = otlp::identity(span, "traceId", 32).ok_or("OTLP 请求 span 缺少有效 traceId")?;
    let id = otlp::identity(span, "spanId", 16).ok_or("OTLP 请求 span 缺少有效 spanId")?;
    let mut row = base(AppKind::Claude, MeasurementScope::Generation, "claude_code.llm_request", &fields);
    row.id = format!("claude:{trace}:{id}");
    row.at_ms = end / 1_000_000;
    row.time_source = NativeTimeSource::SpanEnd;
    row.duration_ms = otlp::number(&fields, "duration_ms");
    row.first_token_ms = otlp::number(&fields, "ttft_ms");
    row.first_content_ms = otlp::number(&fields, "first_content_ms");
    row.attempt_count = otlp::integer(&fields, "attempt").filter(|n| *n > 0);
    row.outcome = match otlp::boolean(&fields, "success") {
        Some(true) => RequestOutcome::Succeeded,
        Some(false) => RequestOutcome::Failed,
        None => RequestOutcome::Unknown,
    };
    claude_tokens(&mut row, &fields);
    row.normalize();
    row.validate()?;
    Ok(row)
}

fn claude_tokens(row: &mut RequestUsageRow, fields: &Attributes) {
    let input = otlp::integer(fields, "input_tokens");
    row.output_tokens = otlp::integer(fields, "output_tokens");
    row.cache_read_tokens = otlp::integer(fields, "cache_read_tokens");
    row.cache_creation_tokens = otlp::integer(fields, "cache_creation_tokens");
    // Claude reports fresh input separately; unknown cache counters cannot become zero.
    row.input_tokens = input.zip(row.cache_read_tokens).zip(row.cache_creation_tokens)
        .and_then(|((input, read), created)| input.checked_add(read)?.checked_add(created));
}

fn finish_log(log: &Value, mut row: RequestUsageRow) -> Result<RequestUsageRow, String> {
    let (at, source) = otlp::log_time(log)?;
    row.at_ms = at / 1_000_000;
    row.time_source = source;
    row.normalize();
    let content = serde_json::to_string(&(at, log.get("traceId"), log.get("spanId"), &row))
        .map_err(|_| "无法编码原生性能身份".to_string())?;
    row.id = asb_switch::sha256_hex(&content);
    row.validate()?;
    Ok(row)
}
