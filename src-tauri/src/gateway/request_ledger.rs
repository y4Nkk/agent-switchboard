//! Durable, credential-free request history for Claude gateway traffic.
//!
//! This ledger is intentionally separate from both provider-balance history
//! and in-memory gateway metrics. It stores only bounded, typed metadata that
//! can be shown or aggregated after a restart; request bodies, URLs, headers,
//! credentials, and provider response text never cross this module's API.


mod migration;
mod store;
#[cfg(test)]
mod tests;

use asb_core::contracts::UpstreamProtocol;
use chrono::{DateTime, FixedOffset, SecondsFormat, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex};

const MAX_ENTRIES: usize = 10_000;
const MAX_SPEND_DAYS: usize = 400;
const MAX_FAILOVER_ATTEMPTS: usize = 32;
const MAX_TEXT_LENGTH: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ClaudeFailoverOutcome {
    Success,
    RetryableFailure,
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClaudeFailoverAttempt {
    pub(crate) profile_id: String,
    pub(crate) route_revision: String,
    pub(crate) upstream_protocol: UpstreamProtocol,
    pub(crate) status: Option<u16>,
    pub(crate) outcome: ClaudeFailoverOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClaudeRequestRecord {
    pub(crate) at: String,
    pub(crate) profile_id: Option<String>,
    pub(crate) route_revision: Option<String>,
    pub(crate) client_protocol: UpstreamProtocol,
    pub(crate) upstream_protocol: Option<UpstreamProtocol>,
    pub(crate) request_model: Option<String>,
    pub(crate) mapped_model: Option<String>,
    pub(crate) response_model: Option<String>,
    pub(crate) cost: Option<ClaudeRequestCost>,
    pub(crate) pricing_error: Option<String>,
    pub(crate) first_token_latency_ms: Option<u64>,
    pub(crate) input_tokens: Option<u64>,
    pub(crate) output_tokens: Option<u64>,
    pub(crate) cache_read_tokens: Option<u64>,
    pub(crate) cache_creation_tokens: Option<u64>,
    pub(crate) reasoning_tokens: Option<u64>,
    pub(crate) status: Option<u16>,
    pub(crate) duration_ms: u64,
    pub(crate) first_byte_latency_ms: Option<u64>,
    #[serde(default)]
    pub(crate) failover_attempts: Vec<ClaudeFailoverAttempt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClaudeRequestCost {
    pub(crate) model: String,
    pub(crate) source: String,
    pub(crate) multiplier: String,
    pub(crate) total_usd: String,
}

pub(crate) struct ClaudeRequestLedger {
    root: PathBuf,
    connection: Mutex<Option<Connection>>,
}

impl ClaudeRequestLedger {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root, connection: Mutex::new(None) }
    }

    pub(crate) fn append(&self, mut record: ClaudeRequestRecord) -> Result<(), String> {
        let at = validate_record(&record)?.with_timezone(&Utc);
        record.at = at.to_rfc3339_opts(SecondsFormat::Millis, true);
        self.with_connection(|connection| store::append(connection, &record, at))
    }

    pub(crate) fn budget_exceeded(
        &self,
        profile: &str,
        billing: Option<&asb_core::contracts::ClaudeBilling>,
    ) -> Result<Option<String>, String> {
        use asb_core::contracts::decimal_micros;
        let Some(billing) = billing.filter(|billing| {
            billing.daily_limit_usd.is_some() || billing.monthly_limit_usd.is_some()
        }) else {
            return Ok(None);
        };
        let today = Utc::now().date_naive();
        let (daily, monthly) = self.with_connection(|connection| store::totals(connection, profile, today))?;
        for (name, spent, limit) in [
            ("当日", daily, &billing.daily_limit_usd),
            ("当月", monthly, &billing.monthly_limit_usd),
        ] {
            if let Some(limit) = limit {
                if spent >= decimal_micros(limit)? {
                    return Ok(Some(format!(
                        "Claude 供应商 UTC {name}参考费用已达到本地限额 {limit} USD"
                    )));
                }
            }
        }
        Ok(None)
    }
    fn with_connection<T>(&self, run: impl FnOnce(&mut Connection) -> Result<T, String>) -> Result<T, String> {
        let mut slot = self.connection.lock().map_err(|_| "Claude 请求账本锁不可用")?;
        if slot.is_none() { *slot = Some(store::open(&self.root)?); }
        run(slot.as_mut().expect("opened Claude request ledger"))
    }
}

pub(crate) fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn validate_record(entry: &ClaudeRequestRecord) -> Result<DateTime<FixedOffset>, String> {
    let at = DateTime::<FixedOffset>::parse_from_rfc3339(&entry.at)
        .map_err(|_| "Claude 请求账本时间无效".to_string())?;
    if let Some(cost) = &entry.cost {
        asb_core::contracts::decimal_micros(&cost.total_usd)?;
        asb_core::contracts::decimal_micros(&cost.multiplier)?;
        validate_text(&cost.model)?;
        validate_text(&cost.source)?;
    }
    for value in [
        entry.profile_id.as_deref(),
        entry.route_revision.as_deref(),
        entry.request_model.as_deref(),
        entry.mapped_model.as_deref(),
        entry.response_model.as_deref(),
        entry.pricing_error.as_deref(),
    ] {
        if let Some(value) = value {
            validate_text(value)?;
        }
    }
    if let Some(status) = entry.status {
        if !(100..=599).contains(&status) {
            return Err("Claude 请求账本 HTTP 状态无效".to_string());
        }
    }
    if entry.failover_attempts.len() > MAX_FAILOVER_ATTEMPTS {
        return Err("Claude 请求账本故障转移摘要过长".to_string());
    }
    for attempt in &entry.failover_attempts {
        validate_text(&attempt.profile_id)?;
        validate_text(&attempt.route_revision)?;
        if let Some(status) = attempt.status {
            if !(100..=599).contains(&status) {
                return Err("Claude 请求账本故障转移 HTTP 状态无效".to_string());
            }
        }
    }
    Ok(at)
}

fn validate_text(value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > MAX_TEXT_LENGTH || value.chars().any(char::is_control) {
        return Err("Claude 请求账本文本字段无效".into());
    }
    Ok(())
}
