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
/// existing host value untouched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexModelSettings {
    /// One of minimal | low | medium | high | xhigh.
    pub reasoning_effort: Option<String>,
    /// One of none | auto | concise | detailed.
    pub reasoning_summary: Option<String>,
    /// One of low | medium | high.
    pub verbosity: Option<String>,
    /// Optional model context window in tokens.
    pub context_window: Option<u64>,
}

/// Claude Code model mapping owned by one profile. The primary model is the
/// profile's `model` field; these are the remaining tiers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeModelSettings {
    pub haiku_model: Option<String>,
    pub sonnet_model: Option<String>,
    pub opus_model: Option<String>,
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

/// A provider profile. It is a small overlay, never a full copy of a user's
/// configuration file. For Codex, `env_key` is the name of the environment
/// variable declared in the managed provider table. It is never a secret
/// value. Claude Code credentials remain owned by its existing login and
/// environment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfile {
    pub id: String,
    pub app: AppKind,
    pub mode: RouteMode,
    pub name: String,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub env_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_options: Option<ModelOptions>,
}

/// Editable provider fields. The application assigns the stable profile id
/// when a draft is persisted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDraft {
    pub app: AppKind,
    pub mode: RouteMode,
    pub name: String,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub env_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_options: Option<ModelOptions>,
}

impl ProviderProfile {
    pub fn from_draft(id: String, draft: ProviderDraft) -> Self {
        Self {
            id,
            app: draft.app,
            mode: draft.mode,
            name: draft.name,
            model: draft.model,
            base_url: draft.base_url,
            env_key: draft.env_key,
            model_options: draft.model_options,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchEntry {
    pub key: String,
    pub value: PatchValue,
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
    /// Host-owned keys present in the file that will remain untouched.
    pub preserved: Vec<String>,
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
    /// Codex custom provider environment-variable name, never its value.
    pub env_key: Option<String>,
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

/// One recorded completed switch or restore. The log is the app's own record;
/// it never contains secrets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchLog {
    pub app: AppKind,
    /// Profile switched to; None when the entry records a restore.
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    /// SHA-256 hex digest of the file content after the operation.
    pub content_hash: String,
    /// Backup created by the operation; undo restores it.
    pub backup_id: String,
    /// RFC 3339 UTC timestamp.
    pub at: String,
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
