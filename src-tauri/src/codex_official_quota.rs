//! Read-only Codex official-subscription quota service.
//!
//! The service is deliberately separate from provider `UsageQuery`: official
//! quota belongs to the existing Codex ChatGPT login, not to a provider API
//! key or endpoint. OAuth values are read only long enough to make the
//! request and never cross the desktop command boundary.
//!
//! This module additionally owns the persisted-baseline types and the pure
//! after-the-fact reset detection derived from consecutive successful reads.
//! It never writes to disk itself; persisting a baseline is owned by the
//! application-state layer.

use asb_core::contracts::{
    CodexOfficialQuota, CodexOfficialQuotaReset, CodexOfficialQuotaResetKind,
    CodexOfficialQuotaStatus, CodexOfficialQuotaWindow,
};
use asb_switch::sha256_hex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

/// The label produced for the server's weekly (604800-second) window. It is
/// the only window whose usage cannot decrease without a reset.
const WEEKLY_WINDOW_LABEL: &str = "7 天";

#[derive(Clone)]
struct CachedQuota {
    account_marker: String,
    quota: CodexOfficialQuota,
}

static CACHE: OnceLock<Mutex<HashMap<String, CachedQuota>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, CachedQuota>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Deserialize)]
struct AuthFile {
    auth_mode: Option<String>,
    tokens: Option<AuthTokens>,
}

#[derive(Deserialize)]
struct AuthTokens {
    access_token: Option<String>,
    account_id: Option<String>,
}

#[derive(Deserialize)]
struct UsageResponse {
    rate_limit: Option<RateLimit>,
}

#[derive(Deserialize)]
struct RateLimit {
    primary_window: Option<RateLimitWindow>,
    secondary_window: Option<RateLimitWindow>,
}

#[derive(Deserialize)]
struct RateLimitWindow {
    used_percent: Option<f64>,
    limit_window_seconds: Option<i64>,
    reset_at: Option<i64>,
}

struct AuthCredentials {
    access_token: String,
    account_id: Option<String>,
}

/// Queries the existing Codex login for one official profile and returns the
/// quota together with the login's account marker. A failed read preserves an
/// earlier in-process result for the same profile and marks it stale; no
/// snapshot is ever written to disk.
pub(crate) fn query(profile_id: &str, auth_path: &Path) -> (CodexOfficialQuota, Option<String>) {
    let (fresh, marker) = fetch(auth_path);
    if fresh.status == CodexOfficialQuotaStatus::Available {
        store_last_success(profile_id, marker.as_deref(), &fresh);
        return (fresh, marker);
    }

    let previous = cache()
        .lock()
        .ok()
        .and_then(|entries| matching_last_success(&entries, profile_id, marker.as_deref()));
    (retain_last_success(fresh, previous.as_ref()), marker)
}

fn store_last_success(profile_id: &str, marker: Option<&str>, quota: &CodexOfficialQuota) {
    let Some(marker) = marker else {
        return;
    };
    if let Ok(mut entries) = cache().lock() {
        entries.insert(
            profile_id.to_string(),
            CachedQuota {
                account_marker: marker.to_string(),
                quota: quota.clone(),
            },
        );
    }
}

fn matching_last_success(
    entries: &HashMap<String, CachedQuota>,
    profile_id: &str,
    marker: Option<&str>,
) -> Option<CodexOfficialQuota> {
    let marker = marker?;
    entries
        .get(profile_id)
        .filter(|entry| entry.account_marker == marker)
        .map(|entry| entry.quota.clone())
}

/// One direct read of the machine's Codex login for the overview, outside any
/// profile. The caller owns display and error handling, so failed reads are
/// returned as statuses instead of being retained in process.
pub(crate) fn query_login(auth_path: &Path) -> (CodexOfficialQuota, Option<String>) {
    fetch(auth_path)
}

/// Removes a profile's last successful official-quota result after deletion
/// or profile reset.
pub(crate) fn invalidate(profile_id: &str) {
    if let Ok(mut entries) = cache().lock() {
        entries.remove(profile_id);
    }
}

/// Drops all in-memory official quota reads with the profile store.
pub(crate) fn clear() {
    if let Ok(mut entries) = cache().lock() {
        entries.clear();
    }
}

fn fetch(auth_path: &Path) -> (CodexOfficialQuota, Option<String>) {
    let credentials = match std::fs::read_to_string(auth_path)
        .ok()
        .and_then(|text| parse_credentials(&text))
    {
        Some(credentials) => credentials,
        None => return (empty(CodexOfficialQuotaStatus::SignInRequired), None),
    };

    let marker = account_marker(credentials.account_id.as_deref());
    let mut headers = format!(
        "Authorization: Bearer {}\r\nAccept: application/json\r\n",
        credentials.access_token
    );
    if let Some(account_id) = credentials.account_id {
        headers.push_str(&format!("ChatGPT-Account-Id: {account_id}\r\n"));
    }

    let quota = match crate::probe::http_get(CODEX_USAGE_URL, &headers) {
        Ok((status, body)) => quota_from_http_response(status, &body, utc_now()),
        Err(_) => empty(CodexOfficialQuotaStatus::Unavailable),
    };
    (quota, marker)
}

fn parse_credentials(content: &str) -> Option<AuthCredentials> {
    let auth: AuthFile = serde_json::from_str(content).ok()?;
    if auth.auth_mode.as_deref() != Some("chatgpt") {
        return None;
    }
    let tokens = auth.tokens?;
    let access_token = tokens.access_token?.trim().to_string();
    if access_token.is_empty() || has_header_control_characters(&access_token) {
        return None;
    }
    let account_id = tokens.account_id.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty() && !has_header_control_characters(&value)).then_some(value)
    });
    Some(AuthCredentials {
        access_token,
        account_id,
    })
}

/// A short non-reversible digest identifying the logged-in account. It only
/// ever lives inside the local baseline file, so a re-login to a different
/// account can be distinguished from a quota reset.
fn account_marker(account_id: Option<&str>) -> Option<String> {
    let account_id = account_id?;
    Some(sha256_hex(account_id)[..16].to_string())
}

fn has_header_control_characters(value: &str) -> bool {
    value
        .chars()
        .any(|character| character == '\r' || character == '\n' || character == '\0')
}

fn quota_from_http_response(status: u16, body: &str, at: String) -> CodexOfficialQuota {
    match status {
        200..=299 => quota_from_body(body, at)
            .unwrap_or_else(|| empty(CodexOfficialQuotaStatus::Unavailable)),
        401 | 403 => empty(CodexOfficialQuotaStatus::ReauthenticationRequired),
        _ => empty(CodexOfficialQuotaStatus::Unavailable),
    }
}

fn quota_from_body(body: &str, at: String) -> Option<CodexOfficialQuota> {
    let response: UsageResponse = serde_json::from_str(body).ok()?;
    let rate_limit = response.rate_limit?;
    let windows = [rate_limit.primary_window, rate_limit.secondary_window]
        .into_iter()
        .flatten()
        .filter_map(normalize_window)
        .collect::<Vec<_>>();
    (!windows.is_empty()).then_some(CodexOfficialQuota {
        status: CodexOfficialQuotaStatus::Available,
        windows,
        at: Some(at),
        stale: false,
        last_reset: None,
    })
}

fn normalize_window(window: RateLimitWindow) -> Option<CodexOfficialQuotaWindow> {
    let used_percent = window.used_percent?;
    if !used_percent.is_finite() || !(0.0..=100.0).contains(&used_percent) {
        return None;
    }
    let seconds = window.limit_window_seconds?;
    if seconds <= 0 {
        return None;
    }
    Some(CodexOfficialQuotaWindow {
        label: quota_window_label(seconds),
        used_percent,
        resets_at: window.reset_at.and_then(unix_timestamp_to_rfc3339),
    })
}

fn quota_window_label(seconds: i64) -> String {
    match seconds {
        18_000 => "5 小时".to_string(),
        604_800 => "7 天".to_string(),
        2_592_000 => "30 天".to_string(),
        seconds if seconds % 86_400 == 0 => format!("{} 天", seconds / 86_400),
        seconds if seconds % 3_600 == 0 => format!("{} 小时", seconds / 3_600),
        seconds => format!("{} 分钟", seconds / 60),
    }
}

fn unix_timestamp_to_rfc3339(timestamp: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|value| value.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

fn utc_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn empty(status: CodexOfficialQuotaStatus) -> CodexOfficialQuota {
    CodexOfficialQuota {
        status,
        windows: Vec::new(),
        at: None,
        stale: false,
        last_reset: None,
    }
}

fn retain_last_success(
    failure: CodexOfficialQuota,
    previous: Option<&CodexOfficialQuota>,
) -> CodexOfficialQuota {
    previous
        .filter(|quota| quota.status == CodexOfficialQuotaStatus::Available)
        .map(|quota| CodexOfficialQuota {
            status: failure.status,
            windows: quota.windows.clone(),
            at: quota.at.clone(),
            stale: true,
            last_reset: None,
        })
        .unwrap_or(failure)
}

/// The persisted comparison baseline for after-the-fact reset detection. It
/// contains only normalized quota values plus the account marker, never a
/// credential or raw upstream payload.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CodexQuotaBaseline {
    /// Short digest of the account id; `None` when the login file omits it.
    pub(crate) account_marker: Option<String>,
    pub(crate) last_read: Option<BaselineRead>,
    pub(crate) last_reset: Option<CodexOfficialQuotaReset>,
}

/// One successful official read kept as the detection baseline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BaselineRead {
    pub(crate) at: String,
    pub(crate) windows: Vec<CodexOfficialQuotaWindow>,
}

impl CodexQuotaBaseline {
    /// Rejects malformed local baseline data without rewriting it.
    pub(crate) fn validate(&self) -> Result<(), String> {
        if let Some(read) = &self.last_read {
            parse_timestamp(&read.at).ok_or_else(|| "基线读取时间无效".to_string())?;
            validate_windows(&read.windows)?;
        }
        if let Some(reset) = &self.last_reset {
            parse_timestamp(&reset.observed_at)
                .ok_or_else(|| "基线重置观测时间无效".to_string())?;
            if let Some(resets_at) = &reset.resets_at {
                parse_timestamp(resets_at).ok_or_else(|| "基线重置目标时间无效".to_string())?;
            }
        }
        Ok(())
    }
}

fn parse_timestamp(value: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    chrono::DateTime::parse_from_rfc3339(value).ok()
}

fn validate_windows(windows: &[CodexOfficialQuotaWindow]) -> Result<(), String> {
    for window in windows {
        if !window.used_percent.is_finite() || !(0.0..=100.0).contains(&window.used_percent) {
            return Err("基线窗口已用百分比无效".to_string());
        }
    }
    Ok(())
}

/// Compares one successful read against the persisted baseline and returns
/// the updated baseline plus the reset event this read observed.
///
/// A changed account marker wipes the history instead of reporting a
/// misleading reset. Detection watches only the weekly window: usage inside a
/// live window can only decrease when the window restarted, and a changed
/// server-declared reset time moves the schedule. `Early` requires the
/// previously declared time to still be in the future; anything else counts
/// as `Scheduled`.
pub(crate) fn apply_read(
    baseline: Option<CodexQuotaBaseline>,
    marker: Option<String>,
    quota: &CodexOfficialQuota,
    now: &str,
) -> (CodexQuotaBaseline, Option<CodexOfficialQuotaReset>) {
    let previous = baseline.unwrap_or_default();
    let account_changed = previous
        .account_marker
        .as_deref()
        .zip(marker.as_deref())
        .is_some_and(|(old, new)| old != new);

    let detected = if account_changed {
        None
    } else {
        previous
            .last_read
            .as_ref()
            .and_then(|read| detect_weekly_reset(read, quota, now))
    };

    let baseline = CodexQuotaBaseline {
        account_marker: marker,
        last_read: Some(BaselineRead {
            at: quota.at.clone().unwrap_or_else(|| now.to_string()),
            windows: quota.windows.clone(),
        }),
        last_reset: if account_changed {
            None
        } else {
            detected.clone().or(previous.last_reset)
        },
    };
    (baseline, detected)
}

fn detect_weekly_reset(
    previous: &BaselineRead,
    quota: &CodexOfficialQuota,
    now: &str,
) -> Option<CodexOfficialQuotaReset> {
    let previous_window = weekly_window(&previous.windows)?;
    let current_window = weekly_window(&quota.windows)?;

    let usage_dropped = current_window.used_percent < previous_window.used_percent;
    let schedule_moved = previous_window
        .resets_at
        .as_deref()
        .zip(current_window.resets_at.as_deref())
        .is_some_and(|(old, new)| old != new);
    if !usage_dropped && !schedule_moved {
        return None;
    }

    let early = previous_window
        .resets_at
        .as_deref()
        .and_then(parse_timestamp)
        .zip(parse_timestamp(now))
        .is_some_and(|(declared, now)| declared > now);
    Some(CodexOfficialQuotaReset {
        observed_at: quota.at.clone().unwrap_or_else(|| now.to_string()),
        kind: if early {
            CodexOfficialQuotaResetKind::Early
        } else {
            CodexOfficialQuotaResetKind::Scheduled
        },
        resets_at: current_window.resets_at.clone(),
    })
}

fn weekly_window(windows: &[CodexOfficialQuotaWindow]) -> Option<&CodexOfficialQuotaWindow> {
    windows
        .iter()
        .find(|window| window.label == WEEKLY_WINDOW_LABEL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_the_existing_chatgpt_oauth_shape() {
        let credentials = parse_credentials(
            r#"{"auth_mode":"chatgpt","tokens":{"access_token":"token","account_id":"account"}}"#,
        )
        .expect("OAuth credentials");
        assert_eq!(credentials.access_token, "token");
        assert_eq!(credentials.account_id.as_deref(), Some("account"));
        assert!(
            parse_credentials(r#"{"auth_mode":"api","tokens":{"access_token":"key"}}"#).is_none()
        );
        assert!(parse_credentials(
            r#"{"auth_mode":"chatgpt","tokens":{"access_token":"line\nbreak"}}"#
        )
        .is_none());
    }

    #[test]
    fn accepts_the_cache_written_by_the_official_login_flow() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let auth_path = directory.path().join("auth.json");
        crate::official_login::credentials::write_codex_auth(
            &auth_path,
            &crate::official_login::credentials::CodexTokens {
                id_token: Some("id-token".to_string()),
                access_token: "access-token".to_string(),
                refresh_token: "refresh-token".to_string(),
            },
            Some("account-1"),
        )
        .expect("official login writes the Codex cache");

        let credentials =
            parse_credentials(&std::fs::read_to_string(&auth_path).expect("written cache"))
                .expect("quota parser accepts the official-login shape");
        assert_eq!(credentials.access_token, "access-token");
        assert_eq!(credentials.account_id.as_deref(), Some("account-1"));
    }

    #[test]
    fn missing_auth_file_requires_sign_in_without_creating_it() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let auth_path = directory.path().join("missing").join("auth.json");

        let (quota, marker) = fetch(&auth_path);

        assert_eq!(quota.status, CodexOfficialQuotaStatus::SignInRequired);
        assert!(quota.windows.is_empty());
        assert_eq!(marker, None);
        assert!(!auth_path.exists());
    }

    #[test]
    fn normalizes_the_two_server_windows_without_credential_data() {
        let quota = quota_from_http_response(
            200,
            r#"{
                "rate_limit": {
                    "primary_window": {"used_percent": 12.5, "limit_window_seconds": 18000, "reset_at": 1760000000},
                    "secondary_window": {"used_percent": 76.25, "limit_window_seconds": 604800, "reset_at": 1760500000}
                }
            }"#,
            "2026-09-01T00:00:00.000Z".to_string(),
        );
        assert_eq!(quota.status, CodexOfficialQuotaStatus::Available);
        assert!(!quota.stale);
        assert_eq!(quota.windows.len(), 2);
        assert_eq!(quota.windows[0].label, "5 小时");
        assert_eq!(quota.windows[0].used_percent, 12.5);
        assert_eq!(quota.windows[1].label, "7 天");
        assert_eq!(quota.windows[1].used_percent, 76.25);
        assert_eq!(quota.at.as_deref(), Some("2026-09-01T00:00:00.000Z"));
    }

    #[test]
    fn rejects_unusable_quota_payloads_and_maps_authorization_failures() {
        let malformed = quota_from_http_response(200, r#"{"rate_limit":{}}"#, "now".to_string());
        assert_eq!(malformed.status, CodexOfficialQuotaStatus::Unavailable);
        assert!(malformed.windows.is_empty());
        assert_eq!(
            quota_from_http_response(401, "{}", "now".to_string()).status,
            CodexOfficialQuotaStatus::ReauthenticationRequired
        );
        assert_eq!(
            quota_from_http_response(503, "{}", "now".to_string()).status,
            CodexOfficialQuotaStatus::Unavailable
        );
    }

    #[test]
    fn keeps_a_last_successful_read_visible_after_a_failure() {
        let previous = CodexOfficialQuota {
            status: CodexOfficialQuotaStatus::Available,
            windows: vec![CodexOfficialQuotaWindow {
                label: "5 小时".to_string(),
                used_percent: 38.0,
                resets_at: None,
            }],
            at: Some("2026-09-01T00:00:00Z".to_string()),
            stale: false,
            last_reset: None,
        };
        let retained = retain_last_success(
            empty(CodexOfficialQuotaStatus::Unavailable),
            Some(&previous),
        );
        assert_eq!(retained.status, CodexOfficialQuotaStatus::Unavailable);
        assert!(retained.stale);
        assert_eq!(retained.windows, previous.windows);
        assert_eq!(retained.at, previous.at);
    }

    #[test]
    fn stale_quota_is_reused_only_for_the_same_logged_in_account() {
        let quota = weekly_quota("2026-09-01T00:00:00Z", 38.0, None);
        let entries = HashMap::from([(
            "official-profile".to_string(),
            CachedQuota {
                account_marker: "account-a".to_string(),
                quota: quota.clone(),
            },
        )]);

        assert_eq!(
            matching_last_success(&entries, "official-profile", Some("account-a")),
            Some(quota)
        );
        assert_eq!(
            matching_last_success(&entries, "official-profile", Some("account-b")),
            None
        );
        assert_eq!(
            matching_last_success(&entries, "official-profile", None),
            None
        );
    }

    #[test]
    fn names_known_and_server_extension_windows() {
        assert_eq!(quota_window_label(2_592_000), "30 天");
        assert_eq!(quota_window_label(86_400), "1 天");
        assert_eq!(quota_window_label(7_200), "2 小时");
    }

    fn weekly_quota(at: &str, used_percent: f64, resets_at: Option<&str>) -> CodexOfficialQuota {
        CodexOfficialQuota {
            status: CodexOfficialQuotaStatus::Available,
            windows: vec![CodexOfficialQuotaWindow {
                label: WEEKLY_WINDOW_LABEL.to_string(),
                used_percent,
                resets_at: resets_at.map(str::to_string),
            }],
            at: Some(at.to_string()),
            stale: false,
            last_reset: None,
        }
    }

    fn baseline_from(quota: &CodexOfficialQuota, marker: Option<&str>) -> CodexQuotaBaseline {
        let (baseline, _) = apply_read(
            None,
            marker.map(str::to_string),
            quota,
            "2026-09-01T00:00:00Z",
        );
        baseline
    }

    #[test]
    fn first_read_establishes_a_baseline_without_detection() {
        let quota = weekly_quota("2026-09-01T08:00:00Z", 40.0, Some("2026-09-04T00:00:00Z"));

        let (baseline, detected) = apply_read(
            None,
            Some("marker".to_string()),
            &quota,
            "2026-09-01T08:00:00Z",
        );

        assert_eq!(detected, None);
        assert_eq!(baseline.account_marker.as_deref(), Some("marker"));
        let read = baseline.last_read.expect("baseline read");
        assert_eq!(read.at, "2026-09-01T08:00:00Z");
        assert_eq!(read.windows, quota.windows);
        assert_eq!(baseline.last_reset, None);
    }

    #[test]
    fn detects_a_scheduled_reset_when_usage_drops_after_the_declared_time() {
        let previous_quota =
            weekly_quota("2026-09-01T08:00:00Z", 80.0, Some("2026-09-02T00:00:00Z"));
        let baseline = baseline_from(&previous_quota, Some("marker"));
        let quota = weekly_quota("2026-09-02T06:00:00Z", 4.0, Some("2026-09-09T00:00:00Z"));

        let (baseline, detected) = apply_read(
            Some(baseline),
            Some("marker".to_string()),
            &quota,
            "2026-09-02T06:00:00Z",
        );

        let reset = detected.expect("scheduled reset");
        assert_eq!(reset.kind, CodexOfficialQuotaResetKind::Scheduled);
        assert_eq!(reset.observed_at, "2026-09-02T06:00:00Z");
        assert_eq!(reset.resets_at.as_deref(), Some("2026-09-09T00:00:00Z"));
        assert_eq!(baseline.last_reset, Some(reset));
    }

    #[test]
    fn detects_an_early_reset_when_usage_drops_before_the_declared_time() {
        let previous_quota =
            weekly_quota("2026-09-01T08:00:00Z", 70.0, Some("2026-09-05T00:00:00Z"));
        let baseline = baseline_from(&previous_quota, Some("marker"));
        let quota = weekly_quota("2026-09-02T06:00:00Z", 3.0, Some("2026-09-09T00:00:00Z"));

        let (_, detected) = apply_read(
            Some(baseline),
            Some("marker".to_string()),
            &quota,
            "2026-09-02T06:00:00Z",
        );

        let reset = detected.expect("early reset");
        assert_eq!(reset.kind, CodexOfficialQuotaResetKind::Early);
    }

    #[test]
    fn detects_a_moved_schedule_even_with_equal_usage() {
        let previous_quota =
            weekly_quota("2026-09-01T08:00:00Z", 50.0, Some("2026-09-04T00:00:00Z"));
        let baseline = baseline_from(&previous_quota, Some("marker"));
        let quota = weekly_quota("2026-09-02T06:00:00Z", 50.0, Some("2026-09-06T00:00:00Z"));

        let (_, detected) = apply_read(
            Some(baseline),
            Some("marker".to_string()),
            &quota,
            "2026-09-02T06:00:00Z",
        );

        let reset = detected.expect("moved schedule");
        assert_eq!(reset.kind, CodexOfficialQuotaResetKind::Early);
    }

    #[test]
    fn keeps_a_known_reset_when_a_later_read_detects_nothing() {
        let previous_quota =
            weekly_quota("2026-09-01T08:00:00Z", 80.0, Some("2026-09-02T00:00:00Z"));
        let with_reset = baseline_from(&previous_quota, Some("marker"));
        let (with_reset, reset) = apply_read(
            Some(with_reset),
            Some("marker".to_string()),
            &weekly_quota("2026-09-02T06:00:00Z", 4.0, Some("2026-09-09T00:00:00Z")),
            "2026-09-02T06:00:00Z",
        );
        assert!(reset.is_some());
        let (baseline, detected) = apply_read(
            Some(with_reset.clone()),
            Some("marker".to_string()),
            &weekly_quota("2026-09-02T07:00:00Z", 5.0, Some("2026-09-09T00:00:00Z")),
            "2026-09-02T07:00:00Z",
        );

        assert_eq!(detected, None);
        assert_eq!(baseline.last_reset, with_reset.last_reset);
    }

    #[test]
    fn a_changed_account_marker_wipes_the_history_instead_of_detecting() {
        let previous_quota =
            weekly_quota("2026-09-01T08:00:00Z", 80.0, Some("2026-09-02T00:00:00Z"));
        let mut baseline = baseline_from(&previous_quota, Some("old"));
        baseline.last_reset = Some(CodexOfficialQuotaReset {
            observed_at: "2026-09-01T09:00:00Z".to_string(),
            kind: CodexOfficialQuotaResetKind::Scheduled,
            resets_at: None,
        });
        let quota = weekly_quota("2026-09-02T06:00:00Z", 4.0, Some("2026-09-09T00:00:00Z"));

        let (baseline, detected) = apply_read(
            Some(baseline),
            Some("new".to_string()),
            &quota,
            "2026-09-02T06:00:00Z",
        );

        assert_eq!(detected, None);
        assert_eq!(baseline.account_marker.as_deref(), Some("new"));
        assert_eq!(baseline.last_reset, None);
        assert!(baseline.last_read.is_some());
    }

    #[test]
    fn movement_of_other_windows_alone_is_ignored() {
        let previous_quota = CodexOfficialQuota {
            status: CodexOfficialQuotaStatus::Available,
            windows: vec![
                CodexOfficialQuotaWindow {
                    label: "5 小时".to_string(),
                    used_percent: 90.0,
                    resets_at: Some("2026-09-01T10:00:00Z".to_string()),
                },
                CodexOfficialQuotaWindow {
                    label: WEEKLY_WINDOW_LABEL.to_string(),
                    used_percent: 40.0,
                    resets_at: Some("2026-09-04T00:00:00Z".to_string()),
                },
            ],
            at: Some("2026-09-01T08:00:00Z".to_string()),
            stale: false,
            last_reset: None,
        };
        let baseline = baseline_from(&previous_quota, Some("marker"));
        let quota = CodexOfficialQuota {
            status: CodexOfficialQuotaStatus::Available,
            windows: vec![
                CodexOfficialQuotaWindow {
                    label: "5 小时".to_string(),
                    used_percent: 10.0,
                    resets_at: Some("2026-09-01T15:00:00Z".to_string()),
                },
                CodexOfficialQuotaWindow {
                    label: WEEKLY_WINDOW_LABEL.to_string(),
                    used_percent: 42.0,
                    resets_at: Some("2026-09-04T00:00:00Z".to_string()),
                },
            ],
            at: Some("2026-09-02T06:00:00Z".to_string()),
            stale: false,
            last_reset: None,
        };

        let (_, detected) = apply_read(
            Some(baseline),
            Some("marker".to_string()),
            &quota,
            "2026-09-02T06:00:00Z",
        );

        assert_eq!(detected, None);
    }

    #[test]
    fn a_missing_weekly_window_skips_detection_but_still_updates_the_baseline() {
        let previous_quota =
            weekly_quota("2026-09-01T08:00:00Z", 40.0, Some("2026-09-04T00:00:00Z"));
        let baseline = baseline_from(&previous_quota, Some("marker"));
        let quota = CodexOfficialQuota {
            status: CodexOfficialQuotaStatus::Available,
            windows: vec![CodexOfficialQuotaWindow {
                label: "5 小时".to_string(),
                used_percent: 12.0,
                resets_at: None,
            }],
            at: Some("2026-09-02T06:00:00Z".to_string()),
            stale: false,
            last_reset: None,
        };

        let (baseline, detected) = apply_read(
            Some(baseline),
            Some("marker".to_string()),
            &quota,
            "2026-09-02T06:00:00Z",
        );

        assert_eq!(detected, None);
        assert!(baseline.last_read.is_some());
    }

    #[test]
    fn a_previous_window_without_a_declared_time_counts_as_scheduled() {
        let previous_quota = weekly_quota("2026-09-01T08:00:00Z", 90.0, None);
        let baseline = baseline_from(&previous_quota, Some("marker"));
        let quota = weekly_quota("2026-09-02T06:00:00Z", 2.0, Some("2026-09-09T00:00:00Z"));

        let (_, detected) = apply_read(
            Some(baseline),
            Some("marker".to_string()),
            &quota,
            "2026-09-02T06:00:00Z",
        );

        assert_eq!(
            detected.expect("reset").kind,
            CodexOfficialQuotaResetKind::Scheduled
        );
    }

    #[test]
    fn an_absent_marker_still_allows_detection() {
        let previous_quota =
            weekly_quota("2026-09-01T08:00:00Z", 80.0, Some("2026-09-02T00:00:00Z"));
        let baseline = baseline_from(&previous_quota, None);
        let quota = weekly_quota("2026-09-02T06:00:00Z", 4.0, Some("2026-09-09T00:00:00Z"));

        let (_, detected) = apply_read(Some(baseline), None, &quota, "2026-09-02T06:00:00Z");

        assert!(detected.is_some());
    }

    #[test]
    fn the_account_marker_is_a_short_stable_digest() {
        let first = account_marker(Some("account-1")).expect("marker");
        let second = account_marker(Some("account-2")).expect("marker");
        assert_eq!(first.len(), 16);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, second);
        assert_eq!(account_marker(Some("account-1")), Some(first));
        assert_eq!(account_marker(None), None);
    }
}
