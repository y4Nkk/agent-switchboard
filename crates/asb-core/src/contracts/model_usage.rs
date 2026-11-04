use serde::{Deserialize, Serialize};
use super::AppKind;

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

/// Whether a local-session usage read came from its persisted snapshot or a
/// completed refresh task. It says nothing about provider
/// billing, quota, or remote account state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelUsageFreshness {
    Cached,
    Fresh,
}

/// One coherent set of calendar reports plus the backend-owned timing. The
/// renderer uses `refresh_after` to schedule foreground-only revalidation;
/// it never invents an independent cache lifetime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageRead {
    pub reports: Vec<ModelUsageReport>,
    pub freshness: ModelUsageFreshness,
    pub refresh_after: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelUsagePhase { Discovering, Checking, Reading, Verifying, Aggregating, Publishing }

/// Counters describe completed work, never a time-based estimate. Byte counters
/// cover the current source operation; source counters cover the whole batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageProgress {
    pub phase: ModelUsagePhase,
    pub completed_sources: u64,
    pub total_sources: Option<u64>,
    pub completed_bytes: u64,
    pub total_bytes: Option<u64>,
    pub completed_calls: u64,
    pub total_calls: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ModelUsageRefresh {
    Running { id: String, started_at: String, progress: ModelUsageProgress },
    Completed { id: String, started_at: String, read: ModelUsageRead },
    Failed { id: String, started_at: String, message: String },
}
