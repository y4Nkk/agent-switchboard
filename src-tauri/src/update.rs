//! Manual application-update checks against the project's GitHub releases.
//!
//! One check answers exactly one question: does the latest published release
//! carry a newer version than this running build? Nothing is downloaded,
//! verified, or installed here — the UI opens the release page and the user
//! decides. The check rides the shared reqwest transport in [`crate::probe`].

use crate::probe;
use serde::Serialize;

/// The repository whose releases are the product's only distribution point.
const RELEASES_API_URL: &str =
    "https://api.github.com/repos/y4Nkk/agent-switchboard/releases/latest";

/// Result of one manual update check, surfaced to the UI as-is.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub current_version: String,
    /// Release tag exactly as published, e.g. `v0.2.0`.
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
    /// RFC 3339 UTC timestamp of the check.
    pub checked_at: String,
}

/// Dotted numeric components of a version after trimming one `v` prefix.
fn numeric_components(version: &str) -> Option<Vec<u64>> {
    let trimmed = version.trim().trim_start_matches(['v', 'V']);
    if trimmed.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for part in trimmed.split('.') {
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        parts.push(part.parse::<u64>().ok()?);
    }
    Some(parts)
}

/// Whether `latest` is a newer dotted version than `current`. Components
/// compare numerically (`0.10.0` beats `0.9.0`); shapes outside that grammar
/// fall back to plain inequality after prefix trimming.
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (numeric_components(latest), numeric_components(current)) {
        (Some(mut left), Some(mut right)) => {
            let width = left.len().max(right.len());
            left.resize(width, 0);
            right.resize(width, 0);
            left > right
        }
        _ => {
            fn trimmed(value: &str) -> &str {
                value.trim().trim_start_matches(['v', 'V'])
            }
            trimmed(latest) != trimmed(current)
        }
    }
}

/// Extracts the release tag and page URL from a GitHub `releases/latest` body.
fn parse_release(text: &str) -> Result<(String, String), String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| "更新服务响应不是有效 JSON".to_string())?;
    let tag = value
        .get("tag_name")
        .and_then(|tag| tag.as_str())
        .filter(|tag| !tag.is_empty())
        .ok_or_else(|| "更新服务响应缺少版本号".to_string())?;
    let url = value
        .get("html_url")
        .and_then(|url| url.as_str())
        .filter(|url| url.starts_with("https://"))
        .ok_or_else(|| "更新服务响应缺少发布页地址".to_string())?;
    Ok((tag.to_string(), url.to_string()))
}

/// Checks the latest published release once. Informational only: no download,
/// no install, no retry loop.
pub fn check(current_version: &str) -> Result<UpdateCheck, String> {
    let (status, body) = probe::http_get(RELEASES_API_URL, "Accept: application/vnd.github+json")?;
    if status == 404 {
        return Err("还没有已发布的版本，无法检查更新".to_string());
    }
    if !(200..300).contains(&status) {
        return Err(format!("更新服务返回 HTTP {status}"));
    }
    let (latest_version, release_url) = parse_release(&body)?;
    Ok(UpdateCheck {
        current_version: current_version.to_string(),
        update_available: is_newer(&latest_version, current_version),
        latest_version,
        release_url,
        checked_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_versions_compare_numerically() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("0.10.0", "0.9.0"));
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(is_newer("0.1.0.1", "0.1.0"));
    }

    #[test]
    fn equal_or_older_versions_are_not_newer() {
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.1", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn non_numeric_shapes_fall_back_to_inequality() {
        assert!(is_newer("nightly", "0.1.0"));
        assert!(is_newer("0.1.0-beta", "0.1.0"));
        assert!(!is_newer("0.1.0-beta.2", "0.1.0-beta.2"));
    }

    #[test]
    fn release_bodies_yield_tag_and_page_url() {
        let (tag, url) = parse_release(
            r#"{"tag_name":"v0.2.0","html_url":"https://github.com/y4Nkk/agent-switchboard/releases/tag/v0.2.0"}"#,
        )
        .expect("release");
        assert_eq!(tag, "v0.2.0");
        assert_eq!(
            url,
            "https://github.com/y4Nkk/agent-switchboard/releases/tag/v0.2.0"
        );
    }

    #[test]
    fn malformed_release_bodies_are_rejected() {
        assert!(parse_release("[]").is_err());
        assert!(parse_release(r#"{"html_url":"https://example.com"}"#).is_err());
        assert!(parse_release(r#"{"tag_name":"v1"}"#).is_err());
        assert!(
            parse_release(r#"{"tag_name":"v1","html_url":"http://insecure.example"}"#).is_err()
        );
    }
}
