//! Typed data contracts shared by the Rust core, the switch executor,
//! persistence, and the UI bindings.
//!
//! This module is the single owner of every field list. Nothing else may
//! redefine these shapes.

use serde::{Deserialize, Serialize};

/// The two supported coding clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// Codex model run parameters owned by one profile. Absent fields leave any
/// existing host value untouched. Reasoning effort, summary, and verbosity
/// are general settings owned by the settings page, not profile fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

/// Numbers picked out of one usage-query response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub remaining: Option<f64>,
    pub used: Option<f64>,
    pub total: Option<f64>,
    pub unit: Option<String>,
    /// RFC 3339 UTC timestamp of the query.
    pub at: String,
}

/// A provider profile. It is a small overlay, never a full copy of a user's
/// configuration file. The profile owns the API key used for its configured
/// endpoint and persists it in the application-owned profile store.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderProfile {
    pub id: String,
    pub app: AppKind,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
    /// Optional usage-balance query; application-side metadata that is never
    /// written into any client configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_query: Option<UsageQuery>,
}

/// Editable provider fields. The application assigns the stable profile id
/// when a draft is persisted. Every profile routes to a custom endpoint;
/// official login is not a profile kind (user decision 2026-08-28).
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderDraft {
    pub app: AppKind,
    pub name: String,
    pub base_url: Option<String>,
    pub api_key: String,
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_options: Option<ModelOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
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

/// A single general-configuration overlay value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PatchValue {
    Bool(bool),
    Str(String),
    Number(f64),
    Array(Vec<PatchValue>),
}

impl PatchValue {
    /// Rendering for previews and diagnostics; secret-shaped keys are
    /// redacted by the caller through [`crate::redact`].
    pub fn display(&self) -> String {
        match self {
            PatchValue::Bool(b) => b.to_string(),
            PatchValue::Str(s) => s.clone(),
            PatchValue::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            PatchValue::Array(items) => {
                let parts: Vec<String> = items.iter().map(PatchValue::display).collect();
                format!("[{}]", parts.join(", "))
            }
        }
    }
}

/// One app-owned key/value pair inside a general-configuration overlay.
/// `value: None` removes the key's line from the target file; `Some(value)`
/// writes the line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchEntry {
    pub key: String,
    pub value: Option<PatchValue>,
}

/// General configuration overlay for one client. Only app-owned keys may
/// appear; validation rejects anything else.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonConfigPatch {
    pub app: AppKind,
    pub entries: Vec<PatchEntry>,
}

/// The full side-effect-free input for one switch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchPlan {
    pub app: AppKind,
    pub profile: ProviderProfile,
    pub common: CommonConfigPatch,
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

/// What kind of write produced a switch-log record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SwitchOp {
    /// A profile switch.
    Switch,
    /// Applying the general-configuration overlay on its own.
    CommonSettings,
}

/// One recorded completed switch or restore. The log is the app's own record;
/// it never contains secrets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchLog {
    pub app: AppKind,
    /// Profile switched to; None when the entry records a restore or a
    /// general-settings write.
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    /// SHA-256 hex digest of the file content after the operation.
    pub content_hash: String,
    /// Backup created by the operation; undo restores it.
    pub backup_id: String,
    /// RFC 3339 UTC timestamp.
    pub at: String,
    /// Which write path produced this record.
    pub operation: SwitchOp,
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
    /// or common overlay has since changed or been deleted.
    #[serde(rename_all = "camelCase")]
    ProfileChanged { profile_name: String },
    /// Content still equals a backup restored by the application. A restore is
    /// not a provider match and must not activate a profile row.
    #[serde(rename_all = "camelCase")]
    RestoredBackup { at: String },
    /// Content equals the last general-settings write. No provider routing
    /// claim is implied.
    #[serde(rename_all = "camelCase")]
    MatchesSettings { at: String },
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
        let legacy = r#"{
            "id": "p1", "app": "codex", "name": "中转",
            "model": null, "baseUrl": "https://relay.example/v1", "apiKey": "sk-x",
            "notes": null, "websiteUrl": null
        }"#;
        let profile: ProviderProfile = serde_json::from_str(legacy).expect("legacy profile");
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
}
