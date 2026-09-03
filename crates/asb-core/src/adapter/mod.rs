//! Client adapter boundary.
//!
//! Adapters are pure text transformers: they parse configuration text,
//! compare it against a validated [`SwitchPlan`], produce a redacted
//! [`SwitchPreview`], and render the candidate file text. They never touch
//! the filesystem — `Preview generation does not write a file` is enforced
//! structurally because these functions take `&str` and return `String`.

pub mod claude;
pub mod codex;

use crate::contracts::{AppKind, CommonSettings, SwitchPlan, SwitchPreview};
use serde::{Deserialize, Serialize};

/// An adapter failure with location hints, safe to show in the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterError {
    pub message: String,
    pub line: Option<usize>,
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            Some(line) => write!(f, "{}（第 {} 行）", self.message, line),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for AdapterError {}

/// Scrubs token-shaped runs out of a message so parse errors never leak a
/// secret that happened to sit in the same document.
pub fn scrub_message(message: impl Into<String>) -> String {
    let original = message.into();
    let parts: Vec<String> = original.split_whitespace().map(str::to_string).collect();
    let mut out = original;
    for part in parts {
        let token_like = (part.contains("sk-") || part.len() >= 24)
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.:=/".contains(c));
        if token_like {
            out = out.replace(&part, crate::redact::REDACTED);
        }
    }
    out
}

/// One planned overlay entry for a single owned key.
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayEntry {
    /// Set or update the key to this value.
    Set(crate::contracts::ConfigValue),
    /// Leave whatever is currently there untouched.
    Leave,
    /// Remove the key if it currently exists.
    RemoveIfPresent,
}

/// Computes the preview for `plan` against `current` file text, using the
/// adapter for the plan's app. `backup_dir` is the logical backup location
/// reported in the preview.
pub fn preview(
    current: &str,
    plan: &SwitchPlan,
    backup_dir: &str,
) -> Result<SwitchPreview, AdapterError> {
    crate::validate::validate_plan(&plan.profile, &plan.common).map_err(|e| AdapterError {
        message: scrub_message(e.to_string()),
        line: None,
    })?;
    match plan.app() {
        AppKind::Codex => codex::preview(current, plan, backup_dir),
        AppKind::Claude => claude::preview(current, plan, backup_dir),
    }
}

/// Renders the candidate file text for `plan` against `current`.
pub fn render(current: &str, plan: &SwitchPlan) -> Result<String, AdapterError> {
    crate::validate::validate_plan(&plan.profile, &plan.common).map_err(|e| AdapterError {
        message: scrub_message(e.to_string()),
        line: None,
    })?;
    match plan.app() {
        AppKind::Codex => codex::render(current, plan),
        AppKind::Claude => claude::render(current, plan),
    }
}

/// Renders only the current client's non-default common settings as a
/// self-contained TOML or JSON fragment. This is a read-only editor preview,
/// not a candidate client file: provider and host-owned configuration remain
/// intentionally absent.
pub fn render_common_settings(
    app: AppKind,
    common: &CommonSettings,
) -> Result<String, AdapterError> {
    common.validate_for(app).map_err(|error| AdapterError {
        message: scrub_message(error.to_string()),
        line: None,
    })?;
    match app {
        AppKind::Codex => codex::render_common_settings(common),
        AppKind::Claude => claude::render_common_settings(common),
    }
}

/// Checks that `text` is syntactically valid for `app` without planning
/// anything. The executor uses this to validate a temporary write before it
/// replaces the live file.
pub fn validate_syntax(app: AppKind, text: &str) -> Result<(), AdapterError> {
    match app {
        AppKind::Codex => codex::check_syntax(text),
        AppKind::Claude => claude::check_syntax(text),
    }
}

/// Reads the active routing facts from configuration text. Panics on invalid
/// text; validate syntax first.
pub fn route_state(app: AppKind, text: &str) -> crate::contracts::RouteState {
    match app {
        AppKind::Codex => codex::route_state(text),
        AppKind::Claude => claude::route_state(text),
    }
}

/// Compares the live configuration text against a previous copy (usually a
/// backup) and reports every owned key that differs, with redacted values.
/// `before` is the previous value, `after` the current one; a key present
/// only in the previous copy is reported as removed.
pub fn owned_diff(
    app: AppKind,
    current: &str,
    previous: &str,
) -> Result<Vec<crate::contracts::KeyChange>, AdapterError> {
    match app {
        AppKind::Codex => codex::owned_diff(current, previous),
        AppKind::Claude => claude::owned_diff(current, previous),
    }
}

/// Shared keyed-value diff for the per-app collectors. Keys are compared in
/// sorted order so output is stable.
pub(crate) fn diff_owned_maps(
    current: &std::collections::BTreeMap<String, String>,
    previous: &std::collections::BTreeMap<String, String>,
) -> Vec<crate::contracts::KeyChange> {
    use crate::contracts::{ChangeKind, KeyChange};
    use std::collections::BTreeSet;

    let keys: BTreeSet<&String> = current.keys().chain(previous.keys()).collect();
    keys.into_iter()
        .filter(|key| current.get(*key) != previous.get(*key))
        .map(|key| {
            let after = current.get(key);
            KeyChange {
                key: key.clone(),
                kind: if after.is_some() {
                    ChangeKind::Set
                } else {
                    ChangeKind::Remove
                },
                before: previous.get(key).map(|v| crate::redact::redact(key, v)),
                after: after.map(|v| crate::redact::redact(key, v)),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{CommonSettingValue, ConfigValue};
    use crate::ownership::default_common_settings;

    #[test]
    fn scrub_message_removes_token_shaped_values() {
        let secret = "sk-live-0123456789abcdefghij";
        let scrubbed = scrub_message(format!("invalid value at key = {secret}"));
        assert!(!scrubbed.contains(secret));
        assert!(scrubbed.contains(crate::redact::REDACTED));
    }

    #[test]
    fn scrub_message_keeps_ordinary_text() {
        assert_eq!(
            scrub_message("expected `=`, found `}`"),
            "expected `=`, found `}`"
        );
    }

    #[test]
    fn common_fragment_contains_only_explicit_common_values() {
        let mut settings = default_common_settings(AppKind::Codex);
        settings.settings.insert(
            "hide_agent_reasoning".to_string(),
            CommonSettingValue::Explicit {
                value: ConfigValue::Bool(true),
            },
        );

        let rendered = render_common_settings(AppKind::Codex, &settings).expect("fragment");

        assert!(rendered.contains("hide_agent_reasoning = true"));
        assert!(!rendered.contains("experimental_bearer_token"));
    }

    #[test]
    fn common_fragment_keeps_claude_automatic_values_empty() {
        let settings = default_common_settings(AppKind::Claude);

        assert_eq!(
            render_common_settings(AppKind::Claude, &settings).expect("fragment"),
            "{}"
        );
    }
}
