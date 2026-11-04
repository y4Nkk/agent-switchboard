//! Durable, credential-free request history for Claude gateway traffic.
//!
//! This ledger is intentionally separate from both provider-balance history
//! and in-memory gateway metrics. It stores only bounded, typed metadata that
//! can be shown or aggregated after a restart; request bodies, URLs, headers,
//! credentials, and provider response text never cross this module's API.


mod spend;
use crate::config_store::write_json_atomic;
use asb_core::contracts::UpstreamProtocol;
use chrono::{DateTime, FixedOffset, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub(crate) const LEDGER_VERSION: u8 = 2;
pub(crate) const MAX_ENTRIES: usize = 10_000;
const MAX_FAILOVER_ATTEMPTS: usize = 32;
const MAX_TEXT_LENGTH: usize = 512;

static LEDGER_MUTATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn mutation_lock() -> &'static Mutex<()> {
    LEDGER_MUTATION_LOCK.get_or_init(|| Mutex::new(()))
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClaudeRequestLedgerFile {
    pub(crate) version: u8,
    #[serde(default)]
    pub(crate) entries: Vec<ClaudeRequestRecord>,
    pub(crate) spend_by_day_utc: spend::DailySpend,
}

/// Process-local handle to the Claude request ledger. Multiple gateway
/// controllers can point at different test directories; the mutation lock is
/// shared so two handles can never interleave an atomic read-modify-write.
pub(crate) struct ClaudeRequestLedger {
    path: PathBuf,
}

impl ClaudeRequestLedger {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub(crate) fn visit_requests(
        &self,
        mut visit: impl FnMut(usize, ClaudeRequestRecord),
    ) -> Result<(), String> {
        let _guard = mutation_lock().lock().map_err(|_| "Claude 请求账本读取锁不可用")?;
        if let Some(ledger) = self.load()? {
            for (index, record) in ledger.entries.into_iter().enumerate() { visit(index, record); }
        }
        Ok(())
    }

    pub(crate) fn append(&self, mut record: ClaudeRequestRecord) -> Result<(), String> {
        validate_record(&record)?;
        record.at = DateTime::<FixedOffset>::parse_from_rfc3339(&record.at)
            .map_err(|e| e.to_string())?
            .with_timezone(&Utc)
            .to_rfc3339_opts(SecondsFormat::Millis, true);
        let _guard = mutation_lock()
            .lock()
            .map_err(|_| "Claude 请求账本写入锁不可用".to_string())?;
        let mut ledger = self.load()?.unwrap_or_else(empty_ledger);
        spend::add(&mut ledger.spend_by_day_utc, &record)?;
        ledger.entries.push(record);
        prune(&mut ledger.entries);
        self.save(&ledger)
    }


    /// Local-calendar spend guard for daily/monthly reference limits; the
    /// pre-request check lives on the ledger because it owns the spend index.
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
        let ledger = self.load()?.unwrap_or_else(empty_ledger);
        let today = chrono::Utc::now().date_naive().to_string();
        let (daily, monthly) = spend::totals(&ledger.spend_by_day_utc, profile, &today);
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
    fn load(&self) -> Result<Option<ClaudeRequestLedgerFile>, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("Claude 请求账本不可读：{error}")),
        };
        let mut ledger = spend::decode(&text)?;
        validate_ledger(&ledger)?;
        ledger.entries.sort_by_key(|entry| {
            DateTime::<FixedOffset>::parse_from_rfc3339(&entry.at).expect("validated ledger time")
        });
        Ok(Some(ledger))
    }

    fn save(&self, ledger: &ClaudeRequestLedgerFile) -> Result<(), String> {
        let text = serde_json::to_string_pretty(ledger)
            .map_err(|_| "Claude 请求账本序列化失败".to_string())?;
        write_json_atomic(&self.path, &text)
            .map_err(|error| format!("无法保存 Claude 请求账本：{error}"))
    }
}

pub(crate) fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn empty_ledger() -> ClaudeRequestLedgerFile {
    ClaudeRequestLedgerFile {
        version: LEDGER_VERSION,
        entries: Vec::new(),
        spend_by_day_utc: Default::default(),
    }
}

fn validate_ledger(ledger: &ClaudeRequestLedgerFile) -> Result<(), String> {
    spend::validate(&ledger.spend_by_day_utc)?;
    if ledger.version != LEDGER_VERSION {
        return Err("Claude 请求账本版本不受支持；请移除旧账本后重试".to_string());
    }
    if ledger.entries.len() > MAX_ENTRIES {
        return Err("Claude 请求账本条目数量超过限制".to_string());
    }
    for entry in &ledger.entries {
        validate_record(entry)?;
    }
    Ok(())
}

fn validate_record(entry: &ClaudeRequestRecord) -> Result<(), String> {
    DateTime::<FixedOffset>::parse_from_rfc3339(&entry.at)
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
    Ok(())
}

fn validate_text(value: &str) -> Result<(), String> {
    if valid_text(value).is_none() {
        Err("Claude 请求账本文本字段无效".to_string())
    } else {
        Ok(())
    }
}

fn valid_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()
        && value.chars().count() <= MAX_TEXT_LENGTH
        && !value.chars().any(char::is_control))
    .then(|| value.to_string())
}

fn prune(entries: &mut Vec<ClaudeRequestRecord>) {
    entries.sort_by(|left, right| left.at.cmp(&right.at));
    if entries.len() > MAX_ENTRIES {
        let keep_from = entries.len() - MAX_ENTRIES;
        entries.drain(..keep_from);
    }
}
