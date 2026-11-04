use asb_core::contracts::{AppKind, UpstreamProtocol};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RequestUsageRange {
    Last15Minutes,
    LastHour,
    Last24Hours,
    Last7Days,
    Last30Days,
    All,
}

impl RequestUsageRange {
    pub(super) fn milliseconds(self) -> Option<u64> {
        let minutes = match self {
            Self::Last15Minutes => 15,
            Self::LastHour => 60,
            Self::Last24Hours => 1440,
            Self::Last7Days => 10080,
            Self::Last30Days => 43200,
            Self::All => return None,
        };
        Some(minutes * 60_000)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RequestUsageQuery {
    pub range: RequestUsageRange,
    pub app: Option<AppKind>,
    pub profile_id: Option<String>,
    pub model: Option<String>,
    pub page: usize,
    pub group_page: usize,
}

impl RequestUsageQuery {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.profile_id.is_some() && self.app.is_none() {
            return Err("按供应商筛选请求性能时必须同时指定客户端".into());
        }
        if self.page == 0 || self.page > 1_000_000 || self.group_page == 0
            || self.group_page > 1_000_000
        {
            return Err("请求性能页码无效".into());
        }
        for text in [&self.profile_id, &self.model].into_iter().flatten() {
            if text.trim().is_empty() || text.len() > 2048 || text.chars().any(char::is_control) {
                return Err("请求性能筛选值无效".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RequestOutcome { Completed, Failed, Disconnected }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RequestModelSource { Response, Mapped, Request, Unknown }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestUsageRow {
    pub id: String,
    pub at_ms: u64,
    pub app: AppKind,
    pub profile_id: String,
    pub model: Option<String>,
    pub model_source: RequestModelSource,
    pub request_model: Option<String>,
    pub mapped_model: Option<String>,
    pub response_model: Option<String>,
    pub upstream_protocol: Option<UpstreamProtocol>,
    pub outcome: RequestOutcome,
    pub status: Option<u16>,
    pub duration_ms: u64,
    pub first_output_ms: Option<u64>,
    pub output_tokens_per_second: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub estimated_cost_usd: Option<String>,
    pub pricing_error: Option<String>,
    pub attempts: Vec<RequestUsageAttempt>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestUsageAttempt {
    pub profile_id: String,
    pub status: Option<u16>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestUsageMetrics {
    pub requests: u64,
    pub completed: u64,
    pub failed: u64,
    pub disconnected: u64,
    pub incomplete: u64,
    pub completion_rate: Option<f64>,
    pub output_rate_p50: Option<f64>,
    pub output_rate_p10: Option<f64>,
    pub speed_samples: usize,
    pub first_output_p50_ms: Option<f64>,
    pub first_output_p95_ms: Option<f64>,
    pub first_output_samples: usize,
    pub duration_p50_ms: Option<f64>,
    pub duration_p95_ms: Option<f64>,
    pub duration_samples: usize,
    pub cache_hit_rate: Option<f64>,
    pub cache_samples: u64,
    pub retry_requests: u64,
    pub retries: u64,
    pub attempt_samples: u64,
    pub recovered_requests: u64,
    pub rate_limited: u64,
    pub upstream_errors: u64,
    pub unclassified_failures: u64,
    pub estimated_cost_usd: Option<String>,
    pub cost_samples: u64,
    pub pricing_errors: u64,
    pub tokens: Vec<RequestUsageTokenTotal>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestUsageTokenTotal {
    pub kind: &'static str,
    pub tokens: Option<u64>,
    pub samples: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestUsageGroup {
    pub app: AppKind,
    pub profile_id: String,
    pub model: Option<String>,
    pub model_source: RequestModelSource,
    pub metrics: RequestUsageMetrics,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestUsageBucket {
    pub at_ms: u64,
    pub requests: u64,
    pub speed_p50: Option<f64>,
    pub first_output_p50_ms: Option<f64>,
    pub failure_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestUsageSource {
    pub app: AppKind,
    pub verified_requests: u64,
    pub earliest_at_ms: Option<u64>,
    pub latest_at_ms: Option<u64>,
    pub retention_limit: Option<usize>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestUsageReport {
    pub generated_at_ms: u64,
    pub start_at_ms: u64,
    pub end_at_ms: u64,
    pub bucket_ms: u64,
    pub summary: RequestUsageMetrics,
    pub sources: Vec<RequestUsageSource>,
    pub groups: Vec<RequestUsageGroup>,
    pub total_groups: usize,
    pub group_page: usize,
    pub group_page_size: usize,
    pub trend: Vec<RequestUsageBucket>,
    pub rows: Vec<RequestUsageRow>,
    pub page: usize,
    pub page_size: usize,
}
