//! Redacted display copies of rendered candidates.
//!
//! The executor keeps the real candidate private for hashing and writes;
//! these helpers produce the pretty-printed, secret-free text the UI shows.

use asb_core::{redact, AppKind};

fn redact_json_content(rendered: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(rendered) else {
        return redact::redact("content", rendered);
    };
    redact_json_value(&mut value, "");
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| redact::redact("content", rendered))
}

fn redact_json_value(value: &mut serde_json::Value, prefix: &str) {
    match value {
        serde_json::Value::Object(entries) => {
            for (key, item) in entries {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                if redact::is_secret_key(&path) {
                    *item = serde_json::Value::String(redact::REDACTED.to_string());
                } else {
                    redact_json_value(item, &path);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_json_value(item, prefix);
            }
        }
        serde_json::Value::String(text) if redact::is_secret_value(text) => {
            *text = redact::REDACTED.to_string();
        }
        _ => {}
    }
}

fn redact_toml_content(rendered: &str) -> String {
    rendered
        .lines()
        .map(|line| {
            let Some((key, _)) = line.split_once('=') else {
                return line.to_string();
            };
            let key = key.trim();
            if redact::is_secret_key(key) {
                format!("{key} = \"{}\"", redact::REDACTED)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn display_content(app: AppKind, rendered: &str) -> String {
    match app {
        AppKind::Claude => redact_json_content(rendered),
        AppKind::Codex => redact_toml_content(rendered),
    }
}

#[cfg(test)]
mod display_content_tests {
    use super::*;

    #[test]
    fn toml_display_hides_bearer_and_api_key_values() {
        let display = display_content(
            AppKind::Codex,
            "experimental_bearer_token = \"sk-live-codex-secret\"\napi_key = \"host-secret\"\nmodel = \"gpt-5\"\n",
        );
        assert!(!display.contains("sk-live-codex-secret"));
        assert!(!display.contains("host-secret"));
        assert!(display.contains(redact::REDACTED));
        assert!(display.contains("model = \"gpt-5\""));
    }

    #[test]
    fn json_display_hides_nested_tokens_and_preserves_non_secrets() {
        let display = display_content(
            AppKind::Claude,
            r#"{"env":{"ANTHROPIC_AUTH_TOKEN":"sk-live-claude-secret","OTHER":"ok"},"model":"claude-x"}"#,
        );
        assert!(!display.contains("sk-live-claude-secret"));
        assert!(display.contains(redact::REDACTED));
        assert!(display.contains("claude-x"));
        assert!(display.contains("\"OTHER\": \"ok\""));
    }
}
