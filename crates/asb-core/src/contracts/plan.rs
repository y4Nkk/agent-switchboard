use serde::{Deserialize, Serialize};

use crate::contracts::{
    AppKind, AuthenticationScheme, CodexModelSettings, ProviderProfile, RouteMode, SettingsValues,
};

/// The full side-effect-free input for one switch.
///
/// Upstream profile facts remain immutable during client projection. Codex
/// keeps official authentication; Claude receives the projected credential.
/// The execution route is neither persisted nor accepted from the renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct SwitchPlan {
    pub profile: ProviderProfile,
    pub client_settings: SettingsValues,
    scope: ProjectionScope,
    client_route: ClientRoute,
    codex_model_catalog: Option<String>,
    codex_managed_auth: Option<super::CodexManagedAuth>,
    codex_preserve_official_login: bool,
    /// Codex-only common-file fragment; Claude plans never carry one.
    codex_common_fragment: Option<CodexCommonFragment>,
    claude_route_revision: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectionScope {
    Provider,
    ProviderAndClient,
}

/// The user's raw common-file TOML snippet plus its per-profile enable
/// decision. Disabled plans still carry the text so the projection can strip
/// previously merged keys whose values were not changed by the user.
#[derive(Debug, Clone, PartialEq)]
pub struct CodexCommonFragment {
    pub text: String,
    pub enabled: bool,
}
#[derive(Clone, PartialEq, Eq)]
enum ClientRoute {
    Direct,
    Gateway {
        base_url: String,
        bearer_token: String,
    },
}
impl std::fmt::Debug for ClientRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct => f.write_str("Direct"),
            Self::Gateway { .. } => f.write_str("Gateway([redacted])"),
        }
    }
}

impl SwitchPlan {
    /// Builds a direct client plan. Validation still reports an invalid custom
    /// profile that lacks its required upstream protocol.
    pub fn direct(profile: ProviderProfile, client_settings: SettingsValues) -> Self {
        Self {
            profile,
            client_settings,
            scope: ProjectionScope::ProviderAndClient,
            client_route: ClientRoute::Direct,
            codex_model_catalog: None,
            codex_managed_auth: None,
            codex_preserve_official_login: true,
            codex_common_fragment: None,
            claude_route_revision: None,
        }
    }

    /// Builds the client-facing projection for the application's loopback
    /// gateway. Codex uses a path capability; Claude uses a Bearer token.
    pub fn through_gateway(
        profile: ProviderProfile,
        client_settings: SettingsValues,
        base_url: String,
        bearer_token: String,
    ) -> Self {
        Self {
            profile,
            client_settings,
            scope: ProjectionScope::ProviderAndClient,
            client_route: ClientRoute::Gateway {
                base_url,
                bearer_token,
            },
            codex_model_catalog: None,
            codex_managed_auth: None,
            codex_preserve_official_login: true,
            codex_common_fragment: None,
            claude_route_revision: None,
        }
    }
    /// Adds the relative `model_catalog_json` pointer owned by a Codex
    /// third-party route. Official and Claude plans never carry this field.
    pub fn with_codex_model_catalog(mut self, pointer: String) -> Self {
        self.codex_model_catalog = Some(pointer);
        self
    }

    /// Repair reapplies the selected provider without replacing live client
    /// preferences or the client's shared fragment with stored values.
    pub fn for_provider_repair(mut self) -> Self {
        self.scope = ProjectionScope::Provider;
        self
    }

    pub fn writes_client_settings(&self) -> bool {
        self.scope == ProjectionScope::ProviderAndClient
    }

    pub fn with_codex_preserve_official_login(mut self, preserve: bool) -> Self {
        self.codex_preserve_official_login = preserve;
        self
    }
    pub fn codex_preserve_official_login(&self) -> bool {
        self.codex_preserve_official_login
    }
    pub fn codex_gateway_credential(&self) -> Option<&str> {
        if self.profile.app != AppKind::Codex {
            return None;
        }
        match &self.client_route {
            ClientRoute::Gateway { bearer_token, .. } => Some(if bearer_token.is_empty() {
                "asb-local-gateway"
            } else {
                bearer_token
            }),
            ClientRoute::Direct => None,
        }
    }

    pub fn with_codex_managed_auth(mut self, auth: super::CodexManagedAuth) -> Self {
        self.codex_managed_auth = Some(auth);
        self
    }

    pub fn with_codex_common_fragment(mut self, fragment: CodexCommonFragment) -> Self {
        self.codex_common_fragment = Some(fragment);
        self
    }

    pub fn codex_common_fragment(&self) -> Option<&CodexCommonFragment> {
        self.codex_common_fragment.as_ref()
    }

    pub fn codex_managed_auth(&self) -> Option<&super::CodexManagedAuth> {
        self.codex_managed_auth.as_ref()
    }

    pub fn codex_model_catalog(&self) -> Option<&str> {
        self.codex_model_catalog.as_deref()
    }

    /// A local restore discriminator, never used to authorize client requests.
    /// The bearer capability remains stable across Claude provider switches.
    pub fn with_claude_route_revision(mut self, revision: String) -> Self {
        self.claude_route_revision = Some(revision);
        self
    }

    pub fn claude_route_revision(&self) -> Option<&str> {
        self.claude_route_revision.as_deref()
    }

    /// Credential delivery used by the client configuration written for this
    /// execution plan. It is absent for official logins.
    pub fn client_authentication(&self) -> Option<AuthenticationScheme> {
        if self.profile.app == AppKind::Codex
            || self.profile.route_mode != RouteMode::Custom
            || self.profile.connection.claude_native.is_some()
        {
            return None;
        }
        match &self.client_route {
            ClientRoute::Direct => self.profile.upstream_authentication(),
            ClientRoute::Gateway { .. } => Some(AuthenticationScheme::Bearer),
        }
    }

    pub fn is_gateway(&self) -> bool {
        matches!(self.client_route, ClientRoute::Gateway { .. })
    }
    pub fn client_base_url(&self) -> Option<&str> {
        // Official logins stay on the built-in provider unless the plan is an
        // official gateway takeover, which installs the loopback entry.
        if self.profile.route_mode == RouteMode::Official && !self.is_gateway() {
            return None;
        }
        match &self.client_route {
            ClientRoute::Direct => self.profile.base_url.as_deref(),
            ClientRoute::Gateway { base_url, .. } => Some(base_url),
        }
    }
    pub fn client_api_key(&self) -> &str {
        if self.profile.app == AppKind::Codex || self.profile.route_mode == RouteMode::Official {
            return "";
        }
        match &self.client_route {
            ClientRoute::Direct => &self.profile.api_key,
            ClientRoute::Gateway { bearer_token, .. } => bearer_token,
        }
    }
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

/// One user-facing warning with renderer translation coordinates. `text`
/// keeps the product-language rendering as the diagnostic detail and the
/// fallback when the renderer does not know `key`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedMessage {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    pub text: String,
}

impl std::fmt::Display for LocalizedMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

impl LocalizedMessage {
    pub fn new(key: impl Into<String>, params: serde_json::Value, text: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            params: (params != serde_json::Value::Null).then_some(params),
            text: text.into(),
        }
    }
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
    pub warnings: Vec<LocalizedMessage>,
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
    /// Claude model tiers in effect, read only from their current canonical keys.
    pub haiku_model: Option<String>,
    pub sonnet_model: Option<String>,
    pub opus_model: Option<String>,
    pub fable_model: Option<String>,
    /// Claude `availableModels` list, when set.
    pub available_models: Option<Vec<String>>,
    /// Scope-of-effect warnings: facts in this file that may be overridden by
    /// profiles, project-level configuration, or command-line flags.
    pub scope_warnings: Vec<LocalizedMessage>,
}

/// What kind of client-file write a persisted history record describes.
/// A projection may be associated with a provider, or may describe a complete
/// application projection without one (for example a restore record). There
/// is no legacy-only runtime operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WriteOperation {
    Projection,
    /// One client endpoint update that belongs to a gateway-wide port
    /// transaction. It cannot be undone as an isolated client restore.
    GatewayPortChange,
    Restore,
}

/// One recorded completed client-file write. The history is application-owned
/// metadata and never contains secrets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigWriteRecord {
    pub app: AppKind,
    /// Present for a provider projection or its gateway endpoint update.
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
