//! Client adapter boundary.
//!
//! Adapters are pure text transformers: they parse configuration text,
//! compare it against a validated [`SwitchPlan`], produce a redacted
//! [`SwitchPreview`], and render the candidate file text. They never touch
//! the filesystem — `Preview generation does not write a file` is enforced
//! structurally because these functions take `&str` and return `String`.

pub mod claude;
pub mod codex;
mod parameters;
#[cfg(test)]
mod current_contract_tests;
#[cfg(test)]
mod expanded_settings_tests;
#[cfg(test)]
mod provider_repair_tests;

/// Reads only catalog-owned client preferences from a complete live client
/// configuration. Provider, credential, extension, and host-owned keys never
/// enter this projection.
pub fn extract_client_settings(
    app: AppKind,
    text: &str,
) -> Result<SettingsValues, AdapterError> {
    match app {
        AppKind::Codex => codex::extract_client_settings(text),
        AppKind::Claude => claude::extract_client_settings(text),
    }
}
pub use parameters::read_provider_parameters;


use crate::contracts::{AppKind, SettingsValues, SwitchPlan, SwitchPreview};
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
        // Keep the bare credential marker out of the linker's string pool.
        let has_openai_marker = part.as_bytes().windows(3)
            .any(|bytes| matches!(bytes, [b's', b'k', b'-']));
        let token_like = (has_openai_marker || part.len() >= 24)
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
    /// Remove a managed table only after its declared entries have been removed.
    RemoveTableIfEmpty,
}

/// Computes the preview for `plan` against `current` file text, using the
/// adapter for the plan's app. `backup_dir` is the logical backup location
/// reported in the preview.
pub fn preview(
    current: &str,
    plan: &SwitchPlan,
    backup_dir: &str,
) -> Result<SwitchPreview, AdapterError> {
    crate::validate::validate_plan(&plan.profile, &plan.client_settings).map_err(|e| {
        AdapterError {
            message: scrub_message(e.to_string()),
            line: None,
        }
    })?;
    match plan.app() {
        AppKind::Codex => codex::preview(current, plan, backup_dir),
        AppKind::Claude => claude::preview(current, plan, backup_dir),
    }
}

/// Renders the candidate file text for `plan` against `current`.
pub fn render(current: &str, plan: &SwitchPlan) -> Result<String, AdapterError> {
    crate::validate::validate_plan(&plan.profile, &plan.client_settings).map_err(|e| {
        AdapterError {
            message: scrub_message(e.to_string()),
            line: None,
        }
    })?;
    match plan.app() {
        AppKind::Codex => codex::render(current, plan),
        AppKind::Claude => claude::render(current, plan),
    }
}

/// Rewrites only the loopback endpoint in an already-owned gateway client
/// configuration. A gateway port change is deliberately narrower than a
/// provider projection: model and client settings remain exactly as the user
/// last wrote them.
pub fn render_gateway_base_url(
    app: AppKind,
    current: &str,
    base_url: &str,
) -> Result<String, AdapterError> {
    match app {
        AppKind::Codex => codex::render_gateway_base_url(current, base_url),
        AppKind::Claude => claude::render_gateway_base_url(current, base_url),
    }
}

/// Applies only client-owned visual settings to an existing real client
/// document. Automatic values remove their owned key; all other content is
/// preserved by the app-specific document editor.
pub fn render_client_settings_into_file(
    app: AppKind,
    current: &str,
    client_settings: &SettingsValues,
) -> Result<String, AdapterError> {
    client_settings
        .validate_client_settings(app)
        .map_err(|error| AdapterError { message: scrub_message(error.to_string()), line: None })?;
    match app {
        AppKind::Codex => codex::render_client_settings_into_file(current, client_settings),
        AppKind::Claude => claude::render_client_settings_into_file(current, client_settings),
    }
}

/// Resets only the standard client scalars and Codex global subagent settings.
/// Extra configuration is preserved from the live document, never reprojected.
pub fn reset_client_settings(app: AppKind, current: &str) -> Result<String, AdapterError> {
    match app {
        AppKind::Claude => claude::reset_client_settings(current),
        AppKind::Codex => {
            let defaults = crate::ownership::default_client_settings(app);
            let rendered = codex::render_client_settings_into_file(current, &defaults)?;
            codex::render_subagent_settings(&rendered, &crate::contracts::CodexSubagentSettings::automatic())
        }
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

/// Matches provider routing and credentials, independently of model and client
/// settings. Official identity describes the selected route, not OAuth validity.
pub fn matches_provider_identity(current: &str, plan: &SwitchPlan) -> Result<bool, AdapterError> {
    validate_syntax(plan.profile.app, current)?;
    let route = route_state(plan.profile.app, current);
    if route.route_mode != plan.profile.route_mode
        || route.base_url.as_deref() != plan.client_base_url()
    {
        return Ok(false);
    }
    match plan.profile.app {
        AppKind::Codex => codex::matches_provider_settings(current, plan),
        AppKind::Claude => claude::matches_provider_credentials(current, plan),
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

/// Which leaf set a rendered preview diffs: catalog-owned keys only, or
/// every leaf including host-owned ones. The deep client reset deletes
/// unmanaged fields, so its preview must list them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewDiff {
    Owned,
    Full,
}

/// Diff over every leaf, including host-owned keys, for reset and backup previews.
pub fn full_diff(
    app: AppKind,
    current: &str,
    previous: &str,
) -> Result<Vec<crate::contracts::KeyChange>, AdapterError> {
    match app {
        AppKind::Codex => codex::full_diff(current, previous),
        AppKind::Claude => claude::full_diff(current, previous),
    }
}

/// Deletes every leaf no ASB module owns and prunes the containers it
/// empties, so the deep client reset can converge a real file onto the
/// configuration the interface models. Preserved: directory scalars, Claude
/// native and manifest-claimed paths, Codex provider and extension table
/// families, and the sub-agent keys the typed render removes itself.
pub fn remove_unmanaged_entries(app: AppKind, text: &str, codex_fragment: &str) -> Result<String, AdapterError> {
    match app {
        AppKind::Codex => codex::remove_unmanaged(text, codex_fragment),
        AppKind::Claude => claude::remove_unmanaged(text),
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


    #[test]
    fn gateway_endpoint_renderer_changes_only_the_endpoint_slot() {
        let codex = "model = \"user-selected\"
threads = 8
model_provider = \"openai\"
openai_base_url = \"http://127.0.0.1:47821/codex/asb_codex_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef/v1\"
";
        let codex_rendered =
            render_gateway_base_url(AppKind::Codex, codex, "http://127.0.0.1:47822/codex/asb_codex_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef/v1").unwrap();
        assert!(codex_rendered.contains("model = \"user-selected\""));
        assert!(codex_rendered.contains("threads = 8"));
        assert!(codex_rendered.contains("47822/codex/asb_codex_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef/v1"));
        assert!(!codex_rendered.contains("47821"));
        assert!(codex_rendered.contains("openai_base_url"));
        assert_eq!(
            owned_diff(AppKind::Codex, &codex_rendered, codex)
                .unwrap()
                .len(),
            1
        );

        let claude = r#"{"model":"user-selected","env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:47821","HOST_KEY":"keep"}}"#;
        let claude_rendered =
            render_gateway_base_url(AppKind::Claude, claude, "http://127.0.0.1:47822").unwrap();
        assert!(claude_rendered.contains("user-selected"));
        assert!(claude_rendered.contains("HOST_KEY"));
        assert!(claude_rendered.contains("47822"));
    }
}
