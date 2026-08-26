//! Ownership table: the single authority for which configuration keys
//! Agent Switchboard is allowed to read, patch, or write.
//!
//! Every key outside these sets is host-owned and must be preserved
//! byte-for-byte. Patches referencing a host-owned key are rejected at
//! validation time, never silently dropped.

use crate::contracts::AppKind;

/// Top-level keys owned by the app for each client.
pub const CODEX_OWNED_KEYS: &[&str] = &[
    "model",
    "model_provider",
    "model_reasoning_effort",
    "model_reasoning_summary",
    "model_verbosity",
    "model_context_window",
    "disable_response_storage",
];

/// Claude Code keys owned by the app (dotted paths into settings.json).
pub const CLAUDE_OWNED_KEYS: &[&str] = &[
    "model",
    "availableModels",
    "env.ANTHROPIC_BASE_URL",
    "env.ANTHROPIC_MODEL",
    "env.ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "env.ANTHROPIC_DEFAULT_SONNET_MODEL",
    "env.ANTHROPIC_DEFAULT_OPUS_MODEL",
];

/// Keys the adapters own but that may only be set through a provider profile,
/// never through a general-config patch. Model routing and run parameters
/// follow the profile being switched to.
pub const PROFILE_EXCLUSIVE_KEYS: &[&str] = &[
    "model",
    "model_provider",
    "model_reasoning_effort",
    "model_reasoning_summary",
    "model_verbosity",
    "model_context_window",
    "availableModels",
    "env.ANTHROPIC_BASE_URL",
    "env.ANTHROPIC_MODEL",
    "env.ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "env.ANTHROPIC_DEFAULT_SONNET_MODEL",
    "env.ANTHROPIC_DEFAULT_OPUS_MODEL",
];

/// Key prefixes owned by the app. Codex manages exactly one provider table,
/// `model_providers.asb`; every other `model_providers.*` table is host-owned.
pub const CODEX_OWNED_PREFIXES: &[&str] = &["model_providers.asb."];

/// Returns true when `key` is app-owned for `app`.
pub fn is_owned(app: AppKind, key: &str) -> bool {
    let (keys, prefixes) = match app {
        AppKind::Codex => (CODEX_OWNED_KEYS, CODEX_OWNED_PREFIXES),
        AppKind::Claude => (CLAUDE_OWNED_KEYS, &[] as &[&str]),
    };
    keys.contains(&key)
        || prefixes
            .iter()
            .any(|p| key.starts_with(p) || key == p.trim_end_matches('.'))
}

/// Returns true when `key` may only be set through a provider profile.
pub fn is_profile_exclusive(key: &str) -> bool {
    PROFILE_EXCLUSIVE_KEYS.contains(&key)
        || key == "model_providers.asb"
        || key.starts_with("model_providers.asb.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_top_level_and_managed_table_are_owned() {
        assert!(is_owned(AppKind::Codex, "model"));
        assert!(is_owned(AppKind::Codex, "model_provider"));
        assert!(is_owned(AppKind::Codex, "model_reasoning_effort"));
        assert!(is_owned(AppKind::Codex, "model_context_window"));
        assert!(is_owned(AppKind::Codex, "model_providers.asb"));
        assert!(is_owned(AppKind::Codex, "model_providers.asb.base_url"));
    }

    #[test]
    fn claude_model_tiers_and_available_models_are_owned() {
        assert!(is_owned(
            AppKind::Claude,
            "env.ANTHROPIC_DEFAULT_HAIKU_MODEL"
        ));
        assert!(is_owned(
            AppKind::Claude,
            "env.ANTHROPIC_DEFAULT_SONNET_MODEL"
        ));
        assert!(is_owned(
            AppKind::Claude,
            "env.ANTHROPIC_DEFAULT_OPUS_MODEL"
        ));
        assert!(is_owned(AppKind::Claude, "availableModels"));
    }

    #[test]
    fn deprecated_claude_model_key_is_not_owned() {
        assert!(!is_owned(AppKind::Claude, "env.ANTHROPIC_SMALL_FAST_MODEL"));
    }

    #[test]
    fn host_keys_are_not_owned() {
        assert!(!is_owned(AppKind::Codex, "threads"));
        assert!(!is_owned(AppKind::Codex, "model_providers.openai.base_url"));
        assert!(!is_owned(AppKind::Claude, "permissions"));
        assert!(!is_owned(AppKind::Claude, "env.HTTP_PROXY"));
    }

    #[test]
    fn profile_exclusive_keys_never_appear_in_general_patches() {
        assert!(is_profile_exclusive("model"));
        assert!(is_profile_exclusive("env.ANTHROPIC_BASE_URL"));
        assert!(is_profile_exclusive("model_providers.asb.base_url"));
        assert!(!is_profile_exclusive("disable_response_storage"));
    }
}
