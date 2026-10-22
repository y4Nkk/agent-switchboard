use serde::{Deserialize, Serialize};

use crate::contracts::AppKind;

/// One explicit usage-balance query mode owned by a profile. Declarative
/// queries use the application's fixed GET/auth/JSON-Pointer behavior;
/// script queries provide the two constrained JavaScript functions evaluated
/// by the desktop runtime. Both modes feed the same business normalization.
/// The tag is the only persisted discriminator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UsageQuery {
    /// Request URL; the `{{baseUrl}}` and `{{apiKey}}` placeholders are
    /// substituted at run time. The desktop runtime sends a GET with its
    /// established dual-ecosystem authorization headers.
    Declarative {
        url: String,
        /// JSON Pointer (RFC 6901) into the response body, e.g.
        /// `data/balance`.
        #[serde(skip_serializing_if = "Option::is_none")]
        remaining_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        used_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        total_path: Option<String>,
        /// Display unit for the extracted numbers, e.g. `USD`.
        #[serde(skip_serializing_if = "Option::is_none")]
        unit: Option<String>,
        /// Minutes between the desktop scheduler's automatic re-queries of
        /// this profile; 0 keeps the profile manual-only. This field is
        /// required: files that omit it are rejected without a default,
        /// conversion, or rewrite.
        refresh_interval_minutes: u32,
    },
    /// JavaScript source that evaluates to `{ request(input), extract(input) }`.
    /// It is compiled and executed only by the constrained desktop runtime.
    /// Extracted facts are normalized separately from query success.
    Script {
        source: String,
        /// Minutes between the desktop scheduler's automatic re-queries of
        /// this profile; 0 keeps the profile manual-only. This field is
        /// required: files that omit it are rejected without a default,
        /// conversion, or rewrite.
        refresh_interval_minutes: u32,
    },
}

impl UsageQuery {
    /// The auto-refresh cadence is mode-independent, so both variants carry
    /// the same field and expose it through this one accessor. The desktop
    /// scheduler is its only consumer.
    pub fn refresh_interval_minutes(&self) -> u32 {
        match self {
            UsageQuery::Declarative {
                refresh_interval_minutes,
                ..
            }
            | UsageQuery::Script {
                refresh_interval_minutes,
                ..
            } => *refresh_interval_minutes,
        }
    }
}

/// Facts returned by extraction, before business availability is determined.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageReadingInput {
    pub plan_name: Option<String>,
    pub remaining: Option<f64>,
    pub used: Option<f64>,
    pub total: Option<f64>,
    pub unit: Option<String>,
    pub resets_at: Option<String>,
    pub is_valid: Option<bool>,
    pub invalid_message: Option<String>,
    pub extra: Option<String>,
}

/// Availability is a successful reading's business state, never a query error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum UsageAvailability {
    Available,
    Exhausted,
    Unavailable {
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    Unknown,
}

/// Normalized reading consumed by the cache, history, and renderer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageReading {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_name: Option<String>,
    pub remaining: Option<f64>,
    pub used: Option<f64>,
    pub total: Option<f64>,
    pub unit: Option<String>,
    /// Provider-reported reset time, normalized to RFC 3339 by the extractor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<String>,
    pub availability: UsageAvailability,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<String>,
}

/// Complete result of one usage-query response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub readings: Vec<UsageReading>,
    /// RFC 3339 UTC timestamp of the query.
    pub at: String,
}

/// Last completed query and its retained reading; shared by rows and tray.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageSnapshot {
    pub summary: Option<UsageSummary>,
    pub error: Option<String>,
}

/// The selectable time span for a read-only aggregation of local client
/// session records. The range is evaluated in the local machine's calendar;
/// it does not describe a provider billing window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelUsageRange {
    Today,
    Last7Days,
    Last30Days,
    All,
}

/// Token consumption observed in local client session records, grouped by
/// client and model. `total_tokens` is the sum of fresh input, cache-read
/// input, cache-creation input, and output. This is intentionally separate
/// from a provider balance or a subscription quota: local logs cannot
/// establish remaining allowance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageGroup {
    pub app: AppKind,
    pub model: Option<String>,
    pub input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub session_count: u64,
}

/// Token consumption assigned to one local calendar day. The date uses the
/// local machine's `YYYY-MM-DD` calendar, matching `ModelUsageRange`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageDay {
    pub date: String,
    pub input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

/// A token subtotal that cannot be assigned to a local calendar day because
/// its source record has no usable timestamp. It is separate from the daily
/// trend so the report never silently represents it as dated usage.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageTokens {
    pub input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

/// One client-local reason why a model-usage report may be incomplete. The
/// report still includes any independently readable session records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageIssue {
    pub app: AppKind,
    pub message: String,
}

/// A credential-free, read-only aggregation over local Codex and Claude Code
/// session records. It never represents a provider billing or quota value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageReport {
    pub range: ModelUsageRange,
    /// RFC 3339 UTC timestamp at which the report was generated.
    pub generated_at: String,
    pub groups: Vec<ModelUsageGroup>,
    /// Daily local-calendar totals for records with usable timestamps.
    pub days: Vec<ModelUsageDay>,
    /// Totals included in `groups` but intentionally absent from `days`.
    /// This can be non-zero only for the `All` range, because date-bounded
    /// ranges cannot decide whether an undated record belongs inside them.
    pub unassigned_tokens: ModelUsageTokens,
    pub issues: Vec<ModelUsageIssue>,
    pub coverage: ModelUsageCoverage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageCoverage {
    pub source_sessions: u64,
    pub scanned_files: u64,
    /// Includes sessions with no completed calls and histories missing call receipts.
    pub sessions_without_usage: u64,
    pub undated_event_tokens: u64,
}

/// One read request for the local-session usage cache. A forced refresh
/// bypasses the aggregate snapshot and revalidates approved source revisions;
/// unchanged call records remain indexed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelUsageRequest {
    pub range: ModelUsageRange,
    pub force_refresh: bool,
}

/// Whether a local-session usage read came from its persisted snapshot or a
/// scan completed in the current request. It says nothing about provider
/// billing, quota, or remote account state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelUsageFreshness {
    Cached,
    Fresh,
}

/// A local-session report plus the backend-owned snapshot timing. The
/// renderer uses `refresh_after` to schedule foreground-only revalidation;
/// it never invents an independent cache lifetime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageRead {
    pub report: ModelUsageReport,
    pub freshness: ModelUsageFreshness,
    pub refresh_after: String,
}

/// One renderer request for persisted, credential-free usage history.
/// Provider history is always resolved against the profile's current query
/// digest by the backend; the renderer never supplies a digest or path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UsageHistoryRequest {
    Provider { profile_id: String },
    Official,
}

/// The meaning of one stored usage-history series. `UsedPercent` is reserved
/// for normalized official quota windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UsageHistoryMetric {
    Remaining,
    Used,
    UsedPercent,
}

/// One observed numeric point. Timestamps are RFC 3339 UTC values written
/// only after a real successful provider or official-quota read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageHistoryPoint {
    pub at: String,
    pub value: f64,
}

/// A renderer-safe history series. It intentionally contains no provider
/// endpoint, query source, account identifier, credential, or raw payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageHistorySeries {
    pub total_points: u64,
    pub first_at: String,
    pub last_at: String,
    pub id: String,
    pub label: String,
    pub unit: Option<String>,
    pub metric: UsageHistoryMetric,
    pub points: Vec<UsageHistoryPoint>,
}

/// The renderer-safe status of one Codex official-subscription quota read.
/// OAuth credentials and account identifiers are intentionally absent from
/// this contract; the desktop service owns them for the duration of a single
/// request only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexOfficialQuotaStatus {
    Available,
    SignInRequired,
    ReauthenticationRequired,
    Unavailable,
}

/// One server-declared Codex subscription limit window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexOfficialQuotaWindow {
    /// A user-facing name derived from the server's window duration.
    pub label: String,
    /// Percentage already consumed in the current window, from 0 to 100.
    pub used_percent: f64,
    /// RFC 3339 UTC reset time when supplied by the server.
    pub resets_at: Option<String>,
}

/// How a locally detected Codex quota reset relates to the previously
/// declared window schedule. `Scheduled` means the previous reset time had
/// already passed; `Early` means usage dropped while it was still ahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexOfficialQuotaResetKind {
    Scheduled,
    Early,
}

/// One locally detected Codex quota reset, observed by comparing consecutive
/// successful official reads of the 7-day window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexOfficialQuotaReset {
    /// RFC 3339 UTC timestamp of the read that first saw the new window.
    pub observed_at: String,
    pub kind: CodexOfficialQuotaResetKind,
    /// The new window's server-declared reset time, when supplied.
    pub resets_at: Option<String>,
}

/// The class of one outbound transport failure, shared by the quota read and
/// the endpoint probe surfaces. The classification owner; user-visible text
/// lives in the renderer's message catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexTransportClass {
    Dns,
    Timeout,
    Connect,
    Tls,
    Other,
}

/// Which stage of one official-quota read failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexQuotaReadFailureKind {
    /// The request never received an HTTP answer.
    Transport,
    /// The server answered outside the accepted status range.
    HttpStatus,
    /// A 2xx body did not parse into any quota window.
    InvalidResponse,
}

/// Why one official-quota read failed after an attempt was made. Present
/// exactly on `Unavailable` and `ReauthenticationRequired` reads; `Available`
/// succeeded and `SignInRequired` never attempted, so both carry none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexQuotaReadFailure {
    pub kind: CodexQuotaReadFailureKind,
    /// Set only when `kind` is `Transport`.
    pub transport_class: Option<CodexTransportClass>,
    /// Set only when `kind` is `HttpStatus`.
    pub http_status: Option<u16>,
}

impl CodexQuotaReadFailure {
    pub fn transport(class: CodexTransportClass) -> Self {
        Self { kind: CodexQuotaReadFailureKind::Transport, transport_class: Some(class), http_status: None }
    }

    pub fn http_status(status: u16) -> Self {
        Self { kind: CodexQuotaReadFailureKind::HttpStatus, transport_class: None, http_status: Some(status) }
    }

    pub const INVALID_RESPONSE: Self = Self {
        kind: CodexQuotaReadFailureKind::InvalidResponse,
        transport_class: None,
        http_status: None,
    };
}

/// A normalized read of the existing Codex ChatGPT-login quota. On a failed
/// refresh, `windows` can retain the most recent in-process successful read
/// and `stale` makes that fact explicit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexOfficialQuota {
    pub status: CodexOfficialQuotaStatus,
    pub windows: Vec<CodexOfficialQuotaWindow>,
    /// RFC 3339 UTC timestamp of the latest successful service response.
    pub at: Option<String>,
    pub stale: bool,
    /// The most recent locally detected reset known when this read was
    /// recorded. Absent on reads that never ran the comparison.
    pub last_reset: Option<CodexOfficialQuotaReset>,
    /// Why this read failed, present exactly on `Unavailable` and
    /// `ReauthenticationRequired`.
    pub failure: Option<CodexQuotaReadFailure>,
}
