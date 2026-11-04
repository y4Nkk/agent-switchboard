use crate::contracts::AppKind;

use crate::ownership::spec::{ProviderSettingSpec, SettingValueType};

/// The built-in identity shared by official and gateway Codex routes.
pub const CODEX_PROVIDER_ID: &str = "openai";
pub const CODEX_PROVIDER_BASE_URL_KEY: &str = "openai_base_url";
pub const CODEX_MODEL_CATALOG_KEY: &str = "model_catalog_json";
/// The Codex sub-agent model key. Its value is derived at render time from
/// the profile's typed `subagentRoute` reference, so it has no scalar editor
/// control; a stored parameter spelling is a retired contract and fails
/// validation.
pub const CODEX_SUBAGENT_MODEL_KEY: &str = "agents.default_subagent_model";
/// Codex's server-side web-search mode. A cross-protocol route cannot carry
/// this Responses-only server tool, so the Codex adapter derives its effective
/// value from the selected route at render time.
pub const CODEX_WEB_SEARCH_KEY: &str = "web_search";

/// Restore identity for Claude gateway backups; stripped on direct/official switches.
pub const CLAUDE_GATEWAY_REVISION_KEY: &str = "env.ASB_CLAUDE_ROUTE_REVISION";

pub(super) const PROVIDER_SETTINGS: &[ProviderSettingSpec] = &[
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: CLAUDE_GATEWAY_REVISION_KEY,
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Codex,
        key: "model",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Codex,
        key: "model_provider",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Codex,
        key: CODEX_PROVIDER_BASE_URL_KEY,
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Codex,
        key: CODEX_MODEL_CATALOG_KEY,
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Codex,
        key: CODEX_SUBAGENT_MODEL_KEY,
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Codex,
        key: "model_context_window",
        value_type: SettingValueType::Integer,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "model",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "availableModels",
        value_type: SettingValueType::StringArray,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_BASE_URL",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_AUTH_TOKEN",
        value_type: SettingValueType::Secret,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_API_KEY",
        value_type: SettingValueType::Secret,
    },
    // Claude Code's documented switch for beta-only request fields. A routed
    // Chat or Responses provider has no target representation for all of
    // those experimental Anthropic features, so the active provider owns the
    // setting and removes it again for a native route.
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_MODEL",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_DEFAULT_HAIKU_MODEL",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_DEFAULT_SONNET_MODEL",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_DEFAULT_OPUS_MODEL",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_DEFAULT_FABLE_MODEL",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.CLAUDE_CODE_SUBAGENT_MODEL",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_DEFAULT_SONNET_MODEL_NAME",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_DEFAULT_OPUS_MODEL_NAME",
        value_type: SettingValueType::String,
    },
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_DEFAULT_FABLE_MODEL_NAME",
        value_type: SettingValueType::String,
    },
    // Historical provider-owned key retained only for one-way cleanup by a
    // projection; it is never read into the current model contract.
    ProviderSettingSpec {
        app: AppKind::Claude,
        key: "env.ANTHROPIC_SMALL_FAST_MODEL",
        value_type: SettingValueType::String,
    },
];
