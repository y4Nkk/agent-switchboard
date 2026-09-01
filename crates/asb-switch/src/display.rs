//! Redacted display copies of rendered candidates.
//!
//! The executor keeps the real candidate private for hashing and writes;
//! these helpers produce the pretty-printed, secret-free text the UI shows.

use asb_core::{redact, AppKind};
use toml_edit::{DocumentMut, Item, Value as TomlValue};

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

fn redact_toml_value(value: &mut TomlValue, path: &str, force_redaction: bool) {
    if force_redaction || value.as_str().is_some_and(redact::is_secret_value) {
        *value = TomlValue::from(redact::REDACTED);
        return;
    }
    if let Some(array) = value.as_array_mut() {
        for entry in array.iter_mut() {
            redact_toml_value(entry, path, force_redaction);
        }
    } else if let Some(table) = value.as_inline_table_mut() {
        for (key, value) in table.iter_mut() {
            let path = join_key(path, key.get());
            let force_redaction = force_redaction || redact::is_secret_key(&path);
            redact_toml_value(value, &path, force_redaction);
        }
    }
}

fn join_key(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

fn redact_toml_item(item: &mut Item, prefix: &str, force_redaction: bool) {
    if let Some(value) = item.as_value_mut() {
        redact_toml_value(value, prefix, force_redaction);
    } else if let Some(table) = item.as_table_like_mut() {
        for (key, item) in table.iter_mut() {
            let path = join_key(prefix, key.get());
            let force_redaction = force_redaction || redact::is_secret_key(&path);
            redact_toml_item(item, &path, force_redaction);
        }
    }
}

fn redact_toml_content(rendered: &str) -> String {
    let Ok(mut document) = rendered.parse::<DocumentMut>() else {
        return redact::REDACTED.to_string();
    };
    redact_toml_item(document.as_item_mut(), "", false);
    document.to_string()
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
    fn toml_display_hides_secret_shaped_values_under_unknown_host_keys() {
        let secret = "sk-live-0123456789abcdefghij";
        let display = display_content(
            AppKind::Codex,
            &format!("host_extension = \"{secret}\"\nmodel = \"gpt-5\"\n"),
        );

        assert!(!display.contains(secret));
        assert!(display.contains(redact::REDACTED));
        assert!(display.contains("model = \"gpt-5\""));
    }

    #[test]
    fn toml_display_hides_secret_shaped_multiline_host_values() {
        let secret = "sk-live-0123456789abcdefghij";
        let display = display_content(
            AppKind::Codex,
            &format!("host_extension = \"\"\"{secret}\"\"\"\nmodel = \"gpt-5\"\n"),
        );

        assert!(!display.contains(secret));
        assert!(display.contains(redact::REDACTED));
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
