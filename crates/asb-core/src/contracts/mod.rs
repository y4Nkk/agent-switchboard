//! Typed data contracts shared by the Rust core, the switch executor,
//! persistence, and the UI bindings.
//!
//! This module is the single owner of every field list. Nothing else may
//! redefine these shapes.

mod claude_provider;
mod codex;
mod codex_auth;
pub use codex_auth::CodexManagedAuth;
mod codex_request;
mod money;
pub use codex_request::{CodexAnthropicCacheTtl, CodexPromptCacheRouting, CodexRequestOptions};
pub use money::{decimal_micros, format_usd_micros};
mod connection;
pub use claude_provider::{ClaudeBilling, ClaudePricingModelSource};
mod kinds;
mod model;
mod plan;
mod provider;
mod responses;
mod settings;
mod subagents;
mod usage;
mod model_usage;
pub use model_usage::*;


pub use codex::{
    codex_model_catalog_document, default_model_limits, official_model_limits, CodexCapabilities,
    CodexCatalogEntry, CodexChatEffortMode, CodexChatEffortParameter, CodexChatReasoning,
    CodexChatThinkingParameter, CodexEndpoint, CodexModelRoute, CodexOperation, CodexProviderDraft,
    CodexProviderFile, CodexProviderProfile, CodexProviderRecord, CodexReasoningLevel,
    CodexRouteMode, CodexRouteSnapshot, CodexSubagentRoute, CodexUpstream,
    CODEX_PROVIDER_SCHEMA_VERSION, CODEX_REASONING_LADDER, DEFAULT_CODEX_CAPABILITIES,
    XAI_API_BASE_URL, XAI_OAUTH_PLACEHOLDER,
};
pub use connection::{
    ClaudeApiKeyField, LocalProxyRequestOverrides, ProviderAuthBinding, ProviderConnectionOptions,
    ProviderEndpoint, PROTECTED_OVERRIDE_HEADERS,
};
pub use kinds::{AppKind, AuthenticationScheme, GlobalPromptDocument, RouteMode, UpstreamProtocol};
pub use model::{
    ClaudeModelRole, ClaudeModelRoute, ClaudeModelRoutes, upgrade_claude_model_options,
    ClaudeModelDisplayNames, ClaudeModelSettings, CodexModelSettings, ExplicitMaxOutputTokens,
    ModelOptions,
};
pub use plan::{
    BackupRecord, ChangeKind, CodexCommonFragment, ConfigWriteRecord, KeyChange, LocalizedMessage,
    RouteState, SwitchPlan, SwitchPreview, WriteOperation,
};
pub use provider::{
    classify_profile_save, codex_official_draft, ProfileSaveKind, ProfileSaveTarget, ProviderDisplay, ProviderDraft,
    ProviderFile, ProviderProfile, ProviderRecord,
};
pub use responses::{ResponsesOptions, ResponsesRequestMode};
pub use settings::{
    ClientSettingsSnapshot, ConfigValue, SettingValue, SettingsValues,
    MAX_EXACT_CONFIG_INTEGER,
};
pub use subagents::{
    CodexSubagentKey, CodexSubagentSettings, CodexSubagentSettingsSnapshot,
};
pub use usage::{
    CodexOfficialQuota, CodexOfficialQuotaReset, CodexOfficialQuotaResetKind,
    CodexOfficialQuotaStatus, CodexOfficialQuotaWindow, CodexQuotaReadFailure,
    CodexQuotaReadFailureKind, CodexTransportClass, UsageHistoryMetric, UsageHistoryPoint,
    UsageHistoryRequest, UsageHistorySeries, UsageQuery, UsageReading, UsageReadingInput,
    UsageAvailability, UsageSummary, UsageSnapshot,
};
