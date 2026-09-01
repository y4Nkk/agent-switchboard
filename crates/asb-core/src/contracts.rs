//! Typed data contracts shared by the Rust core, the switch executor,
//! persistence, and the UI bindings.
//!
//! This module is the single owner of every field list. Nothing else may
//! redefine these shapes.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The two supported coding clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppKind {
    Codex,
    Claude,
}

impl AppKind {
    pub fn config_label(self) -> &'static str {
        match self {
            AppKind::Codex => "~/.codex/config.toml",
            AppKind::Claude => "~/.claude/settings.json",
        }
    }

    /// Directory segment owning this client's persisted application files.
    pub fn dir_name(self) -> &'static str {
        match self {
            AppKind::Codex => "codex",
            AppKind::Claude => "claude",
        }
    }

    /// The one user-global instruction document supported for this client.
    pub fn global_prompt_file_name(self) -> &'static str {
        match self {
            AppKind::Codex => "AGENTS.md",
            AppKind::Claude => "CLAUDE.md",
        }
    }
}

/// One user-global instruction document. The backend owns its absolute target
/// path; the renderer receives only the stable file name, text, and version
/// hash needed to edit it without constructing a filesystem path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalPromptDocument {
    pub app: AppKind,
    pub file_name: String,
    pub content: String,
    pub content_hash: String,
    pub exists: bool,
}

/// How a profile routes a client. The mode is explicit; an empty base URL
/// never implies official routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RouteMode {
    /// Use the client's official login and default endpoint.
    Official,
    /// Route through a user-declared service endpoint.
    Custom,
}

/// Codex model run parameters owned by one profile. An absent context window
/// removes the managed line on switch, so one provider cannot leak it into
/// the next. Reasoning effort, summary, and verbosity are general settings
/// owned by the settings page, not profile fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexModelSettings {
    /// Optional model context window in tokens.
    pub context_window: Option<u64>,
}

/// Claude Code model mapping owned by one profile. The primary model is the
/// profile's `model` field; these are the remaining tiers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaudeModelSettings {
    /// The primary model itself lives on `ProviderProfile::model`.
    pub primary_one_m: bool,
    pub haiku_model: Option<String>,
    pub sonnet_model: Option<String>,
    pub sonnet_one_m: bool,
    pub opus_model: Option<String>,
    pub opus_one_m: bool,
    /// Optional `availableModels` list in settings.json.
    pub available_models: Option<Vec<String>>,
}

/// Per-client model options attached to a profile. The variant must match the
/// profile's app; validation rejects mismatches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ModelOptions {
    Codex(CodexModelSettings),
    Claude(ClaudeModelSettings),
}

/// One explicit usage-balance query mode owned by a profile. Declarative
/// queries use the application's fixed GET/auth/JSON-Pointer behavior;
/// script queries provide the two constrained JavaScript functions evaluated
/// by the desktop runtime. The tag is the only persisted discriminator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UsageQuery {
    /// Request URL; the `{{baseUrl}}` and `{{apiKey}}` placeholders are
    /// substituted at run time. The desktop runtime sends a GET with its
    /// established dual-ecosystem authorization headers.
    Declarative {
        url: String,
        /// JSON Pointer (RFC 6901) into the response body, e.g.
        /// `data/balance`.
        #[serde(skip_serializing_if = "Option::is_none")]
        remaining_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        used_path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        total_path: Option<String>,
        /// Display unit for the extracted numbers, e.g. `USD`.
        #[serde(skip_serializing_if = "Option::is_none")]
        unit: Option<String>,
    },
    /// JavaScript source that evaluates to `{ request(input), extract(input) }`.
    /// It is compiled and executed only by the constrained desktop runtime.
    Script { source: String },
}

/// One named or unnamed set of numbers picked out of a usage-query response.
/// A declarative query always produces exactly one unnamed reading; a script
/// may return several named readings when one provider exposes multiple plans.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UsageReading {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_name: Option<String>,
    pub remaining: Option<f64>,
    pub used: Option<f64>,
    pub total: Option<f64>,
    pub unit: Option<String>,
}

/// Complete result of one usage-query response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub readings: Vec<UsageReading>,
    /// RFC 3339 UTC timestamp of the query.
    pub at: String,
}

/// The renderer-safe status of one Codex official-subscription quota read.
/// OAuth credentials and account identifiers are intentionally absent from
/// this contract; the desktop service owns them for the duration of a single
/// request only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexOfficialQuotaStatus {
    Available,
    SignInRequired,
    ReauthenticationRequired,
    Unavailable,
}

/// One server-declared Codex subscription limit window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexOfficialQuotaWindow {
    /// A user-facing name derived from the server's window duration.
    pub label: String,
    /// Percentage already consumed in the current window, from 0 to 100.
    pub used_percent: f64,
    /// RFC 3339 UTC reset time when supplied by the server.
    pub resets_at: Option<String>,
}

/// A normalized read of the existing Codex ChatGPT-login quota. On a failed
/// refresh, `windows` can retain the most recent in-process successful read
/// and `stale` makes that fact explicit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexOfficialQuota {
    pub status: CodexOfficialQuotaStatus,
    pub windows: Vec<CodexOfficialQuotaWindow>,
    /// RFC 3339 UTC timestamp of the latest successful service response.
    pub at: Option<String>,
    pub stale: bool,
}

/// A provider profile. It is a small overlay, never a full copy of a user's
/// configuration file. `route_mode` is the one routing owner: custom profiles
/// own an endpoint and API key; official profiles represent the client's
/// native login without copying any credential cache.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProfile {
    pub id: String,
    pub app: AppKind,
    pub route_mode: RouteMode,
    pub name: String,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_options: Option<ModelOptions>,
    /// Local-only note; never written into any client configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Provider homepage, used for navigation only.
    pub website_url: Option<String>,
    /// Optional usage-balance query; application-side metadata that is never
    /// written into any client configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_query: Option<UsageQuery>,
}

/// Editable provider fields. The application assigns the stable profile id
/// when a draft is persisted. Routing mode is explicit; it is never inferred
/// from absent endpoint or credential fields.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderDraft {
    pub app: AppKind,
    pub route_mode: RouteMode,
    pub name: String,
    pub base_url: Option<String>,
    pub api_key: String,
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_options: Option<ModelOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub website_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_query: Option<UsageQuery>,
}

impl std::fmt::Debug for ProviderProfile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderProfile")
            .field("id", &self.id)
            .field("app", &self.app)
            .field("route_mode", &self.route_mode)
            .field("name", &self.name)
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("api_key", &crate::redact::REDACTED)
            .field("model_options", &self.model_options)
            .field("notes", &self.notes)
            .field("website_url", &self.website_url)
            .field("usage_query", &self.usage_query)
            .finish()
    }
}

impl std::fmt::Debug for ProviderDraft {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderDraft")
            .field("app", &self.app)
            .field("route_mode", &self.route_mode)
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("api_key", &crate::redact::REDACTED)
            .field("model", &self.model)
            .field("model_options", &self.model_options)
            .field("notes", &self.notes)
            .field("website_url", &self.website_url)
            .field("usage_query", &self.usage_query)
            .finish()
    }
}

impl ProviderProfile {
    pub fn from_draft(id: String, draft: ProviderDraft) -> Self {
        Self {
            id,
            app: draft.app,
            route_mode: draft.route_mode,
            name: draft.name,
            model: draft.model,
            base_url: draft.base_url,
            api_key: draft.api_key,
            model_options: draft.model_options,
            notes: draft.notes,
            website_url: draft.website_url,
            usage_query: draft.usage_query,
        }
    }
}

/// One provider's persisted file: the complete managed state of exactly one
/// supplier, stored as `providers/{client}/{id}.json`. The client association
/// is owned by the containing directory, so the file itself carries no `app`
/// field, and the sort position lives in the file so no separate index
/// exists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderFile {
    /// Stable UUID; must equal the file name's stem.
    pub id: String,
    pub name: String,
    /// Sort position within the client's provider list.
    pub position: u64,
    pub route_mode: RouteMode,
    pub api_key: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_options: Option<ModelOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub website_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_query: Option<UsageQuery>,
}

impl ProviderFile {
    /// Attaches the client association owned by the storage directory.
    pub fn into_profile(self, app: AppKind) -> ProviderProfile {
        ProviderProfile {
            id: self.id,
            app,
            route_mode: self.route_mode,
            name: self.name,
            model: self.model,
            base_url: self.base_url,
            api_key: self.api_key,
            model_options: self.model_options,
            notes: self.notes,
            website_url: self.website_url,
            usage_query: self.usage_query,
        }
    }

    /// Strips the client association and records the file's sort position.
    pub fn from_profile(profile: &ProviderProfile, position: u64) -> Self {
        Self {
            id: profile.id.clone(),
            name: profile.name.clone(),
            position,
            route_mode: profile.route_mode,
            api_key: profile.api_key.clone(),
            base_url: profile.base_url.clone(),
            model: profile.model.clone(),
            model_options: profile.model_options.clone(),
            notes: profile.notes.clone(),
            website_url: profile.website_url.clone(),
            usage_query: profile.usage_query.clone(),
        }
    }
}

/// One provider profile together with the storage revision of its file. The
/// revision lets an editor refuse to overwrite an externally changed provider
/// file, mirroring the optimistic check used for common settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRecord {
    pub profile: ProviderProfile,
    pub file_hash: String,
}

/// A plain configuration value. General settings store one of these for every
/// supported parameter; there is deliberately no null or remove state. The
/// array shape exists for provider-owned lists such as `availableModels` and
/// can never pass common-settings validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    Bool(bool),
    Str(String),
    Number(f64),
    Array(Vec<ConfigValue>),
}

impl ConfigValue {
    /// Rendering for previews and diagnostics; secret-shaped keys are
    /// redacted by the caller through [`crate::redact`].
    pub fn display(&self) -> String {
        match self {
            ConfigValue::Bool(b) => b.to_string(),
            ConfigValue::Str(s) => s.clone(),
            ConfigValue::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            ConfigValue::Array(items) => {
                let parts: Vec<String> = items.iter().map(ConfigValue::display).collect();
                format!("[{}]", parts.join(", "))
            }
        }
    }
}

/// The complete general-parameter values for exactly one client, persisted as
/// `common/{client}.json`. Every supported parameter carries a concrete
/// value; defaults come from the ownership directory, never from this file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommonSettings {
    pub settings: BTreeMap<String, ConfigValue>,
}

impl CommonSettings {
    pub fn value(&self, key: &str) -> Option<&ConfigValue> {
        self.settings.get(key)
    }
}

/// One client's common settings together with the stable revision used for
/// optimistic editor saves. It represents application state only and never a
/// client file preview or write.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonSettingsSnapshot {
    pub settings: CommonSettings,
    pub settings_hash: String,
}

/// A side-effect-free rendering of the current draft's general-settings
/// fragment. It never includes provider routing, credentials, or host-owned
/// configuration.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonSettingsPreview {
    pub app: AppKind,
    pub target: String,
    pub content: String,
}

/// The full side-effect-free input for one switch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchPlan {
    pub profile: ProviderProfile,
    pub common: CommonSettings,
}

impl SwitchPlan {
    /// Client selection has one owner: the selected provider profile.
    pub fn app(&self) -> AppKind {
        self.profile.app
    }
}

/// How one owned key changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Set,
    Remove,
}

/// One changed key with redacted before/after values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyChange {
    pub key: String,
    pub kind: ChangeKind,
    pub before: Option<String>,
    pub after: Option<String>,
}

/// The non-mutating result of planning a switch. Every value the UI shows in
/// a diff is already redacted here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchPreview {
    pub app: AppKind,
    /// Adapter label for the target configuration file. The desktop command
    /// replaces it with the resolved local path before returning it to the UI.
    pub target: String,
    pub changes: Vec<KeyChange>,
    pub warnings: Vec<String>,
    /// Directory where the pre-switch backup will be written.
    pub backup_dir: String,
}

/// Metadata about one backup file.
fn backup_target_existed() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRecord {
    pub id: String,
    pub app: AppKind,
    pub target_path: String,
    pub backup_path: String,
    /// RFC 3339 UTC timestamp.
    pub created_at: String,
    /// SHA-256 hex digest of the backed-up content.
    pub content_hash: String,
    /// Whether the target existed when this snapshot was created. Older backup
    /// metadata always represents an existing target.
    #[serde(default = "backup_target_existed")]
    pub target_existed: bool,
    pub reason: String,
}

/// The currently active routing facts for one client, derived read-only
/// from its configuration text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteState {
    pub app: AppKind,
    /// Routing mode derived from the file: no custom endpoint means official.
    pub route_mode: RouteMode,
    /// Active provider display name, when its table declares one.
    pub provider_name: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    /// Codex custom provider protocol, used to determine whether an import
    /// can be rendered by the current adapter.
    pub wire_api: Option<String>,
    /// Codex run parameters found in the live configuration, when this is a
    /// Codex route. They are imported into the profile that owns them.
    pub codex_model_options: Option<CodexModelSettings>,
    /// Claude model tiers in effect. The Haiku tier falls back to the
    /// deprecated `ANTHROPIC_SMALL_FAST_MODEL` when that is all the file has.
    pub haiku_model: Option<String>,
    pub sonnet_model: Option<String>,
    pub opus_model: Option<String>,
    /// Claude `availableModels` list, when set.
    pub available_models: Option<Vec<String>>,
    /// Scope-of-effect warnings: facts in this file that may be overridden by
    /// profiles, project-level configuration, or command-line flags.
    pub scope_warnings: Vec<String>,
}

/// What kind of client-file write a persisted history record describes.
/// A projection may be associated with a provider, or may describe a complete
/// application projection without one (for example a migrated historical
/// record). There is no legacy-only runtime operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WriteOperation {
    Projection,
    Restore,
}

/// One recorded completed client-file write. The history is application-owned
/// metadata and never contains secrets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigWriteRecord {
    pub app: AppKind,
    /// Present only for a provider projection.
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    /// SHA-256 hex digest of the file content after the operation.
    pub content_hash: String,
    /// Backup created by the operation; undo restores it.
    pub backup_id: String,
    /// RFC 3339 UTC timestamp.
    pub at: String,
    /// The client-file write fact represented by this record.
    pub operation: WriteOperation,
}

/// Whether the current file content still matches one current profile, a
/// restored backup, or the app's last recorded switch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MatchStatus {
    /// Content equals one current profile's expected rendering.
    #[serde(rename_all = "camelCase")]
    MatchesProfile {
        profile_id: String,
        profile_name: String,
    },
    /// Content still equals the last switch output, but the associated profile
    /// or common settings have since changed or been deleted.
    #[serde(rename_all = "camelCase")]
    ProfileChanged { profile_name: String },
    /// Content still equals a backup restored by the application. A restore is
    /// not a provider match and must not activate a profile row.
    #[serde(rename_all = "camelCase")]
    RestoredBackup { at: String },
    /// The app switched this file before, but the content now matches neither
    /// the last switch record nor any profile.
    #[serde(rename_all = "camelCase")]
    ExternallyModified { at: String },
    /// The app never switched this file and no profile matches its content.
    Unmanaged,
    /// The file is missing or unparseable; matching is not decidable.
    Unknown,
}

/// A suggested Provider profile derived read-only from discovered content.
/// Importing it is an explicit later user decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProposal {
    pub app: AppKind,
    pub draft: ProviderDraft,
    pub basis: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_without_usage_query_still_deserialize() {
        let current = r#"{
            "id": "p1", "app": "codex", "routeMode": "custom", "name": "中转",
            "model": null, "baseUrl": "https://relay.example/v1", "apiKey": "sk-x",
            "notes": null, "websiteUrl": null
        }"#;
        let profile: ProviderProfile = serde_json::from_str(current).expect("current profile");
        assert_eq!(profile.usage_query, None);
    }

    #[test]
    fn usage_query_round_trips_as_the_only_tagged_contract() {
        let json = r#"{
            "kind": "declarative",
            "url": "{{baseUrl}}/user/balance",
            "remainingPath": "data/balance",
            "totalPath": "data/total",
            "unit": "USD"
        }"#;
        let query: UsageQuery = serde_json::from_str(json).expect("usage query");
        assert!(matches!(
            query,
            UsageQuery::Declarative {
                remaining_path: Some(ref path),
                ..
            } if path == "data/balance"
        ));
        assert!(serde_json::from_str::<UsageQuery>(r#"{"url":"u","remainingPath":"x"}"#).is_err());
        assert!(serde_json::from_str::<UsageQuery>(
            r#"{"kind":"declarative","url":"u","bogus":1}"#
        )
        .is_err());
        assert!(serde_json::from_str::<UsageQuery>(
            r#"{"kind":"script","source":"({})","url":"u"}"#
        )
        .is_err());
    }

    #[test]
    fn provider_files_carry_no_client_field_and_round_trip_through_profiles() {
        let profile = ProviderProfile {
            id: "0b91a2f4-6c85-4a12-9f0d-2f4a1b3c5d6e".into(),
            app: AppKind::Claude,
            route_mode: RouteMode::Custom,
            name: "中继".into(),
            model: Some("claude-opus-4".into()),
            base_url: Some("https://relay.example".into()),
            api_key: "test-api-key".into(),
            model_options: None,
            notes: None,
            website_url: None,
            usage_query: None,
        };
        let file = ProviderFile::from_profile(&profile, 300);
        let text = serde_json::to_string(&file).expect("provider file serializes");
        assert!(!text.contains("\"app\""));
        assert!(text.contains("\"websiteUrl\":null"));
        let parsed: ProviderFile = serde_json::from_str(&text).expect("provider file parses");
        assert_eq!(parsed.position, 300);
        assert_eq!(parsed.into_profile(AppKind::Claude), profile);
    }

    #[test]
    fn codex_model_settings_reject_unknown_persisted_members() {
        assert!(serde_json::from_str::<ProviderFile>(
            r#"{
                "id":"0b91a2f4-6c85-4a12-9f0d-2f4a1b3c5d6e",
                "name":"中继",
                "position":100,
                "apiKey":"test-api-key",
                "baseUrl":"https://relay.example",
                "model":null,
                "modelOptions":{"kind":"codex","contextWindow":128000,"legacyField":true}
            }"#
        )
        .is_err());
    }

    #[test]
    fn common_settings_store_plain_values_only() {
        let json = r#"{ "settings": { "model_reasoning_effort": "high", "disable_response_storage": true } }"#;
        let settings: CommonSettings = serde_json::from_str(json).expect("common settings");
        assert_eq!(
            settings.value("model_reasoning_effort"),
            Some(&ConfigValue::Str("high".into()))
        );
        assert_eq!(
            settings.value("disable_response_storage"),
            Some(&ConfigValue::Bool(true))
        );
        // The file shape is fixed: no extra members are accepted.
        assert!(
            serde_json::from_str::<CommonSettings>(r#"{ "settings": {}, "extra": 1 }"#).is_err()
        );
    }
}
