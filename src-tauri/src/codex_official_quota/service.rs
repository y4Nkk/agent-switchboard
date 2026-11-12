use asb_core::contracts::{
    CodexOfficialQuota, CodexOfficialQuotaStatus, CodexOfficialQuotaWindow, CodexQuotaReadFailure,
    CodexTransportClass,
};
use asb_switch::sha256_hex;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
#[derive(Clone)]
pub(super) struct CachedQuota {
    pub(super) account_marker: String,
    pub(super) quota: CodexOfficialQuota,
}

static CACHE: OnceLock<Mutex<HashMap<String, CachedQuota>>> = OnceLock::new();

pub(super) fn cache() -> &'static Mutex<HashMap<String, CachedQuota>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Deserialize)]
struct AuthFile {
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
    tokens: Option<AuthTokens>,
}

#[derive(Deserialize)]
struct AuthTokens {
    access_token: Option<String>,
    pub(super) account_id: Option<String>,
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

pub(super) struct AuthCredentials {
    pub(super) access_token: String,
    pub(super) account_id: Option<String>,
}

/// Queries the existing Codex login for one official profile and returns the
/// quota together with the login's account marker. A failed read preserves an
/// earlier in-process result for the same profile and marks it stale; no
/// snapshot is ever written to disk.
pub(crate) fn query(profile_id: &str, auth_path: &Path) -> (CodexOfficialQuota, Option<String>) {
    let (fresh, marker) = fetch(auth_path);
    if fresh.status == CodexOfficialQuotaStatus::Available {
        store_result(profile_id, marker.as_deref(), &fresh);
        return (fresh, marker);
    }

    let previous = cache()
        .lock()
        .ok()
        .and_then(|entries| matching_quota(&entries, profile_id, marker.as_deref()));
    let retained = retain_last_success(fresh, previous.as_ref());
    store_result(profile_id, marker.as_deref(), &retained);
    (retained, marker)
}

/// Cache-only projection, checked against the current native account identity.
pub(crate) fn cached(profile_id: &str, auth_path: &Path) -> Result<Option<CodexOfficialQuota>, String> {
    let text = match std::fs::read_to_string(auth_path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Some(empty(CodexOfficialQuotaStatus::SignInRequired)));
        }
        Err(_) => return Err("无法确认 Codex 原生账号，未展示缓存额度".into()),
    };
    let Some(credentials) = parse_credentials(&text) else {
        return Ok(Some(empty(CodexOfficialQuotaStatus::SignInRequired)));
    };
    let marker = account_marker(credentials.account_id.as_deref());
    let entries = cache().lock().map_err(|_| "Codex 官方额度缓存不可用")?;
    Ok(matching_quota(&entries, profile_id, marker.as_deref()))
}

pub(crate) fn store_result(profile_id: &str, marker: Option<&str>, quota: &CodexOfficialQuota) {
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

pub(super) fn matching_quota(
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

/// Removes a profile's last successful official-quota result after deletion.
pub(crate) fn invalidate(profile_id: &str) {
    if let Ok(mut entries) = cache().lock() {
        entries.remove(profile_id);
    }
}

pub(super) fn fetch(auth_path: &Path) -> (CodexOfficialQuota, Option<String>) {
    if crate::official_login::observation::require_codex_login(
        &auth_path.with_file_name("config.toml"),
    )
    .is_err()
    {
        return (empty(CodexOfficialQuotaStatus::SignInRequired), None);
    }
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
        Err(failure) => failed(
            CodexOfficialQuotaStatus::Unavailable,
            CodexQuotaReadFailure::transport(transport_class(&failure)),
        ),
    };
    (quota, marker)
}

/// Reduces one request failure to its transport class. Grammar rejection of
/// the app-constructed URL and headers cannot occur, so it lands in the
/// generic class instead of inventing a second kind of quota failure.
pub(crate) fn transport_class(failure: &crate::probe::RequestFailure) -> CodexTransportClass {
    match failure {
        crate::probe::RequestFailure::Transport(transport) => transport.class,
        crate::probe::RequestFailure::Malformed(_) | crate::probe::RequestFailure::Policy(_) => CodexTransportClass::Other,
    }
}

pub(super) fn parse_credentials(content: &str) -> Option<AuthCredentials> {
    let auth: AuthFile = serde_json::from_str(content).ok()?;
    if auth
        .openai_api_key
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        return None;
    }
    crate::official_login::observation::parse_codex_login(content)
        .require()
        .ok()?;
    let tokens = auth.tokens?;
    let access_token = tokens.access_token?.trim().to_string();
    if access_token.is_empty() || has_header_control_characters(&access_token) {
        return None;
    }
    let account_id = match tokens.account_id {
        Some(value) => {
            let value = value.trim().to_string();
            if value.is_empty() || has_header_control_characters(&value) {
                return None;
            }
            Some(value)
        }
        None => None,
    };
    Some(AuthCredentials {
        access_token,
        account_id,
    })
}

/// A short non-reversible digest identifying the logged-in account. It only
/// ever lives inside the local baseline file, so a re-login to a different
/// account can be distinguished from a quota reset.
pub(super) fn account_marker(account_id: Option<&str>) -> Option<String> {
    let account_id = account_id?;
    Some(sha256_hex(account_id)[..16].to_string())
}

fn has_header_control_characters(value: &str) -> bool {
    value.chars().any(char::is_control)
}

pub(crate) fn quota_from_http_response(status: u16, body: &str, at: String) -> CodexOfficialQuota {
    match status {
        200..=299 => quota_from_body(body, at).unwrap_or_else(|| {
            failed(
                CodexOfficialQuotaStatus::Unavailable,
                CodexQuotaReadFailure::INVALID_RESPONSE,
            )
        }),
        401 | 403 => failed(
            CodexOfficialQuotaStatus::ReauthenticationRequired,
            CodexQuotaReadFailure::http_status(status),
        ),
        _ => failed(
            CodexOfficialQuotaStatus::Unavailable,
            CodexQuotaReadFailure::http_status(status),
        ),
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
        failure: None,
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

pub(super) fn quota_window_label(seconds: i64) -> String {
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

/// A read that never attempted the network, so it carries no failure reason.
pub(super) fn empty(status: CodexOfficialQuotaStatus) -> CodexOfficialQuota {
    CodexOfficialQuota {
        status,
        windows: Vec::new(),
        at: None,
        stale: false,
        last_reset: None,
        failure: None,
    }
}

/// A read that attempted the network and failed; the constructor keeps the
/// failure-reason-must-be-present invariant in one place.
pub(crate) fn failed(
    status: CodexOfficialQuotaStatus,
    failure: CodexQuotaReadFailure,
) -> CodexOfficialQuota {
    CodexOfficialQuota {
        status,
        windows: Vec::new(),
        at: None,
        stale: false,
        last_reset: None,
        failure: Some(failure),
    }
}

pub(super) fn retain_last_success(
    failure: CodexOfficialQuota,
    previous: Option<&CodexOfficialQuota>,
) -> CodexOfficialQuota {
    previous
        .filter(|quota| !quota.windows.is_empty())
        .map(|quota| CodexOfficialQuota {
            status: failure.status,
            windows: quota.windows.clone(),
            at: quota.at.clone(),
            stale: true,
            last_reset: quota.last_reset.clone(),
            failure: failure.failure,
        })
        .unwrap_or(failure)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_failures_keep_the_same_successful_windows() {
        let mut success = empty(CodexOfficialQuotaStatus::Available);
        success.at = Some("2026-09-21T00:00:00Z".into());
        success.windows.push(CodexOfficialQuotaWindow {
            label: "5 小时".into(), used_percent: 25.0, resets_at: None,
        });
        let timeout = || failed(
            CodexOfficialQuotaStatus::Unavailable,
            CodexQuotaReadFailure::transport(CodexTransportClass::Timeout),
        );
        let first = retain_last_success(timeout(), Some(&success));
        let second = retain_last_success(timeout(), Some(&first));
        assert_eq!(second.windows, success.windows);
        assert_eq!(second.at, success.at);
        assert_eq!(second.status, CodexOfficialQuotaStatus::Unavailable);
        assert!(second.stale);
        assert_eq!(second.failure, first.failure);
        assert!(second.failure.is_some());
    }

    #[test]
    fn attempted_reads_always_carry_a_failure_and_attempted_never_do() {
        assert_eq!(empty(CodexOfficialQuotaStatus::SignInRequired).failure, None);
        let parsed = quota_from_http_response(200, "{\"rate_limit\":null}", "2026-10-15T00:00:00Z".into());
        assert_eq!(parsed.status, CodexOfficialQuotaStatus::Unavailable);
        assert_eq!(parsed.failure, Some(CodexQuotaReadFailure::INVALID_RESPONSE));
        let rejected = quota_from_http_response(429, "", "2026-10-15T00:00:00Z".into());
        assert_eq!(rejected.failure, Some(CodexQuotaReadFailure::http_status(429)));
        let reauth = quota_from_http_response(401, "", "2026-10-15T00:00:00Z".into());
        assert_eq!(reauth.status, CodexOfficialQuotaStatus::ReauthenticationRequired);
        assert_eq!(reauth.failure, Some(CodexQuotaReadFailure::http_status(401)));
    }

    #[test]
    fn cached_quota_never_crosses_accounts() {
        let entries = HashMap::from([("profile".into(), CachedQuota {
            account_marker: "account-a".into(),
            quota: empty(CodexOfficialQuotaStatus::Unavailable),
        })]);
        assert!(matching_quota(&entries, "profile", Some("account-a")).is_some());
        assert!(matching_quota(&entries, "profile", Some("account-b")).is_none());
        assert!(matching_quota(&entries, "profile", None).is_none());
    }
}
