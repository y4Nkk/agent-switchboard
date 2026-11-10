use asb_core::contracts::AppKind;
use serde::{Deserialize, Serialize};
pub(crate) use crate::native_telemetry::{MeasurementScope, RequestOutcome, RequestUsageRow};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RequestUsageRange { Last15Minutes, LastHour, Last24Hours, Last7Days, Last30Days, All }

impl RequestUsageRange {
    pub(super) fn milliseconds(self) -> Option<u64> {
        let minutes = match self {
            Self::Last15Minutes => 15, Self::LastHour => 60, Self::Last24Hours => 1440,
            Self::Last7Days => 10080, Self::Last30Days => 43200, Self::All => return None,
        };
        Some(minutes * 60_000)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RequestUsageQuery {
    pub range: RequestUsageRange,
    pub app: Option<AppKind>,
    pub scope: MeasurementScope,
    pub model: Option<String>,
    pub page: usize,
    pub group_page: usize,
}

impl RequestUsageQuery {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.page == 0 || self.page > 1_000_000 || self.group_page == 0 || self.group_page > 1_000_000 {
            return Err("请求性能页码无效".into());
        }
        if self.model.as_ref().is_some_and(|text| text.trim().is_empty() || text.len() > 2048
            || text.chars().any(char::is_control)) { return Err("请求性能筛选值无效".into()); }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestUsageMetrics {
    pub records: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub unknown: u64,
    pub mixed_measurements: bool,
    pub success_ratio: Option<f64>,
    pub output_rate_p50: Option<f64>,
    pub output_rate_p10: Option<f64>,
    pub speed_samples: usize,
    pub first_token_p50_ms: Option<f64>,
    pub first_token_p95_ms: Option<f64>,
    pub first_token_samples: usize,
    pub first_content_p50_ms: Option<f64>,
    pub first_content_samples: usize,
    pub duration_p50_ms: Option<f64>,
    pub duration_p95_ms: Option<f64>,
    pub duration_samples: usize,
    pub cache_hit_rate: Option<f64>,
    pub cache_samples: u64,
    pub retry_records: u64,
    pub retries: u64,
    pub attempt_samples: u64,
    pub recovered_records: u64,
    pub rate_limited: u64,
    pub upstream_errors: u64,
    pub unclassified_failures: u64,
    pub estimated_cost_usd: Option<String>,
    pub cost_samples: u64,
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
    pub model: Option<String>,
    pub event_name: String,
    pub metrics: RequestUsageMetrics,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestUsageBucket {
    pub at_ms: u64,
    pub speed_p50: Option<f64>,
    pub first_token_p50_ms: Option<f64>,
    pub duration_p50_ms: Option<f64>,
    pub failure_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestUsageSource {
    pub app: AppKind,
    pub verified_records: u64,
    pub earliest_at_ms: Option<u64>,
    pub latest_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestUsageReport {
    pub generated_at_ms: u64,
    pub start_at_ms: u64,
    pub end_at_ms: u64,
    pub bucket_ms: u64,
    pub scope: MeasurementScope,
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
