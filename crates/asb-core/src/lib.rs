//! asb-core owns the typed contracts and all pure planning logic.
//!
//! Everything in this crate is side-effect free: it transforms configuration
//! text into previews and rendered candidates, and it never touches the
//! filesystem. Live-file mutation belongs exclusively to `asb-switch`.

pub mod adapter;
pub mod ccswitch;
pub mod claude_auth;
pub mod claude_common;
pub mod claude_gemini;
mod claude_model;
pub mod claude_native;
pub mod contracts;
mod config_path;
pub mod discovery;
pub mod endpoint;
pub mod extensions;
pub mod lock;
pub mod ownership;
pub mod provider_transfer;
pub mod provider_conversion;
pub mod redact;
pub mod validate;

pub use discovery::{discover, inspect, DiscoveredFile, DiscoveredState, DiscoveryReport};

pub use ccswitch::{map_row, CcSwitchProposal, CcSwitchProviderDraft, CcSwitchRow, CcSwitchSkip};

pub use adapter::{preview, render, route_state, validate_syntax, AdapterError};
pub use contracts::{
    codex_official_draft, AppKind, AuthenticationScheme, BackupRecord, ChangeKind,
    ClaudeModelSettings, ClientSettingsSnapshot, CodexCapabilities,
    CodexCatalogEntry, CodexEndpoint, CodexModelRoute, CodexModelSettings, CodexOperation,
    CodexProviderDraft, CodexProviderFile, CodexProviderProfile, CodexProviderRecord,
    CodexRouteMode, CodexRouteSnapshot, CodexSubagentKey, CodexSubagentRoute,
    CodexSubagentSettings,
    CodexSubagentSettingsSnapshot, CodexUpstream, ConfigValue,
    ConfigWriteRecord, ExplicitMaxOutputTokens, GlobalPromptDocument, KeyChange,
    ModelOptions, ProfileSaveTarget, ProviderDisplay, ProviderDraft, ProviderFile, ProviderProfile, ProviderRecord,
    RouteMode, RouteState, SettingValue, SettingsValues, SwitchPlan,
    SwitchPreview, UpstreamProtocol, WriteOperation, CODEX_PROVIDER_SCHEMA_VERSION,
};
pub use lock::{classify_lock, LockFileData, LockHolder, LockStatus, PidLiveness};
pub use redact::{redact, REDACTED};
pub use validate::{validate_plan, ValidationError};
