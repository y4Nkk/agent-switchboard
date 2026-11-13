use serde::{Deserialize, Serialize};

use crate::contracts::{
    AppKind, AuthenticationScheme, CodexRouteMode, CodexUpstream, ExplicitMaxOutputTokens,
    ModelOptions, ProviderConnectionOptions, ResponsesOptions, ResponsesRequestMode, RouteMode,
    SettingsValues, UpstreamProtocol, UsageQuery,
};

/// Application-side display metadata. It is carried over from a source row or
/// the offline presets, round-trips through the profile store, never reaches
/// any client configuration, and never participates in routing identity. The
/// structural official/custom owner stays `route_mode`; `category` is only the
/// source's grouping label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDisplay {
    /// Source icon glyph name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Source icon color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<String>,
    /// Source grouping label such as `official` or `third_party`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Source creation timestamp in milliseconds, when declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
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
    #[serde(default)]
    pub connection: ProviderConnectionOptions,
    pub api_key: String,
    /// Explicit credential delivery; absent uses the protocol default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication: Option<AuthenticationScheme>,
    /// Required for custom routes and absent for official routes. It tells the
    /// activation service whether the client connects directly or through the
    /// local protocol gateway.
    pub upstream_protocol: Option<UpstreamProtocol>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responses_options: Option<ResponsesOptions>,
    /// Required only when Codex converts a Responses request to an Anthropic
    /// Messages upstream that requires `max_tokens` even when Codex omits a
    /// per-request output limit. This is an explicit profile setting, never
    /// a gateway fallback.
    pub max_output_tokens: ExplicitMaxOutputTokens,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_options: Option<ModelOptions>,
    pub parameters: SettingsValues,
    /// Claude-only additional settings.json fragment owned by this profile.
    /// It applies while the profile is active and is removed on switch-away
    /// through the profile ownership manifest. Codex profiles never carry it.
    #[serde(default, skip_serializing_if = "crate::claude_common::Extra::is_empty")]
    pub claude_fragment: crate::claude_common::Extra,
    /// Local-only note; never written into any client configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Provider homepage, used for navigation only.
    pub website_url: Option<String>,
    /// Source display columns; application-side metadata only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<ProviderDisplay>,
    /// Optional usage-balance query; application-side metadata that is never
    /// written into any client configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_query: Option<UsageQuery>,
    /// Whole minutes between backend queries of the official Codex
    /// subscription quota; application-side metadata that is never
    /// written into any client configuration. Absent keeps the profile
    /// manual-only, so there is no separate zero representation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub official_quota_refresh_interval_minutes: Option<u32>,
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
    #[serde(default)]
    pub connection: ProviderConnectionOptions,
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication: Option<AuthenticationScheme>,
    pub upstream_protocol: Option<UpstreamProtocol>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responses_options: Option<ResponsesOptions>,
    pub max_output_tokens: ExplicitMaxOutputTokens,
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_options: Option<ModelOptions>,
    pub parameters: SettingsValues,
    #[serde(default, skip_serializing_if = "crate::claude_common::Extra::is_empty")]
    pub claude_fragment: crate::claude_common::Extra,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub website_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<ProviderDisplay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_query: Option<UsageQuery>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub official_quota_refresh_interval_minutes: Option<u32>,
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
            .field("authentication", &self.authentication)
            .field("upstream_protocol", &self.upstream_protocol)
            .field("responses_options", &self.responses_options)
            .field("max_output_tokens", &self.max_output_tokens)
            .field("model_options", &self.model_options)
            .field("parameters", &self.parameters)
            .field("claude_fragment", &self.claude_fragment)
            .field("notes", &self.notes)
            .field("website_url", &self.website_url)
            .field("display", &self.display)
            .field("usage_query", &self.usage_query)
            .field(
                "official_quota_refresh_interval_minutes",
                &self.official_quota_refresh_interval_minutes,
            )
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
            .field("authentication", &self.authentication)
            .field("upstream_protocol", &self.upstream_protocol)
            .field("responses_options", &self.responses_options)
            .field("max_output_tokens", &self.max_output_tokens)
            .field("model", &self.model)
            .field("model_options", &self.model_options)
            .field("parameters", &self.parameters)
            .field("claude_fragment", &self.claude_fragment)
            .field("notes", &self.notes)
            .field("website_url", &self.website_url)
            .field("display", &self.display)
            .field("usage_query", &self.usage_query)
            .field(
                "official_quota_refresh_interval_minutes",
                &self.official_quota_refresh_interval_minutes,
            )
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
            connection: draft.connection,
            api_key: draft.api_key,
            authentication: draft.authentication,
            upstream_protocol: draft.upstream_protocol,
            responses_options: draft.responses_options,
            max_output_tokens: draft.max_output_tokens,
            model_options: draft.model_options,
            parameters: draft.parameters,
            claude_fragment: draft.claude_fragment,
            notes: draft.notes,
            website_url: draft.website_url,
            display: draft.display,
            usage_query: draft.usage_query,
            official_quota_refresh_interval_minutes: draft.official_quota_refresh_interval_minutes,
        }
    }

    /// Managed xAI credentials are only sent to their fixed API origin.
    pub fn upstream_base_url(&self) -> Option<&str> {
        if self.app == AppKind::Codex
            && self.connection.provider_type.as_deref() == Some("xai_oauth")
        {
            Some(crate::contracts::XAI_API_BASE_URL)
        } else {
            self.base_url.as_deref()
        }
    }

    /// Whether the selected protocol or request shape needs the local gateway.
    pub fn requires_gateway(&self) -> bool {
        if self.route_mode != RouteMode::Custom || self.connection.claude_native.is_some() {
            return false;
        }
        if self.app == AppKind::Codex {
            let Some(upstream) = self
                .upstream_protocol
                .and_then(CodexUpstream::from_protocol)
            else {
                return true;
            };
            let request_mode = self
                .responses_options
                .map(|options| options.request_mode)
                .unwrap_or(ResponsesRequestMode::Standard);
            let subagent_route = match &self.model_options {
                Some(ModelOptions::Codex(settings)) => settings.subagent_route.as_ref(),
                None => None,
                Some(ModelOptions::Claude(_)) => {
                    unreachable!("profile validation rejects mismatched options")
                }
            };
            return CodexRouteMode::for_profile(
                upstream,
                request_mode,
                &self.connection,
                self.authentication,
                self.base_url.as_deref(),
                subagent_route,
                false,
            )
            .requires_gateway();
        }
        matches!(&self.model_options, Some(ModelOptions::Claude(settings)) if settings.has_provider_routes())
            || self.requires_protocol_translation()
            || self.connection.requires_gateway(self.base_url.as_deref())
    }

    pub fn requires_protocol_translation(&self) -> bool {
        self.route_mode == RouteMode::Custom
            && self.upstream_protocol != Some(UpstreamProtocol::native_for(self.app))
    }

    pub fn upstream_authentication(&self) -> Option<AuthenticationScheme> {
        if self.route_mode == RouteMode::Official {
            return None;
        }
        self.upstream_protocol
            .map(|protocol| protocol.resolve_authentication(self.authentication))
    }

    /// Whether persisting `draft` over this profile would change any field the
    /// client-file projection and the gateway route consume. Name, notes,
    /// website, usage query, and the official quota interval are
    /// application-side metadata and never qualify; this predicate is the
    /// single owner of that rule.
    pub fn draft_touches_live_configuration(&self, draft: &ProviderDraft) -> bool {
        self.route_mode != draft.route_mode
            || self.model != draft.model
            || self.base_url != draft.base_url
            || self.connection != draft.connection
            || self.api_key != draft.api_key
            || self.authentication != draft.authentication
            || self.upstream_protocol != draft.upstream_protocol
            || self.responses_options != draft.responses_options
            || self.max_output_tokens != draft.max_output_tokens
            || self.model_options != draft.model_options
            || self.parameters != draft.parameters
            || self.claude_fragment != draft.claude_fragment
    }
}

/// Creation and updates have distinct identities; an absent revision cannot
/// reinterpret a failed update as a new provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ProfileSaveTarget {
    Create {
        #[serde(rename = "profileId")]
        profile_id: String,
    },
    Update {
        #[serde(rename = "profileId")]
        profile_id: String,
        #[serde(rename = "expectedFileHash")]
        expected_file_hash: String,
    },
}

impl ProfileSaveTarget {
    pub fn profile_id(&self) -> &str {
        match self {
            Self::Create { profile_id } | Self::Update { profile_id, .. } => profile_id,
        }
    }
}

/// What persisting one provider draft will do. This classification is the
/// single contract for the editor save flow; the interface never re-derives
/// which fields affect live configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileSaveKind {
    /// A new provider with the editing session's stable id; nothing is enabled.
    Create,
    /// The draft equals the stored profile; nothing is written.
    NoChange,
    /// Store the profile only. Either the change is metadata-only, or the
    /// provider is not currently active so live configuration cannot be
    /// affected.
    SaveOnly,
    /// The provider is currently active and effective parameters changed:
    /// store the profile and re-apply the client configuration in one
    /// transaction.
    SaveAndApply,
}

/// Classifies a provider draft against the stored profile and the observed
/// active identity. Callers pass `is_active: false` whenever the draft does
/// not touch live configuration, where the flag cannot change the result.
pub fn classify_profile_save(
    existing: Option<&ProviderProfile>,
    draft: &ProviderDraft,
    is_active: bool,
) -> ProfileSaveKind {
    let Some(profile) = existing else {
        return ProfileSaveKind::Create;
    };
    let candidate = ProviderProfile::from_draft(profile.id.clone(), draft.clone());
    if candidate == *profile {
        return ProfileSaveKind::NoChange;
    }
    if is_active && profile.draft_touches_live_configuration(draft) {
        return ProfileSaveKind::SaveAndApply;
    }
    ProfileSaveKind::SaveOnly
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
    /// Sort position within the client's visible provider list.
    pub position: u64,
    pub route_mode: RouteMode,
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication: Option<AuthenticationScheme>,
    pub upstream_protocol: Option<UpstreamProtocol>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responses_options: Option<ResponsesOptions>,
    pub max_output_tokens: ExplicitMaxOutputTokens,
    pub base_url: Option<String>,
    #[serde(default)]
    pub connection: ProviderConnectionOptions,
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_options: Option<ModelOptions>,
    pub parameters: SettingsValues,
    #[serde(default, skip_serializing_if = "crate::claude_common::Extra::is_empty")]
    pub claude_fragment: crate::claude_common::Extra,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub website_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<ProviderDisplay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_query: Option<UsageQuery>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub official_quota_refresh_interval_minutes: Option<u32>,
}

/// The canonical Codex official-login draft: routing identity only — no
/// endpoint, credential, or catalog. The single owner behind the the source application
/// official-row import and the post-login record creation; both only supply
/// provider parameters.
pub fn codex_official_draft(parameters: SettingsValues) -> ProviderDraft {
    ProviderDraft {
        app: AppKind::Codex,
        route_mode: RouteMode::Official,
        name: "Codex 官方登录".to_string(),
        base_url: None,
        connection: ProviderConnectionOptions::default(),
        api_key: String::new(),
        authentication: None,
        upstream_protocol: None,
        responses_options: None,
        max_output_tokens: None.into(),
        model: None,
        model_options: None,
        parameters,
        claude_fragment: Default::default(),
        notes: None,
        website_url: None,
        display: None,
        usage_query: None,
        official_quota_refresh_interval_minutes: None,
    }
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
            connection: self.connection,
            api_key: self.api_key,
            authentication: self.authentication,
            upstream_protocol: self.upstream_protocol,
            responses_options: self.responses_options,
            max_output_tokens: self.max_output_tokens,
            model_options: self.model_options,
            parameters: self.parameters,
            claude_fragment: self.claude_fragment,
            notes: self.notes,
            website_url: self.website_url,
            display: self.display,
            usage_query: self.usage_query,
            official_quota_refresh_interval_minutes: self.official_quota_refresh_interval_minutes,
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
            authentication: profile.authentication,
            base_url: profile.base_url.clone(),
            connection: profile.connection.clone(),
            model: profile.model.clone(),
            upstream_protocol: profile.upstream_protocol,
            responses_options: profile.responses_options,
            max_output_tokens: profile.max_output_tokens,
            model_options: profile.model_options.clone(),
            parameters: profile.parameters.clone(),
            claude_fragment: profile.claude_fragment.clone(),
            notes: profile.notes.clone(),
            website_url: profile.website_url.clone(),
            display: profile.display.clone(),
            usage_query: profile.usage_query.clone(),
            official_quota_refresh_interval_minutes: profile
                .official_quota_refresh_interval_minutes,
        }
    }
}

/// One provider profile together with the storage revision of its file. The
/// revision lets an editor refuse to overwrite an externally changed provider
/// file, mirroring the optimistic check used for client settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRecord {
    pub profile: ProviderProfile,
    pub position: u64,
    pub file_hash: String,
}

#[cfg(test)]
mod save_target_tests;
