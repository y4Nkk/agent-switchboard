use super::{feed::validate_timestamp, hub::{HUB_SOURCE_URL, MAX_BANKED, MAX_EVENTS, MAX_INCIDENTS}, CodexResetRead};
use serde::{Deserialize, Serialize};

/// The second monitor's normalized snapshot: one forecast answer plus three
/// ledgers (reset events, OpenAI platform status, radar highlights). Only
/// public, already-normalized facts; never a credential or raw payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexResetHubStatus {
    pub source_url: String,
    pub generated_at: String,
    pub checked_at: String,
    pub source_warning: Option<String>,
    pub forecast: HubForecast,
    pub timeline: HubTimeline,
    pub platform: HubPlatform,
    pub feed: HubFeed,
}

pub type CodexResetHubRead = CodexResetRead<CodexResetHubStatus>;

/// The site's own confidence in its probability model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HubConfidence {
    Low,
    Medium,
    High,
}

/// One public probability figure with its modelled interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HubChance {
    pub percent: u32,
    pub range_lower: f64,
    pub range_upper: f64,
}

/// The historical hour window most resets land in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HubWindow {
    pub start_hour: u32,
    pub end_hour: u32,
    pub timezone: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HubCadence {
    pub median_days: f64,
    pub sample: u32,
    pub mean_days: f64,
    pub accelerating: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HubWait {
    pub days: f64,
    pub sample: u32,
    pub shorter_share: f64,
    pub median_days: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HubForecast {
    pub updated_at: String,
    pub confidence: HubConfidence,
    pub chance_24h: HubChance,
    pub chance_48h: HubChance,
    pub last_reset_at: String,
    pub age_days: f64,
    pub window: Option<HubWindow>,
    pub cadence: Option<HubCadence>,
    pub wait: Option<HubWait>,
    /// The site's own Simplified Chinese explanation of the current context.
    pub context_note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HubEventKind {
    Reset,
    Credits,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HubEvent {
    pub id: String,
    pub date: String,
    pub kind: HubEventKind,
    pub summary: String,
    pub url: Option<String>,
    pub announced_at: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HubTimeline {
    pub updated_at: String,
    pub resets: u32,
    pub credits: u32,
    pub events: Vec<HubEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HubSurfaceStatus {
    Operational,
    Degraded,
    Outage,
    Maintenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HubSurface {
    pub id: String,
    pub label: String,
    pub status: HubSurfaceStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HubPlatformStats {
    pub days: u32,
    pub incidents: u32,
    pub resolved: u32,
    pub open: u32,
    pub median_recovery_minutes: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HubIncident {
    pub name: String,
    pub impact: String,
    pub resolved: bool,
    pub started_at: String,
    pub resolved_at: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HubPlatform {
    pub checked_at: String,
    pub stale: bool,
    pub surfaces: Vec<HubSurface>,
    pub stats: HubPlatformStats,
    pub incidents: Vec<HubIncident>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HubSignalPost {
    pub summary: String,
    pub at: String,
    pub url: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HubBankedNote {
    pub at: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HubFeed {
    pub fetched_at: String,
    pub stale: bool,
    pub signal: Option<HubSignalPost>,
    pub banked: Vec<HubBankedNote>,
}

impl CodexResetHubStatus {
    /// Rejects malformed local cache data without rewriting it, mirroring the
    /// first source's strictness: stored rows re-validate every fact.
    pub(crate) fn validate_cached(&self) -> Result<(), String> {
        if self.source_url != HUB_SOURCE_URL {
            return Err("缓存的重置信号来源无效".to_string());
        }
        validate_timestamp(&self.generated_at, "生成时间")?;
        validate_timestamp(&self.checked_at, "本次读取时间")?;

        validate_forecast(&self.forecast)?;
        validate_timeline(&self.timeline)?;
        validate_platform(&self.platform)?;
        validate_feed(&self.feed)?;
        Ok(())
    }
}

fn validate_forecast(forecast: &HubForecast) -> Result<(), String> {
    validate_timestamp(&forecast.updated_at, "forecast 生成时间")?;
    validate_timestamp(&forecast.last_reset_at, "forecast 上次重置时间")?;
    if !forecast.age_days.is_finite() || forecast.age_days < 0.0 {
        return Err("缓存的距上次重置天数无效".to_string());
    }
    for (label, chance) in [
        ("24 小时", &forecast.chance_24h),
        ("48 小时", &forecast.chance_48h),
    ] {
        if chance.percent > 100
            || !(0.0..=1.0).contains(&chance.range_lower)
            || !(0.0..=1.0).contains(&chance.range_upper)
            || chance.range_lower > chance.range_upper
        {
            return Err(format!("缓存的{label}概率数值无效"));
        }
    }
    if let Some(window) = &forecast.window {
        if window.start_hour >= window.end_hour || window.end_hour > 24 || window.timezone.trim().is_empty() {
            return Err("缓存的到账窗口无效".to_string());
        }
    }
    if let Some(cadence) = &forecast.cadence {
        if !cadence.median_days.is_finite() || cadence.median_days < 0.0
            || !cadence.mean_days.is_finite() || cadence.mean_days < 0.0 {
            return Err("缓存的重置节奏无效".to_string());
        }
    }
    if let Some(wait) = &forecast.wait {
        if !wait.days.is_finite() || wait.days < 0.0
            || !wait.median_days.is_finite() || wait.median_days < 0.0
            || !(0.0..=1.0).contains(&wait.shorter_share) {
            return Err("缓存的等待对比无效".to_string());
        }
    }
    if let Some(note) = &forecast.context_note {
        if note.trim().is_empty() {
            return Err("缓存的 forecast 上下文说明无效".to_string());
        }
    }
    Ok(())
}

fn validate_timeline(timeline: &HubTimeline) -> Result<(), String> {
    validate_timestamp(&timeline.updated_at, "timeline 生成时间")?;
    if timeline.events.len() > MAX_EVENTS {
        return Err("缓存的重置时间线范围无效".to_string());
    }
    for event in &timeline.events {
        if event.id.trim().is_empty() {
            return Err("缓存的时间线事件标识无效".to_string());
        }
        chrono::NaiveDate::parse_from_str(&event.date, "%Y-%m-%d")
            .map_err(|_| "缓存的时间线事件日期无效".to_string())?;
        validate_timestamp(&event.announced_at, "时间线事件时间")?;
        if let Some(url) = &event.url {
            if !url.starts_with("https://") {
                return Err("缓存的时间线事件链接无效".to_string());
            }
        }
    }
    Ok(())
}

fn validate_platform(platform: &HubPlatform) -> Result<(), String> {
    validate_timestamp(&platform.checked_at, "平台状态检查时间")?;
    if platform.surfaces.is_empty() {
        return Err("缓存的平台状态缺少平台面".to_string());
    }
    for surface in &platform.surfaces {
        if surface.id.trim().is_empty() || surface.label.trim().is_empty() {
            return Err("缓存的平台面无效".to_string());
        }
    }
    if platform.incidents.len() > MAX_INCIDENTS {
        return Err("缓存的故障记录范围无效".to_string());
    }
    for incident in &platform.incidents {
        if incident.name.trim().is_empty() || incident.impact.trim().is_empty() {
            return Err("缓存的故障记录无效".to_string());
        }
        validate_timestamp(&incident.started_at, "故障开始时间")?;
        if let Some(resolved_at) = &incident.resolved_at {
            validate_timestamp(resolved_at, "故障恢复时间")?;
        }
        if let Some(url) = &incident.url {
            if !url.starts_with("https://") {
                return Err("缓存的故障链接无效".to_string());
            }
        }
    }
    if let Some(median) = platform.stats.median_recovery_minutes {
        if !median.is_finite() || median < 0.0 {
            return Err("缓存的故障恢复时长无效".to_string());
        }
    }
    Ok(())
}

fn validate_feed(feed: &HubFeed) -> Result<(), String> {
    validate_timestamp(&feed.fetched_at, "feed 抓取时间")?;
    if let Some(signal) = &feed.signal {
        validate_timestamp(&signal.at, "feed 信号时间")?;
        if signal.summary.trim().is_empty() || !signal.url.starts_with("https://") {
            return Err("缓存的 feed 信号帖无效".to_string());
        }
    }
    if feed.banked.len() > MAX_BANKED {
        return Err("缓存的重置卡记录范围无效".to_string());
    }
    for note in &feed.banked {
        validate_timestamp(&note.at, "重置卡记录时间")?;
        if note.text.trim().is_empty() {
            return Err("缓存的重置卡记录无效".to_string());
        }
    }
    Ok(())
}
