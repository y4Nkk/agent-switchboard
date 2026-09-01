//! Read-only Codex official-subscription quota service.
//!
//! The service is deliberately separate from provider `UsageQuery`: official
//! quota belongs to the existing Codex ChatGPT login, not to a provider API
//! key or endpoint. OAuth values are read only long enough to make the
//! request and never cross the desktop command boundary.

use asb_core::contracts::{CodexOfficialQuota, CodexOfficialQuotaStatus, CodexOfficialQuotaWindow};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";

static CACHE: OnceLock<Mutex<HashMap<String, CodexOfficialQuota>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, CodexOfficialQuota>> {
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

/// Queries the existing Codex login for one official profile. A failed read
/// preserves an earlier in-process result for the same profile and marks it
/// stale; no snapshot is ever written to disk.
pub(crate) fn query(profile_id: &str, auth_path: &Path) -> CodexOfficialQuota {
    let fresh = query_uncached(auth_path);
    if fresh.status == CodexOfficialQuotaStatus::Available {
        if let Ok(mut entries) = cache().lock() {
            entries.insert(profile_id.to_string(), fresh.clone());
        }
        return fresh;
    }

    let previous = cache()
        .lock()
        .ok()
        .and_then(|entries| entries.get(profile_id).cloned());
    retain_last_success(fresh, previous.as_ref())
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

fn query_uncached(auth_path: &Path) -> CodexOfficialQuota {
    let credentials = match std::fs::read_to_string(auth_path)
        .ok()
        .and_then(|text| parse_credentials(&text))
    {
        Some(credentials) => credentials,
        None => return empty(CodexOfficialQuotaStatus::SignInRequired),
    };

    let mut headers = format!(
        "Authorization: Bearer {}\r\nAccept: application/json\r\n",
        credentials.access_token
    );
    if let Some(account_id) = credentials.account_id {
        headers.push_str(&format!("ChatGPT-Account-Id: {account_id}\r\n"));
    }

    match crate::probe::http_get(CODEX_USAGE_URL, &headers) {
        Ok((status, body)) => quota_from_http_response(status, &body, utc_now()),
        Err(_) => empty(CodexOfficialQuotaStatus::Unavailable),
    }
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
        })
        .unwrap_or(failure)
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
    fn missing_auth_file_requires_sign_in_without_creating_it() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let auth_path = directory.path().join("missing").join("auth.json");

        let quota = query_uncached(&auth_path);

        assert_eq!(quota.status, CodexOfficialQuotaStatus::SignInRequired);
        assert!(quota.windows.is_empty());
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
    fn names_known_and_server_extension_windows() {
        assert_eq!(quota_window_label(2_592_000), "30 天");
        assert_eq!(quota_window_label(86_400), "1 天");
        assert_eq!(quota_window_label(7_200), "2 小时");
    }
}
