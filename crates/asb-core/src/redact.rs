//! Secret redaction.
//!
//! Every value that leaves the core toward previews, diffs, errors, or logs
//! passes through this module. Secret-shaped keys render as one stable
//! token, never a partially visible value.

/// The one redaction marker used everywhere.
pub const REDACTED: &str = "••••••••";

/// Key fragments that mark a value as secret-shaped. Matched
/// case-insensitively against the dotted key path.
const SECRET_MARKERS: &[&str] = &["token", "secret", "api_key", "apikey", "credential", "auth"];

/// True when a key path names something secret-like.
pub fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SECRET_MARKERS.iter().any(|m| lower.contains(m))
}

/// Value prefixes that mark a value as a live secret, whatever the key says.
const SECRET_VALUE_PREFIXES: &[&str] =
    &["sk-", "ghp_", "gho_", "github_pat_", "xox", "AKIA", "AIza"];

/// True when a raw value is secret-shaped: a known token prefix, or a long
/// pure-alphanumeric run that no model name or URL would produce.
pub fn is_secret_value(value: &str) -> bool {
    if SECRET_VALUE_PREFIXES.iter().any(|p| value.starts_with(p)) {
        return true;
    }
    value.len() >= 32 && value.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Renders `value` for display: the stable redaction token when the key is
/// secret-shaped or the value itself looks like a leaked token, the raw text
/// otherwise.
pub fn redact(key: &str, value: &str) -> String {
    if is_secret_key(key) || is_secret_value(value) {
        REDACTED.to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_keys_render_a_stable_token() {
        assert_eq!(redact("env.ANTHROPIC_AUTH_TOKEN", "sk-live-abc"), REDACTED);
        assert_eq!(redact("model_providers.asb.api_key", "zzz"), REDACTED);
        // The token is stable across calls so diffs do not flicker.
        assert_eq!(redact("api_key", "a"), redact("api_key", "b"));
    }

    #[test]
    fn ordinary_values_render_verbatim() {
        assert_eq!(redact("model", "gpt-5"), "gpt-5");
        assert_eq!(
            redact("model", "claude-sonnet-4-20250514"),
            "claude-sonnet-4-20250514"
        );
        assert_eq!(redact("env.ANTHROPIC_BASE_URL", "https://x/"), "https://x/");
    }

    #[test]
    fn token_shaped_values_redact_even_under_ordinary_keys() {
        assert_eq!(
            redact("model_providers.asb.env_key", "sk-live-0123456789abcdef"),
            REDACTED
        );
        let long_key = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
        assert_eq!(redact("some_key", long_key), REDACTED);
    }
}
