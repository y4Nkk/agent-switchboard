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
        /// Minutes between automatic re-queries of an expanded usage panel;
        /// 0 keeps the panel manual-only. A required field like every other:
        /// provider files saved before it existed are upgraded to 0 at the
        /// store's read boundary, never defaulted on parse.
        refresh_interval_minutes: u32,
    },
    /// JavaScript source that evaluates to `{ request(input), extract(input) }`.
    /// It is compiled and executed only by the constrained desktop runtime.
    Script {
        source: String,
        /// Minutes between automatic re-queries of an expanded usage panel;
        /// 0 keeps the panel manual-only. A required field like every other:
        /// provider files saved before it existed are upgraded to 0 at the
        /// store's read boundary, never defaulted on parse.
        refresh_interval_minutes: u32,
    },
}

impl UsageQuery {
    /// The auto-refresh cadence is mode-independent, so both variants carry
    /// the same field and expose it through this one accessor.
    pub fn refresh_interval_minutes(&self) -> u32 {
        match self {
            UsageQuery::Declarative {
                refresh_interval_minutes,
                ..
            }
            | UsageQuery::Script {
                refresh_interval_minutes,
                ..
            } => *refresh_interval_minutes,
        }
    }
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

/// The selectable time span for a read-only aggregation of local client
/// session records. The range is evaluated in the local machine's calendar;
/// it does not describe a provider billing window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelUsageRange {
    Today,
    Last7Days,
    Last30Days,
    All,
}

/// Token consumption observed in local client session records, grouped by
/// client and model. `total_tokens` is the sum of fresh input, cache-read
/// input, cache-creation input, and output. This is intentionally separate
/// from a provider balance or a subscription quota: local logs cannot
/// establish remaining allowance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageGroup {
    pub app: AppKind,
    pub model: Option<String>,
    pub input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub session_count: u64,
}

/// Token consumption assigned to one local calendar day. The date uses the
/// local machine's `YYYY-MM-DD` calendar, matching `ModelUsageRange`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageDay {
    pub date: String,
    pub input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

/// A token subtotal that cannot be assigned to a local calendar day because
/// its source record has no usable timestamp. It is separate from the daily
/// trend so the report never silently represents it as dated usage.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageTokens {
    pub input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

/// One client-local reason why a model-usage report may be incomplete. The
/// report still includes any independently readable session records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageIssue {
    pub app: AppKind,
    pub message: String,
}

/// A credential-free, read-only aggregation over local Codex and Claude Code
/// session records. It never represents a provider billing or quota value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageReport {
    pub range: ModelUsageRange,
    /// RFC 3339 UTC timestamp at which the report was generated.
    pub generated_at: String,
    pub groups: Vec<ModelUsageGroup>,
    /// Daily local-calendar totals for records with usable timestamps.
    pub days: Vec<ModelUsageDay>,
    /// Totals included in `groups` but intentionally absent from `days`.
    /// This can be non-zero only for the `All` range, because date-bounded
    /// ranges cannot decide whether an undated record belongs inside them.
    pub unassigned_tokens: ModelUsageTokens,
    pub issues: Vec<ModelUsageIssue>,
}

/// One read request for the local-session usage cache. A forced refresh
/// bypasses the saved snapshot and re-scans the approved session roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelUsageRequest {
    pub range: ModelUsageRange,
    pub force_refresh: bool,
}

/// Whether a local-session usage read came from its persisted snapshot or a
/// scan completed in the current request. It says nothing about provider
/// billing, quota, or remote account state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelUsageFreshness {
    Cached,
    Fresh,
}

/// A local-session report plus the backend-owned snapshot timing. The
/// renderer uses `refresh_after` to schedule foreground-only revalidation;
/// it never invents an independent cache lifetime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsageRead {
    pub report: ModelUsageReport,
    pub freshness: ModelUsageFreshness,
    pub refresh_after: String,
    pub cache_warning: Option<String>,
}

/// One renderer request for persisted, credential-free usage history.
/// Provider history is always resolved against the profile's current query
/// digest by the backend; the renderer never supplies a digest or path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UsageHistoryRequest {
    Provider { profile_id: String },
    Official,
}

/// The meaning of one stored usage-history series. `UsedPercent` is reserved
/// for ratios that have a trustworthy total, including official quotas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UsageHistoryMetric {
    Remaining,
    Used,
    UsedPercent,
}

/// One observed numeric point. Timestamps are RFC 3339 UTC values written
/// only after a real successful provider or official-quota read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageHistoryPoint {
    pub at: String,
    pub value: f64,
}

/// A renderer-safe history series. It intentionally contains no provider
/// endpoint, query source, account identifier, credential, or raw payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageHistorySeries {
    pub id: String,
    pub label: String,
    pub unit: Option<String>,
    pub metric: UsageHistoryMetric,
    pub points: Vec<UsageHistoryPoint>,
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

/// How a locally detected Codex quota reset relates to the previously
/// declared window schedule. `Scheduled` means the previous reset time had
/// already passed; `Early` means usage dropped while it was still ahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexOfficialQuotaResetKind {
    Scheduled,
    Early,
}

/// One locally detected Codex quota reset, observed by comparing consecutive
/// successful official reads of the 7-day window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexOfficialQuotaReset {
    /// RFC 3339 UTC timestamp of the read that first saw the new window.
    pub observed_at: String,
    pub kind: CodexOfficialQuotaResetKind,
    /// The new window's server-declared reset time, when supplied.
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
    /// The most recent locally detected reset known when this read was
    /// recorded. Absent on reads that never ran the comparison.
    pub last_reset: Option<CodexOfficialQuotaReset>,
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

/// A plain configuration value. The array shape exists for provider-owned
/// lists such as `availableModels` and can never pass common-settings
/// validation.
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

/// The application-owned intent for one officially supported common setting.
///
/// `Automatic` deliberately means that Agent Switchboard does not emit the
/// setting into the client configuration. It is not a synthetic host value:
/// the client and active model choose their own documented default. `Explicit`
/// writes exactly the value the user selected, including a value that happens
/// to match a documented default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum CommonSettingValue {
    Automatic,
    Explicit { value: ConfigValue },
}

/// The complete common-setting intent for exactly one client, persisted as
/// `common/{client}.json`. Every supported parameter has one tagged intent so
/// an absent client-file key can never be confused with an explicit false or
/// a guessed application default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommonSettings {
    pub settings: BTreeMap<String, CommonSettingValue>,
}

impl CommonSettings {
    pub fn value(&self, key: &str) -> Option<&CommonSettingValue> {
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
    /// The configuration backup created by the same Codex two-file operation.
    /// Credential backups carry this link; ordinary backups do not.
    #[serde(default)]
    pub linked_backup_id: Option<String>,
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
    fn model_usage_report_serializes_as_a_read_only_client_model_summary() {
        let report = ModelUsageReport {
            range: ModelUsageRange::Last7Days,
            generated_at: "2026-09-03T08:00:00Z".to_string(),
            groups: vec![ModelUsageGroup {
                app: AppKind::Codex,
                model: Some("gpt-5.6-codex".to_string()),
                input_tokens: 120,
                cache_read_input_tokens: 40,
                cache_creation_input_tokens: 20,
                output_tokens: 30,
                total_tokens: 210,
                session_count: 2,
            }],
            days: vec![ModelUsageDay {
                date: "2026-09-03".to_string(),
                input_tokens: 120,
                cache_read_input_tokens: 40,
                cache_creation_input_tokens: 20,
                output_tokens: 30,
                total_tokens: 210,
            }],
            unassigned_tokens: ModelUsageTokens::default(),
            issues: vec![ModelUsageIssue {
                app: AppKind::Claude,
                message: "当前客户端未提供可解析的 token 记录".to_string(),
            }],
        };

        let value = serde_json::to_value(&report).expect("report serializes");
        assert_eq!(value["range"], "last7Days");
        assert_eq!(value["groups"][0]["cacheReadInputTokens"], 40);
        assert_eq!(value["days"][0]["date"], "2026-09-03");
        assert_eq!(value["unassignedTokens"]["totalTokens"], 0);
        assert_eq!(value["issues"][0]["app"], "claude");
    }

    #[test]
    fn model_usage_read_contract_requires_an_explicit_cache_policy() {
        let request = ModelUsageRequest {
            range: ModelUsageRange::Last7Days,
            force_refresh: true,
        };
        assert_eq!(
            serde_json::to_value(request).expect("request serializes"),
            serde_json::json!({"range":"last7Days","forceRefresh":true})
        );
        assert!(
            serde_json::from_value::<ModelUsageRequest>(serde_json::json!({
                "range":"today"
            }))
            .is_err()
        );

        let read = ModelUsageRead {
            report: ModelUsageReport {
                range: ModelUsageRange::Today,
                generated_at: "2026-09-04T08:00:00Z".to_string(),
                groups: Vec::new(),
                days: Vec::new(),
                unassigned_tokens: ModelUsageTokens::default(),
                issues: Vec::new(),
            },
            freshness: ModelUsageFreshness::Cached,
            refresh_after: "2026-09-04T08:05:00Z".to_string(),
            cache_warning: None,
        };
        let value = serde_json::to_value(read).expect("read serializes");
        assert_eq!(value["freshness"], "cached");
        assert_eq!(value["refreshAfter"], "2026-09-04T08:05:00Z");
        assert!(value.get("cacheWarning").is_some());
    }

    #[test]
    fn usage_history_contract_exposes_only_the_requested_scope_and_series_points() {
        let request = UsageHistoryRequest::Provider {
            profile_id: "profile-1".to_string(),
        };
        let series = UsageHistorySeries {
            id: "provider-123".to_string(),
            label: "默认方案余额".to_string(),
            unit: Some("次".to_string()),
            metric: UsageHistoryMetric::Remaining,
            points: vec![UsageHistoryPoint {
                at: "2026-09-03T08:00:00.000Z".to_string(),
                value: 12.5,
            }],
        };

        assert_eq!(
            serde_json::to_value(request).expect("request serializes"),
            serde_json::json!({"kind":"provider","profileId":"profile-1"})
        );
        let value = serde_json::to_value(series).expect("series serializes");
        assert_eq!(value["metric"], "remaining");
        assert_eq!(value["points"][0]["value"], 12.5);
    }

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
            "unit": "USD",
            "refreshIntervalMinutes": 0
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
    fn usage_query_auto_refresh_interval_is_required_and_round_trips() {
        // The interval is a strict required field; upgrading files saved
        // before it existed belongs to the store, not to the contract.
        assert!(serde_json::from_str::<UsageQuery>(
            r#"{"kind":"declarative","url":"u","remainingPath":"x"}"#
        )
        .is_err());
        assert!(
            serde_json::from_str::<UsageQuery>(r#"{"kind":"script","source":"({})"}"#).is_err()
        );

        let timed: UsageQuery =
            serde_json::from_str(r#"{"kind":"script","source":"({})","refreshIntervalMinutes":5}"#)
                .expect("timed usage query");
        assert_eq!(timed.refresh_interval_minutes(), 5);
        assert_eq!(
            serde_json::to_value(&timed).expect("serialized"),
            serde_json::json!({"kind":"script","source":"({})","refreshIntervalMinutes":5})
        );
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
    fn common_settings_store_automatic_or_explicit_values_only() {
        let json = r#"{
            "settings": {
                "model_reasoning_effort": { "mode": "explicit", "value": "high" },
                "hide_agent_reasoning": { "mode": "automatic" }
            }
        }"#;
        let settings: CommonSettings = serde_json::from_str(json).expect("common settings");
        assert_eq!(
            settings.value("model_reasoning_effort"),
            Some(&CommonSettingValue::Explicit {
                value: ConfigValue::Str("high".into())
            })
        );
        assert_eq!(
            settings.value("hide_agent_reasoning"),
            Some(&CommonSettingValue::Automatic)
        );
        // The file shape is fixed: no extra members are accepted.
        assert!(
            serde_json::from_str::<CommonSettings>(r#"{ "settings": {}, "extra": 1 }"#).is_err()
        );
    }
}
