use super::{TokenTotals, UsageEvent};
use asb_core::contracts::AppKind;
use chrono::{DateTime, Local, NaiveDate};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct ClaudeCandidate {
    pub event: UsageEvent,
    stopped: bool,
}

pub(super) fn observe(value: &Value, session: &str, candidates: &mut HashMap<String, ClaudeCandidate>) -> Result<(), String> {
    if value.get("type").and_then(Value::as_str) != Some("assistant") { return Ok(()); }
    let Some(message) = value.get("message").filter(|message| message.get("usage").is_some()) else { return Ok(()); };
    merge(candidates, candidate(message, value, session)?);
    Ok(())
}

pub(super) fn merge(candidates: &mut HashMap<String, ClaudeCandidate>, mut candidate: ClaudeCandidate) {
    let key = candidate.event.identity.clone();
    if let Some(current) = candidates.get(&key) { candidate.event.session = current.event.session.clone(); }
    let preferred = candidates.get(&key).is_none_or(|current|
        (candidate.stopped, candidate.event.tokens.output) > (current.stopped, current.event.tokens.output));
    if preferred { candidates.insert(key, candidate); }
}

fn candidate(message: &Value, envelope: &Value, session: &str) -> Result<ClaudeCandidate, String> {
    let identity = message.get("id").and_then(Value::as_str).filter(|id| !id.is_empty()).ok_or("缺少消息身份")?;
    let usage = message.get("usage").ok_or("缺少用量")?;
    let input = required(usage, "input_tokens")?;
    let output = required(usage, "output_tokens")?;
    let cache_read = optional(usage, "cache_read_input_tokens")?;
    let cache_creation = optional(usage, "cache_creation_input_tokens")?;
    let total = input.checked_add(output).and_then(|n| n.checked_add(cache_read))
        .and_then(|n| n.checked_add(cache_creation)).ok_or("用量计数溢出")?;
    Ok(ClaudeCandidate { stopped: message.get("stop_reason").and_then(Value::as_str).is_some_and(|s| !s.is_empty()),
        event: UsageEvent { app: AppKind::Claude, identity: identity.to_string(), session: session.to_string(),
            model: message.get("model").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string),
            at: envelope.get("timestamp").and_then(Value::as_str).map(str::to_string),
            tokens: TokenTotals { input, output, cache_read, cache_creation, total } } })
}

fn required(value: &Value, key: &str) -> Result<u64, String> {
    value.get(key).and_then(Value::as_u64).ok_or_else(|| format!("{key} 不是有效非负整数"))
}
fn optional(value: &Value, key: &str) -> Result<u64, String> {
    value.get(key).map_or(Ok(0), |_| required(value, key))
}
pub(super) fn local_date(timestamp: &str) -> Option<NaiveDate> {
    DateTime::parse_from_rfc3339(timestamp).ok().map(|at| at.with_timezone(&Local).date_naive())
}
