//! Read-only Codex reset signals from Codex Runway's public status feed.
//!
//! The feed is an independent public monitor, not an OpenAI account API. This
//! module normalizes only the three facts shown in the overview and never
//! reads local credentials, session data, or account quotas.

use crate::probe;
use serde::{Deserialize, Serialize};

const STATUS_URL: &str = "https://www.codexrunway.com/api/status.json";
const STATUS_SCHEMA_VERSION: u32 = 1;
const TIBO_HANDLE: &str = "thsottiaux";
const MAX_POST_TEXT_CHARS: usize = 600;

/// A normalized result of one explicit public reset-signal check.
///
/// This is also the complete on-disk cache contract. It deliberately contains
/// only public, already-normalized values and never a credential or raw feed
/// payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexResetStatus {
    pub source_url: String,
    pub feed_status: CodexResetFeedStatus,
    pub generated_at: String,
    pub last_successful_check_at: String,
    pub checked_at: String,
    pub latest_confirmed_signal: Option<ResetSignal>,
    pub next_scheduled_reset: Option<ResetSignal>,
    pub latest_relevant_tibo_post: Option<TiboPost>,
    pub source_warning: Option<String>,
}

/// What the overview is currently showing. `Cached` means the app has not
/// completed a newer public read in this run; it is not a claim about the
/// freshness of the external monitor itself.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexResetRead {
    pub status: CodexResetStatus,
    pub freshness: CodexResetFreshness,
    pub cache_warning: Option<String>,
}

impl CodexResetRead {
    pub fn cached(status: CodexResetStatus) -> Self {
        Self {
            status,
            freshness: CodexResetFreshness::Cached,
            cache_warning: None,
        }
    }

    pub fn live(status: CodexResetStatus, cache_warning: Option<String>) -> Self {
        Self {
            status,
            freshness: CodexResetFreshness::Live,
            cache_warning,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexResetFreshness {
    Cached,
    Live,
}

/// The monitor's own health declaration. It says nothing about a user's
/// Codex entitlement or quota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexResetFeedStatus {
    Ok,
    Degraded,
}

/// The public reset category reported by the source. It describes the signal,
/// never the entitlement of the signed-in account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResetType {
    Global,
    Banked,
    Other,
}

/// One confirmed or scheduled reset event. Scheduled events retain their
/// precision because a date-level estimate must not look like an exact time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResetSignal {
    pub announced_at: String,
    pub effective_at: Option<String>,
    pub schedule_precision: Option<String>,
    pub confidence: f64,
    pub reset_type: ResetType,
}

/// The newest reset-related public post attributed to Tibo in this feed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TiboPost {
    pub announced_at: String,
    pub text: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Feed {
    schema_version: u32,
    generated_at: String,
    last_successful_check_at: String,
    monitor: FeedMonitor,
    #[serde(default)]
    events: Vec<FeedEvent>,
    #[serde(default)]
    reset_timeline: ResetTimeline,
}

#[derive(Debug, Deserialize)]
struct FeedMonitor {
    status: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResetTimeline {
    next_schedule: Option<FeedEvent>,
    #[serde(default)]
    fulfilled_schedules: Vec<TimelineCompletion>,
    #[serde(default)]
    manual_completions: Vec<TimelineCompletion>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimelineCompletion {
    completed_at: String,
    #[serde(default)]
    schedule: Option<FeedEvent>,
    #[serde(default)]
    schedules: Vec<FeedEvent>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedEvent {
    kind: String,
    #[serde(default)]
    reset_type: Option<String>,
    announced_at: String,
    #[serde(default)]
    effective_at: Option<String>,
    #[serde(default)]
    schedule_precision: Option<String>,
    #[serde(default)]
    confidence: f64,
    #[serde(default)]
    source: Option<FeedSource>,
    #[serde(default)]
    text: String,
}

#[derive(Debug, Deserialize)]
struct FeedSource {
    handle: Option<String>,
    url: Option<String>,
}

fn parse_timestamp(value: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    chrono::DateTime::parse_from_rfc3339(value).ok()
}

fn validate_timestamp(value: &str, label: &str) -> Result<(), String> {
    parse_timestamp(value)
        .map(|_| ())
        .ok_or_else(|| format!("公开 feed 的{label}不是有效时间"))
}

impl CodexResetStatus {
    /// Rejects malformed local cache data without rewriting it. The caller can
    /// then preserve the original file for inspection and ask for a refresh.
    pub(crate) fn validate_cached(&self) -> Result<(), String> {
        if self.source_url != STATUS_URL {
            return Err("缓存的重置信号来源无效".to_string());
        }
        validate_timestamp(&self.generated_at, "生成时间")?;
        validate_timestamp(&self.last_successful_check_at, "最近成功检查时间")?;
        validate_timestamp(&self.checked_at, "本次读取时间")?;

        for (label, signal) in [
            ("确认重置信号", self.latest_confirmed_signal.as_ref()),
            ("预计重置信号", self.next_scheduled_reset.as_ref()),
        ] {
            let Some(signal) = signal else {
                continue;
            };
            validate_timestamp(&signal.announced_at, label)?;
            if let Some(effective_at) = &signal.effective_at {
                validate_timestamp(effective_at, label)?;
            }
            if !(0.0..=1.0).contains(&signal.confidence) {
                return Err("缓存的重置信号置信度无效".to_string());
            }
        }

        if let Some(post) = &self.latest_relevant_tibo_post {
            validate_timestamp(&post.announced_at, "Tibo 动态时间")?;
            if post.text.trim().is_empty() || !is_tibo_post_url(&post.url) {
                return Err("缓存的 Tibo 动态无效".to_string());
            }
        }
        Ok(())
    }
}

fn reset_signal(event: &FeedEvent) -> Result<ResetSignal, String> {
    validate_timestamp(&event.announced_at, "公告时间")?;
    if let Some(effective_at) = &event.effective_at {
        validate_timestamp(effective_at, "预计时间")?;
    }
    if !(0.0..=1.0).contains(&event.confidence) {
        return Err("公开 feed 的置信度无效".to_string());
    }
    Ok(ResetSignal {
        announced_at: event.announced_at.clone(),
        effective_at: event.effective_at.clone(),
        schedule_precision: event.schedule_precision.clone(),
        confidence: event.confidence,
        reset_type: match event.reset_type.as_deref() {
            Some("global") => ResetType::Global,
            Some("banked") => ResetType::Banked,
            _ => ResetType::Other,
        },
    })
}

fn completion_signal(
    completion: &TimelineCompletion,
    event: &FeedEvent,
) -> Option<(chrono::DateTime<chrono::FixedOffset>, ResetSignal)> {
    let completed_at = parse_timestamp(&completion.completed_at)?;
    let mut signal = reset_signal(event).ok()?;
    signal.announced_at = completion.completed_at.clone();
    signal.effective_at = None;
    signal.schedule_precision = None;
    Some((completed_at, signal))
}

fn completion_events(completion: &TimelineCompletion) -> impl Iterator<Item = &FeedEvent> {
    completion
        .schedule
        .iter()
        .chain(completion.schedules.iter())
}

fn latest_confirmed_signal(feed: &Feed) -> Option<ResetSignal> {
    let direct_events = feed.events.iter().filter_map(|event| {
        (event.kind == "reset_completed")
            .then(|| parse_timestamp(&event.announced_at).zip(reset_signal(event).ok()))
            .flatten()
    });
    let timeline_events = feed
        .reset_timeline
        .fulfilled_schedules
        .iter()
        .chain(feed.reset_timeline.manual_completions.iter())
        .flat_map(|completion| {
            completion_events(completion)
                .filter_map(move |event| completion_signal(completion, event))
        });

    direct_events
        .chain(timeline_events)
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(_, signal)| signal)
}

fn is_tibo_post_url(url: &str) -> bool {
    let Some(post_id) = url.strip_prefix("https://x.com/thsottiaux/status/") else {
        return false;
    };
    !post_id.is_empty() && post_id.bytes().all(|byte| byte.is_ascii_digit())
}

fn compact_text(text: &str) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match compact.char_indices().nth(MAX_POST_TEXT_CHARS) {
        Some((index, _)) => format!("{}…", &compact[..index]),
        None => compact,
    }
}

fn tibo_post(event: &FeedEvent) -> Option<TiboPost> {
    let source = event.source.as_ref()?;
    if source.handle.as_deref() != Some(TIBO_HANDLE) {
        return None;
    }
    let url = source.url.as_deref()?;
    if !is_tibo_post_url(url) || parse_timestamp(&event.announced_at).is_none() {
        return None;
    }
    let text = compact_text(&event.text);
    if text.is_empty() {
        return None;
    }
    Some(TiboPost {
        announced_at: event.announced_at.clone(),
        text,
        url: url.to_string(),
    })
}

fn latest_tibo_post<'a>(events: impl Iterator<Item = &'a FeedEvent>) -> Option<TiboPost> {
    events
        .filter_map(|event| {
            let timestamp = parse_timestamp(&event.announced_at)?;
            tibo_post(event).map(|post| (timestamp, post))
        })
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(_, post)| post)
}

fn parse_feed(text: &str, checked_at: String) -> Result<CodexResetStatus, String> {
    let feed: Feed =
        serde_json::from_str(text).map_err(|_| "公开 reset feed 不是有效 JSON".to_string())?;
    if feed.schema_version != STATUS_SCHEMA_VERSION {
        return Err("公开 reset feed 的版本不受支持".to_string());
    }
    validate_timestamp(&feed.generated_at, "生成时间")?;
    validate_timestamp(&feed.last_successful_check_at, "最近成功检查时间")?;

    let latest_confirmed_signal = latest_confirmed_signal(&feed);
    let next_scheduled_reset = feed
        .reset_timeline
        .next_schedule
        .as_ref()
        .map(reset_signal)
        .transpose()?;
    let feed_status = if feed.monitor.status == "ok" {
        CodexResetFeedStatus::Ok
    } else {
        CodexResetFeedStatus::Degraded
    };
    let source_warning = (feed_status == CodexResetFeedStatus::Degraded)
        .then(|| "公开信号源未确认正常，展示的内容可能不是最新状态。".to_string());

    Ok(CodexResetStatus {
        source_url: STATUS_URL.to_string(),
        feed_status,
        generated_at: feed.generated_at,
        last_successful_check_at: feed.last_successful_check_at,
        checked_at,
        latest_confirmed_signal,
        next_scheduled_reset,
        latest_relevant_tibo_post: latest_tibo_post(
            feed.events
                .iter()
                .chain(feed.reset_timeline.next_schedule.iter())
                .chain(
                    feed.reset_timeline
                        .fulfilled_schedules
                        .iter()
                        .chain(feed.reset_timeline.manual_completions.iter())
                        .flat_map(completion_events),
                ),
        ),
        source_warning,
    })
}

/// Fetches the fixed public status endpoint once. It does not poll, access X
/// directly, or infer a per-account reset state. Persisting a successful
/// normalized result is owned by the application-state layer.
pub fn check() -> Result<CodexResetStatus, String> {
    let (status, body) = probe::http_get(STATUS_URL, "Accept: application/json")?;
    if !(200..300).contains(&status) {
        return Err(format!("公开 reset feed 返回 HTTP {status}"));
    }
    parse_feed(
        &body,
        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_fixture(events: &str, next_schedule: &str, monitor_status: &str) -> CodexResetStatus {
        let body = format!(
            r#"{{
                "schemaVersion": 1,
                "generatedAt": "2026-08-31T03:08:02.232Z",
                "lastSuccessfulCheckAt": "2026-08-31T03:08:02.232Z",
                "monitor": {{ "status": "{monitor_status}" }},
                "events": [{events}],
                "resetTimeline": {{ "nextSchedule": {next_schedule} }}
            }}"#
        );
        parse_feed(&body, "2026-08-31T03:10:00Z".to_string()).expect("feed")
    }

    #[test]
    fn normalizes_confirmed_reset_schedule_and_latest_tibo_post() {
        let older = r#"{
            "kind":"reset_completed",
            "announcedAt":"2026-08-30T02:00:00Z",
            "confidence":0.9,
            "source":{"handle":"thsottiaux","url":"https://x.com/thsottiaux/status/100"},
            "text":"Older reset signal"
        }"#;
        let newest = r#"{
            "kind":"reset_completed",
            "announcedAt":"2026-08-31T02:34:27Z",
            "confidence":0.98,
            "source":{"handle":"thsottiaux","url":"https://x.com/thsottiaux/status/200"},
            "text":"Newer reset signal\nwith a line break"
        }"#;
        let schedule = r#"{
            "kind":"reset_scheduled",
            "announcedAt":"2026-08-31T03:00:00Z",
            "effectiveAt":"2026-08-31T09:00:00Z",
            "schedulePrecision":"datetime",
            "confidence":0.84,
            "source":{"handle":"thsottiaux","url":"https://x.com/thsottiaux/status/300"},
            "text":"Newly announced schedule"
        }"#;

        let status = parse_fixture(&format!("{older},{newest}"), schedule, "ok");

        assert_eq!(status.source_url, STATUS_URL);
        assert_eq!(status.feed_status, CodexResetFeedStatus::Ok);
        assert_eq!(
            status
                .latest_confirmed_signal
                .as_ref()
                .map(|reset| &reset.announced_at),
            Some(&"2026-08-31T02:34:27Z".to_string())
        );
        assert_eq!(
            status
                .next_scheduled_reset
                .as_ref()
                .and_then(|reset| reset.effective_at.as_deref()),
            Some("2026-08-31T09:00:00Z")
        );
        assert_eq!(
            status
                .latest_relevant_tibo_post
                .as_ref()
                .map(|post| post.text.as_str()),
            Some("Newly announced schedule")
        );
    }

    #[test]
    fn preserves_an_absent_schedule_and_marks_degraded_source() {
        let status = parse_fixture("", "null", "error");

        assert_eq!(status.feed_status, CodexResetFeedStatus::Degraded);
        assert!(status.latest_confirmed_signal.is_none());
        assert!(status.next_scheduled_reset.is_none());
        assert!(status.latest_relevant_tibo_post.is_none());
        assert!(status.source_warning.is_some());
    }

    #[test]
    fn ignores_untrusted_tibo_links() {
        let event = r#"{
            "kind":"reset_completed",
            "announcedAt":"2026-08-31T02:34:27Z",
            "confidence":0.98,
            "source":{"handle":"thsottiaux","url":"https://example.com/thsottiaux/status/200"},
            "text":"Do not expose this link"
        }"#;
        let status = parse_fixture(event, "null", "ok");

        assert!(status.latest_relevant_tibo_post.is_none());
    }

    #[test]
    fn normalizes_a_manually_confirmed_reset_card_from_the_timeline() {
        let body = r#"{
            "schemaVersion": 1,
            "generatedAt": "2026-09-04T08:40:02.783Z",
            "lastSuccessfulCheckAt": "2026-09-04T08:40:02.783Z",
            "monitor": { "status": "ok" },
            "events": [],
            "resetTimeline": {
                "nextSchedule": null,
                "manualCompletions": [{
                    "completedAt": "2026-09-04T02:30:00Z",
                    "schedules": [{
                        "kind": "reset_scheduled",
                        "resetType": "banked",
                        "announcedAt": "2026-09-03T23:12:09Z",
                        "effectiveAt": "2026-09-04T02:12:09Z",
                        "schedulePrecision": "datetime",
                        "confidence": 0.93,
                        "source": {
                            "handle": "thsottiaux",
                            "url": "https://x.com/thsottiaux/status/2095651088502591861"
                        },
                        "text": "The first reset card has landed."
                    }]
                }]
            }
        }"#;

        let status = parse_feed(body, "2026-09-04T08:45:00Z".to_string()).expect("feed");
        let signal = status.latest_confirmed_signal.expect("completed signal");

        assert_eq!(signal.announced_at, "2026-09-04T02:30:00Z");
        assert_eq!(signal.reset_type, ResetType::Banked);
        assert_eq!(
            status.latest_relevant_tibo_post.map(|post| post.text),
            Some("The first reset card has landed.".to_string())
        );
    }

    #[test]
    fn rejects_unknown_schemas_and_malformed_timestamps() {
        let unsupported = r#"{
            "schemaVersion": 2,
            "generatedAt":"2026-08-31T03:08:02.232Z",
            "lastSuccessfulCheckAt":"2026-08-31T03:08:02.232Z",
            "monitor":{"status":"ok"}
        }"#;
        assert!(parse_feed(unsupported, "2026-08-31T03:10:00Z".to_string()).is_err());

        let invalid_time = r#"{
            "schemaVersion": 1,
            "generatedAt":"not-a-time",
            "lastSuccessfulCheckAt":"2026-08-31T03:08:02.232Z",
            "monitor":{"status":"ok"}
        }"#;
        assert!(parse_feed(invalid_time, "2026-08-31T03:10:00Z".to_string()).is_err());
    }
}
