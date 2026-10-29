use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct ForecastPayload {
    pub(super) updated_at: String,
    pub(super) probabilities: ForecastProbabilities,
    pub(super) confidence: String,
    pub(super) last_reset_at: String,
    pub(super) age_days: f64,
    #[serde(default)]
    pub(super) time_window: Option<ForecastWindowPayload>,
    #[serde(default)]
    pub(super) cadence: Option<ForecastCadencePayload>,
    #[serde(default)]
    pub(super) wait_comparison: Option<ForecastWaitPayload>,
    #[serde(default)]
    pub(super) context_copy: Option<ForecastContextCopyPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct ForecastProbabilities {
    pub(super) rounded_24h: f64,
    pub(super) rounded_48h: f64,
    pub(super) range_24h: ForecastRange,
    pub(super) range_48h: ForecastRange,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct ForecastRange {
    pub(super) lower: f64,
    pub(super) upper: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct ForecastWindowPayload {
    pub(super) start_hour: u32,
    pub(super) end_hour: u32,
    pub(super) timezone: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct ForecastCadencePayload {
    pub(super) recent_median_days: f64,
    pub(super) recent_sample: u32,
    pub(super) weighted_mean_days: f64,
    pub(super) accelerating: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct ForecastWaitPayload {
    pub(super) wait_days: f64,
    pub(super) sample: u32,
    pub(super) shorter_share: f64,
    pub(super) median_days: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct ForecastContextCopyPayload {
    #[serde(default)]
    pub(super) zh: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct TimelinePayload {
    pub(super) updated_at: String,
    #[serde(default)]
    pub(super) events: Vec<TimelineEventPayload>,
    #[serde(default)]
    pub(super) stats: Option<TimelineStatsPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct TimelineStatsPayload {
    #[serde(default)]
    pub(super) reset: u32,
    #[serde(default)]
    pub(super) credits: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct TimelineEventPayload {
    pub(super) id: String,
    pub(super) date: String,
    #[serde(rename = "type")]
    pub(super) kind: String,
    #[serde(default)]
    pub(super) summary: String,
    #[serde(default)]
    pub(super) url: Option<String>,
    pub(super) announced_at: String,
    #[serde(default)]
    pub(super) confirmed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct StatusHistoryPayload {
    pub(super) checked_at: String,
    #[serde(default)]
    pub(super) stale: bool,
    pub(super) current: CurrentStatusPayload,
    #[serde(default)]
    pub(super) incidents: Vec<IncidentPayload>,
    #[serde(default)]
    pub(super) stats: Option<StatusStatsPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct CurrentStatusPayload {
    #[serde(default)]
    pub(super) surfaces: Vec<SurfacePayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct SurfacePayload {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct IncidentPayload {
    pub(super) name: String,
    pub(super) status: String,
    pub(super) impact: String,
    pub(super) started_at: String,
    #[serde(default)]
    pub(super) resolved_at: Option<String>,
    #[serde(default)]
    pub(super) source_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct StatusStatsPayload {
    #[serde(default)]
    pub(super) days: u32,
    #[serde(default)]
    pub(super) incident_count: u32,
    #[serde(default)]
    pub(super) resolved_incident_count: u32,
    #[serde(default)]
    pub(super) open_incident_count: u32,
    #[serde(default)]
    pub(super) median_recovery_minutes: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct RadarFeedPayload {
    pub(super) fetched_at: String,
    #[serde(default)]
    pub(super) stale: bool,
    #[serde(default)]
    pub(super) signal: Option<FeedSignalPayload>,
    #[serde(default)]
    pub(super) banked_observations: Vec<FeedBankedPayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct FeedSignalPayload {
    #[serde(default)]
    pub(super) summary: Option<String>,
    pub(super) at: String,
    pub(super) url: String,
    #[serde(default)]
    pub(super) active: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct FeedBankedPayload {
    pub(super) at: String,
    #[serde(default)]
    pub(super) text: Option<String>,
}
