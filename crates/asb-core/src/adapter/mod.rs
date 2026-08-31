//! Client adapter boundary.
//!
//! Adapters are pure text transformers: they parse configuration text,
//! compare it against a validated [`SwitchPlan`], produce a redacted
//! [`SwitchPreview`], and render the candidate file text. They never touch
//! the filesystem — `Preview generation does not write a file` is enforced
//! structurally because these functions take `&str` and return `String`.

pub mod claude;
pub mod codex;

use crate::contracts::{AppKind, PatchValue, SwitchPlan, SwitchPreview};
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
    Set(crate::contracts::PatchValue),
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
    match plan.app {
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
    match plan.app {
        AppKind::Codex => codex::render(current, plan),
        AppKind::Claude => claude::render(current, plan),
    }
}

/// Computes the preview for a general-settings-only apply: the patch's own
/// lines merged into `current`, with no profile routing.
pub fn common_preview(
    current: &str,
    common: &crate::contracts::CommonConfigPatch,
    backup_dir: &str,
) -> Result<SwitchPreview, AdapterError> {
    common.validate().map_err(|e| AdapterError {
        message: scrub_message(e.to_string()),
        line: None,
    })?;
    match common.app {
        AppKind::Codex => {
            codex::preview_entries(current, codex::common_overlay(common), backup_dir)
        }
        AppKind::Claude => {
            let (entries, warnings) = claude::common_overlay(common);
            claude::preview_entries(current, entries, warnings, backup_dir)
        }
    }
}

/// Renders the candidate file text for a general-settings-only apply.
pub fn common_render(
    current: &str,
    common: &crate::contracts::CommonConfigPatch,
) -> Result<String, AdapterError> {
    common.validate().map_err(|e| AdapterError {
        message: scrub_message(e.to_string()),
        line: None,
    })?;
    match common.app {
        AppKind::Codex => codex::render_entries(current, codex::common_overlay(common)),
        AppKind::Claude => {
            let (entries, _) = claude::common_overlay(common);
            claude::render_entries(current, entries)
        }
    }
}

/// Whether `text` currently carries the toggle's applied line. Unparseable
/// text and non-matching values both count as inactive.
pub fn toggle_is_active(app: AppKind, text: &str, key: &str, applied: bool) -> bool {
    let expected = PatchValue::Bool(applied).display();
    let found = match app {
        AppKind::Codex => codex::parse_owned_scalar(text, key),
        AppKind::Claude => claude::parse_owned_scalar(text, key),
    };
    found.is_ok_and(|value| value.as_deref() == Some(expected.as_str()))
}

/// Textual value at an owned dotted path, for settings rows that read the
/// live file instead of matching one expected value.
pub fn owned_scalar(app: AppKind, text: &str, key: &str) -> Result<Option<String>, AdapterError> {
    match app {
        AppKind::Codex => codex::parse_owned_scalar(text, key),
        AppKind::Claude => claude::parse_owned_scalar(text, key),
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
}
