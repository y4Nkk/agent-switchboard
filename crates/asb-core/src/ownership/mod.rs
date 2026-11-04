//! Ownership directory: the single authority for which configuration keys
//! Agent Switchboard is allowed to read, patch, or write.
//!
//! Every key outside these sets is host-owned and must be preserved
//! byte-for-byte. Patches referencing a host-owned key are rejected at
//! validation time, never silently dropped.

mod choices;
mod directory;
mod inputs;
mod lookup;
mod provider;
mod resources;
mod spec;
mod toggles;

pub use choices::{CLAUDE_CHOICES, CODEX_CHOICES, CODEX_SUBAGENT_REASONING_EFFORT_KEY};
pub use lookup::{
    choice_spec, default_client_settings, default_provider_parameters, is_owned, is_provider_owned,
    official_setting_directory, owner_for, provider_absent_action, setting_choices, setting_groups,
    setting_spec, setting_specs, setting_toggles, toggle_spec,
};
pub use provider::{
    CLAUDE_GATEWAY_REVISION_KEY, CODEX_MODEL_CATALOG_KEY, CODEX_PROVIDER_BASE_URL_KEY,
    CODEX_PROVIDER_ID, CODEX_SUBAGENT_MODEL_KEY, CODEX_WEB_SEARCH_KEY,
};
pub use resources::{is_claude_extension_path, is_claude_credential_path, is_codex_extension_path};
pub(crate) use resources::preserve_only_path;
pub use spec::{
    ChoiceControl, ChoiceOption, ChoiceSpec, OfficialSettingDisposition, OfficialSettingEntry,
    ProviderAbsentAction, SettingControl, SettingOwner, SettingSpec, SettingValueType, ToggleSpec,
};
pub use toggles::{CLAUDE_TOGGLES, CODEX_TOGGLES};

/// Removed on an explicitly confirmed write; never read as current preferences.
pub const RETIRED_CODEX_CLIENT_KEYS: &[&str] = &[
    "disable_paste_burst", "tools.view_image", "features.unified_exec",
];
pub const RETIRED_CODEX_PROVIDER_KEYS: &[&str] = &["features.personality"];
