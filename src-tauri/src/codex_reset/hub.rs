//! Read-only Codex reset signals from codex-reset.com, a second independent
//! public monitor. Four fixed endpoints form one source — the probability
//! forecast, the reset timeline ledger, the OpenAI platform status history,
//! and the X radar feed — and are read together in one check: the source
//! answers as a whole or not at all. This module normalizes only the facts
//! the reset panel shows and never reads local credentials or quotas.

use super::{
    CodexResetHubStatus, HubBankedNote, HubCadence, HubChance, HubConfidence, HubEvent, HubEventKind,
    HubFeed, HubForecast, HubIncident, HubPlatform, HubPlatformStats, HubSignalPost, HubSurface,
    HubSurfaceStatus, HubTimeline, HubWait, HubWindow,
};
use crate::probe;
use super::feed::validate_timestamp;

pub(super) const HUB_SOURCE_URL: &str = "https://codex-reset.com";
const FORECAST_URL: &str = "https://codex-reset.com/api/forecast";
const TIMELINE_URL: &str = "https://codex-reset.com/api/timeline";
const STATUS_HISTORY_URL: &str = "https://codex-reset.com/api/status-history";
const FEED_URL: &str = "https://codex-reset.com/api/feed";

/// Stored ledger and incident rows are capped at what the panel displays.
pub(super) const MAX_EVENTS: usize = 40;
pub(super) const MAX_INCIDENTS: usize = 12;
pub(super) const MAX_BANKED: usize = 10;

mod payload;
use payload::*;

fn valid_days(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

fn valid_url(url: &str) -> bool {
    url.starts_with("https://")
}

fn chance(percent: f64, range: &ForecastRange, label: &str) -> Result<HubChance, String> {
    if !(0.0..=100.0).contains(&percent)
        || !(0.0..=1.0).contains(&range.lower)
        || !(0.0..=1.0).contains(&range.upper)
        || range.lower > range.upper
    {
        return Err(format!("codex-reset.com forecast 的{label}概率数值无效"));
    }
    Ok(HubChance {
        percent: percent.round() as u32,
        range_lower: range.lower,
        range_upper: range.upper,
    })
}

fn parse_timestamp_or(value: &str, error: &str) -> Result<chrono::DateTime<chrono::FixedOffset>, String> {
    chrono::DateTime::parse_from_rfc3339(value).map_err(|_| error.to_string())
}

fn parse_forecast(text: &str) -> Result<HubForecast, String> {
    let payload: ForecastPayload = serde_json::from_str(text)
        .map_err(|error| format!("codex-reset.com forecast 接口载荷无效：{error}"))?;
    validate_timestamp(&payload.updated_at, "forecast 生成时间")?;
    validate_timestamp(&payload.last_reset_at, "forecast 上次重置时间")?;
    if !valid_days(payload.age_days) {
        return Err("codex-reset.com forecast 的距上次重置天数无效".to_string());
    }
    let confidence = match payload.confidence.as_str() {
        "low" => HubConfidence::Low,
        "medium" => HubConfidence::Medium,
        "high" => HubConfidence::High,
        _ => return Err("codex-reset.com forecast 的信心等级无效".to_string()),
    };
    let chance_24h = chance(payload.probabilities.rounded_24h, &payload.probabilities.range_24h, "24 小时")?;
    let chance_48h = chance(payload.probabilities.rounded_48h, &payload.probabilities.range_48h, "48 小时")?;
    let window = payload.time_window.map(parse_window).transpose()?;
    let cadence = payload.cadence.map(parse_cadence).transpose()?;
    let wait = payload.wait_comparison.map(parse_wait).transpose()?;
    let context_note = payload
        .context_copy
        .and_then(|copy| copy.zh)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    Ok(HubForecast {
        updated_at: payload.updated_at,
        confidence,
        chance_24h,
        chance_48h,
        last_reset_at: payload.last_reset_at,
        age_days: payload.age_days,
        window,
        cadence,
        wait,
        context_note,
    })
}

fn parse_window(window: ForecastWindowPayload) -> Result<HubWindow, String> {
    if window.start_hour >= window.end_hour || window.end_hour > 24 || window.timezone.trim().is_empty() {
        return Err("codex-reset.com forecast 的到账窗口无效".to_string());
    }
    Ok(HubWindow {
        start_hour: window.start_hour,
        end_hour: window.end_hour,
        timezone: window.timezone,
    })
}

fn parse_cadence(cadence: ForecastCadencePayload) -> Result<HubCadence, String> {
    if !valid_days(cadence.recent_median_days) || !valid_days(cadence.weighted_mean_days) {
        return Err("codex-reset.com forecast 的重置节奏无效".to_string());
    }
    Ok(HubCadence {
        median_days: cadence.recent_median_days,
        sample: cadence.recent_sample,
        mean_days: cadence.weighted_mean_days,
        accelerating: cadence.accelerating,
    })
}

fn parse_wait(wait: ForecastWaitPayload) -> Result<HubWait, String> {
    if !valid_days(wait.wait_days)
        || !valid_days(wait.median_days)
        || !(0.0..=1.0).contains(&wait.shorter_share)
    {
        return Err("codex-reset.com forecast 的等待对比无效".to_string());
    }
    Ok(HubWait {
        days: wait.wait_days,
        sample: wait.sample,
        shorter_share: wait.shorter_share,
        median_days: wait.median_days,
    })
}

fn parse_timeline(text: &str) -> Result<HubTimeline, String> {
    let payload: TimelinePayload = serde_json::from_str(text)
        .map_err(|error| format!("codex-reset.com timeline 接口载荷无效：{error}"))?;
    validate_timestamp(&payload.updated_at, "timeline 生成时间")?;
    let mut events = Vec::new();
    for event in payload.events {
        if event.id.trim().is_empty() {
            return Err("codex-reset.com timeline 的事件标识无效".to_string());
        }
        chrono::NaiveDate::parse_from_str(&event.date, "%Y-%m-%d")
            .map_err(|_| "codex-reset.com timeline 的事件日期无效".to_string())?;
        let kind = match event.kind.as_str() {
            "reset" => HubEventKind::Reset,
            "credits" => HubEventKind::Credits,
            _ => return Err("codex-reset.com timeline 的事件类型无效".to_string()),
        };
        let announced = parse_timestamp_or(
            &event.announced_at,
            "codex-reset.com timeline 的事件时间无效",
        )?;
        if let Some(url) = &event.url {
            if !valid_url(url) {
                return Err("codex-reset.com timeline 的事件链接无效".to_string());
            }
        }
        events.push((
            announced,
            HubEvent {
                id: event.id,
                date: event.date,
                kind,
                summary: event.summary,
                url: event.url,
                announced_at: event.announced_at,
                confirmed: event.confirmed,
            },
        ));
    }
    events.sort_by(|(left, _), (right, _)| right.cmp(left));
    events.truncate(MAX_EVENTS);
    let stats = payload.stats.unwrap_or(TimelineStatsPayload { reset: 0, credits: 0 });
    Ok(HubTimeline {
        updated_at: payload.updated_at,
        resets: stats.reset,
        credits: stats.credits,
        events: events.into_iter().map(|(_, event)| event).collect(),
    })
}

fn surface_status(raw: &str) -> HubSurfaceStatus {
    match raw {
        "operational" => HubSurfaceStatus::Operational,
        "degraded_performance" | "partial_outage" => HubSurfaceStatus::Degraded,
        "major_outage" => HubSurfaceStatus::Outage,
        "maintenance" => HubSurfaceStatus::Maintenance,
        // An unknown future state must stay visible, never read as healthy.
        _ => HubSurfaceStatus::Degraded,
    }
}

fn parse_platform(text: &str) -> Result<HubPlatform, String> {
    let payload: StatusHistoryPayload = serde_json::from_str(text)
        .map_err(|error| format!("codex-reset.com status-history 接口载荷无效：{error}"))?;
    validate_timestamp(&payload.checked_at, "status-history 检查时间")?;
    if payload.current.surfaces.is_empty() {
        return Err("codex-reset.com status-history 缺少平台面".to_string());
    }
    let mut surfaces = Vec::new();
    for surface in payload.current.surfaces {
        if surface.id.trim().is_empty() || surface.label.trim().is_empty() {
            return Err("codex-reset.com status-history 的平台面无效".to_string());
        }
        surfaces.push(HubSurface {
            id: surface.id,
            label: surface.label,
            status: surface_status(&surface.status),
        });
    }
    let incidents = parse_incidents(payload.incidents)?;
    let stats = payload.stats.unwrap_or(StatusStatsPayload {
        days: 0,
        incident_count: 0,
        resolved_incident_count: 0,
        open_incident_count: 0,
        median_recovery_minutes: None,
    });
    if let Some(median) = stats.median_recovery_minutes {
        if !valid_days(median) {
            return Err("codex-reset.com status-history 的恢复时长无效".to_string());
        }
    }
    Ok(HubPlatform {
        checked_at: payload.checked_at,
        stale: payload.stale,
        surfaces,
        stats: HubPlatformStats {
            days: stats.days,
            incidents: stats.incident_count,
            resolved: stats.resolved_incident_count,
            open: stats.open_incident_count,
            median_recovery_minutes: stats.median_recovery_minutes,
        },
        incidents,
    })
}

fn parse_incidents(input: Vec<IncidentPayload>) -> Result<Vec<HubIncident>, String> {
    let mut incidents = Vec::new();
    for incident in input {
        if incident.name.trim().is_empty() || incident.impact.trim().is_empty() {
            return Err("codex-reset.com status-history 的故障记录无效".to_string());
        }
        let started = parse_timestamp_or(
            &incident.started_at,
            "codex-reset.com status-history 的故障时间无效",
        )?;
        if let Some(resolved_at) = &incident.resolved_at {
            validate_timestamp(resolved_at, "status-history 故障恢复时间")?;
        }
        if let Some(url) = &incident.source_url {
            if !valid_url(url) {
                return Err("codex-reset.com status-history 的故障链接无效".to_string());
            }
        }
        incidents.push((
            started,
            HubIncident {
                name: incident.name,
                impact: incident.impact,
                resolved: incident.status == "resolved",
                started_at: incident.started_at,
                resolved_at: incident.resolved_at,
                url: incident.source_url,
            },
        ));
    }
    incidents.sort_by(|(left, _), (right, _)| right.cmp(left));
    incidents.truncate(MAX_INCIDENTS);
    Ok(incidents.into_iter().map(|(_, incident)| incident).collect())
}

fn parse_radar(text: &str) -> Result<HubFeed, String> {
    let payload: RadarFeedPayload =
        serde_json::from_str(text).map_err(|error| format!("codex-reset.com feed 接口载荷无效：{error}"))?;
    validate_timestamp(&payload.fetched_at, "feed 抓取时间")?;
    let signal = payload.signal.and_then(|signal| {
        let summary = signal.summary?.trim().to_string();
        if summary.is_empty() {
            return None;
        }
        chrono::DateTime::parse_from_rfc3339(&signal.at).ok()?;
        if !valid_url(&signal.url) {
            return None;
        }
        Some(HubSignalPost {
            summary,
            at: signal.at,
            url: signal.url,
            active: signal.active,
        })
    });
    let mut banked = Vec::new();
    for note in payload.banked_observations {
        let Some(text) = note.text.filter(|text| !text.trim().is_empty()) else {
            continue;
        };
        let Ok(at) = chrono::DateTime::parse_from_rfc3339(&note.at) else {
            continue;
        };
        banked.push((at, HubBankedNote { at: note.at, text }));
    }
    banked.sort_by(|(left, _), (right, _)| right.cmp(left));
    banked.truncate(MAX_BANKED);
    Ok(HubFeed {
        fetched_at: payload.fetched_at,
        stale: payload.stale,
        signal,
        banked: banked.into_iter().map(|(_, note)| note).collect(),
    })
}

fn fetch_json(url: &str, label: &str) -> Result<String, String> {
    let (status, body) = probe::http_get(url, "Accept: application/json")
        .map_err(|error| format!("codex-reset.com {label} 接口不可用：{error}"))?;
    if !(200..300).contains(&status) {
        return Err(format!("codex-reset.com {label} 接口返回 HTTP {status}"));
    }
    Ok(body)
}

/// Reads the four fixed endpoints once each. One failing endpoint fails the
/// whole source read with the endpoint named; nothing is persisted here.
pub(super) fn check() -> Result<CodexResetHubStatus, String> {
    let forecast = parse_forecast(&fetch_json(FORECAST_URL, "forecast")?)?;
    let timeline = parse_timeline(&fetch_json(TIMELINE_URL, "timeline")?)?;
    let platform = parse_platform(&fetch_json(STATUS_HISTORY_URL, "status-history")?)?;
    let feed = parse_radar(&fetch_json(FEED_URL, "feed")?)?;
    let generated_at = [
        forecast.updated_at.as_str(),
        timeline.updated_at.as_str(),
        platform.checked_at.as_str(),
        feed.fetched_at.as_str(),
    ]
    .into_iter()
    .filter_map(|value| chrono::DateTime::parse_from_rfc3339(value).ok().map(|time| (time, value)))
    .max_by(|(left, _), (right, _)| left.cmp(right))
    .map(|(_, value)| value.to_string())
    .ok_or_else(|| "codex-reset.com 的接口时间全部无效".to_string())?;
    let source_warning = (platform.stale || feed.stale)
        .then(|| "信号源标记数据可能过期，展示的内容可能不是最新状态。".to_string());
    Ok(CodexResetHubStatus {
        source_url: HUB_SOURCE_URL.to_string(),
        generated_at,
        checked_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        source_warning,
        forecast,
        timeline,
        platform,
        feed,
    })
}

#[cfg(test)]
mod tests;
