use super::{
    AuthenticationScheme, ProviderConnectionOptions, ResponsesRequestMode, SettingsValues, UsageQuery,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

mod routing;
pub use routing::{CodexRouteMode, CodexUpstream};
pub const CODEX_PROVIDER_SCHEMA_VERSION: u8 = 2;

/// A validated Codex upstream API root. The value is never an operation URL;
/// operation paths are appended only by the gateway endpoint resolver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CodexEndpoint(pub String);

/// The stored credential of a managed xAI card: never a real key, the
/// gateway resolves the account token per request.
pub const XAI_OAUTH_PLACEHOLDER: &str = "xai_oauth_placeholder";

/// The only upstream a managed xAI (SuperGrok) card may talk to. The
/// editable provider endpoint is ignored so a managed account token can
/// never be sent elsewhere.
pub const XAI_API_BASE_URL: &str = "https://api.x.ai/v1";

mod capabilities;
pub use capabilities::*;

mod reasoning;
pub use reasoning::*;

/// Official published model limits, cited per official model id (OpenAI model
/// pages, 2026-09). A source-stated fact always wins; the editor mirrors this
/// table as `OFFICIAL_MODEL_LIMITS` in src/components/codex-provider-editor/draft.ts.
pub fn official_model_limits(model_id: &str) -> Option<(u64, u64)> {
    match model_id {
        "gpt-6-astra" | "gpt-5.6-sol" | "gpt-5.6-terra" | "gpt-5.6-luna" => {
            Some((1_050_000, 128_000))
        }
        _ => None,
    }
}

/// The single owner of "leave empty" limit resolution: official published
/// limits for a known official id, the generic positive defaults otherwise.
pub fn default_model_limits(model_id: &str) -> (u64, u64) {
    official_model_limits(model_id).unwrap_or((128_000, 8_192))
}

/// A client-visible model. The model directory is application-owned input;
/// it is rendered to Codex's `model_catalog_json` projection during a switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexCatalogEntry {
    pub id: String,
    pub context_window: u64,
    pub max_output_tokens: u64,
    pub function_tools: bool,
    pub custom_tools: bool,
    pub tool_search: bool,
    pub reasoning: bool,
    pub default_reasoning_level: CodexReasoningLevel,
    pub supported_reasoning_levels: Vec<CodexReasoningLevel>,
    pub images: bool,
    pub compact: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_parallel_tool_calls: Option<bool>,
}

impl CodexCatalogEntry {
    /// The default catalog entry for a bare model id: known limits, every
    /// capability enabled, full reasoning ladder. Live-config import and
    /// universal connections share this single fallback semantics.
    pub fn default_entry(id: &str) -> Self {
        let (context_window, max_output_tokens) = default_model_limits(id);
        Self {
            id: id.to_string(),
            context_window,
            max_output_tokens,
            function_tools: true,
            custom_tools: true,
            tool_search: true,
            reasoning: true,
            default_reasoning_level: CodexReasoningLevel::Medium,
            supported_reasoning_levels: CODEX_REASONING_LADDER.to_vec(),
            images: false,
            compact: true,
            display_name: None,
            description: None,
            base_instructions: None,
            supports_parallel_tool_calls: None,
        }
    }

    /// Renders one entry in Codex's external model-catalog schema. Provider
    /// files own the source facts; both the on-disk projection and `/v1/models`
    /// use this exact representation.
    pub fn model_catalog_entry_json(&self, priority: usize) -> Value {
        let display_name = self.display_name.as_deref().unwrap_or(&self.id);
        let description = self.description.as_deref().unwrap_or(display_name);
        let base_instructions = self.base_instructions.as_deref().unwrap_or(
            "You are Codex, a coding agent. You and the user share the same workspace and collaborate to achieve the user's goals.",
        );
        let supports_parallel_tool_calls = self
            .supports_parallel_tool_calls
            .unwrap_or(self.function_tools);
        json!({
            "slug": self.id,
            "display_name": display_name,
            "description": description,
            "base_instructions": base_instructions,
            "default_reasoning_level": self.default_reasoning_level,
            "supported_reasoning_levels": self.supported_reasoning_levels.iter().map(|level| json!({
                "effort": level,
                "description": level.description(),
            })).collect::<Vec<_>>(),
            "shell_type": "shell_command",
            "visibility": "list",
            "supported_in_api": true,
            "priority": 1000 + priority,
            "supports_reasoning_summaries": true,
            "default_reasoning_summary": "none",
            "support_verbosity": false,
            "truncation_policy": {"mode": "bytes", "limit": 10_000},
            "supports_parallel_tool_calls": supports_parallel_tool_calls,
            "supports_image_detail_original": false,
            "context_window": self.context_window,
            "max_output_tokens": self.max_output_tokens,
            "max_context_window": self.context_window,
            "effective_context_window_percent": 95,
            "experimental_supported_tools": [],
            "input_modalities": if self.images { vec!["text", "image"] } else { vec!["text"] },
            "supports_search_tool": self.tool_search,
            "supports_function_tools": self.function_tools,
            "supports_custom_tools": self.custom_tools,
            "supports_tool_search": self.tool_search,
            "supports_reasoning": self.reasoning,
            "supports_compact": self.compact,
        })
    }
}

/// The one client-facing Codex model directory projection. A provider file
/// validates its catalog before writing this document; the active route
/// snapshot is built from that same validated catalog.
pub fn codex_model_catalog_document(catalog: &[CodexCatalogEntry]) -> Value {
    json!({
        "models": catalog
            .iter()
            .enumerate()
            .map(|(priority, entry)| entry.model_catalog_entry_json(priority))
            .collect::<Vec<_>>(),
    })
}

/// One explicit client-model to upstream-model binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexModelRoute {
    pub client_model: String,
    pub upstream_model: String,
}

/// The model a Codex sub-agent runs on, resolved against another provider
/// profile's catalog. This is the only form the subagent model takes: there
/// is no in-provider spelling, so every subagent model routes through the
/// local gateway by profile reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexSubagentRoute {
    pub profile_id: String,
    pub model: String,
}

impl CodexSubagentRoute {
    /// The single wire spelling written into `config.toml` and echoed back by
    /// Codex in request bodies. The first slash separates the profile id; a
    /// model id may itself contain further slashes.
    pub const WIRE_PREFIX: &'static str = "asb:";

    pub fn wire_id(&self) -> String {
        format!("{}{}/{}", Self::WIRE_PREFIX, self.profile_id, self.model)
    }

    /// Structural parse of the wire spelling. Format authority for the
    /// profile id lives in [`Self::has_canonical_profile_id`].
    pub fn parse_wire_id(value: &str) -> Option<Self> {
        let rest = value.strip_prefix(Self::WIRE_PREFIX)?;
        let slash = rest.find('/')?;
        if slash == 0 {
            return None;
        }
        let (profile_id, model) = rest.split_at(slash);
        let model = &model[1..];
        if model.is_empty() {
            return None;
        }
        Some(Self {
            profile_id: profile_id.to_string(),
            model: model.to_string(),
        })
    }

    /// Profile ids are the hyphenated UUIDs the store assigns. The check is
    /// deliberately local: cross-file existence stays with the store.
    pub fn has_canonical_profile_id(&self) -> bool {
        let bytes = self.profile_id.as_bytes();
        bytes.len() == 36
            && bytes[8] == b'-'
            && bytes[13] == b'-'
            && bytes[18] == b'-'
            && bytes[23] == b'-'
            && bytes
                .iter()
                .enumerate()
                .all(|(index, byte)| matches!(index, 8 | 13 | 18 | 23) || byte.is_ascii_hexdigit())
    }
}

/// The sole persistent third-party Codex profile. Official login is a separate
/// generic-store record, so it is never encoded here: a third-party file
/// always means a custom route.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexProviderProfile {
    pub id: String,
    pub name: String,
    pub route_mode: CodexRouteMode,
    pub endpoint: CodexEndpoint,
    pub api_key: String,
    /// Optional explicit upstream credential delivery. Missing values use the
    /// protocol default; non-native Responses authentication requires the
    /// local gateway because Codex's config.toml cannot express it.
    #[serde(default)]
    pub authentication: Option<AuthenticationScheme>,
    /// Connection behavior imported from the source application or edited by the shared
    /// provider machinery. It is never written as an arbitrary client key.
    #[serde(default)]
    pub connection: ProviderConnectionOptions,
    pub upstream: CodexUpstream,
    /// The native Responses request projection is an explicit Codex routing
    /// fact. It is never inferred from the selected upstream.
    pub request_mode: ResponsesRequestMode,
    pub default_model: String,
    pub catalog: Vec<CodexCatalogEntry>,
    pub model_routes: Vec<CodexModelRoute>,
    /// The sub-agent model route. Present only when this profile's sub-agents
    /// run on a referenced profile's model; its wire id resolves at the
    /// gateway, which every subagent-routed profile goes through.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subagent_route: Option<CodexSubagentRoute>,
    pub capabilities: CodexCapabilities,
}

/// Editable Codex third-party provider data. The application assigns the
/// stable UUID on creation; this type deliberately has no route-mode or
/// official-login fields because an official Codex route is not a profile.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexProviderDraft {
    pub name: String,
    pub endpoint: CodexEndpoint,
    pub api_key: String,
    #[serde(default)]
    pub authentication: Option<AuthenticationScheme>,
    #[serde(default)]
    pub connection: ProviderConnectionOptions,
    pub upstream: CodexUpstream,
    pub request_mode: ResponsesRequestMode,
    pub default_model: String,
    pub catalog: Vec<CodexCatalogEntry>,
    pub model_routes: Vec<CodexModelRoute>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subagent_route: Option<CodexSubagentRoute>,
    pub capabilities: CodexCapabilities,
    pub parameters: SettingsValues,
    pub notes: Option<String>,
    pub website_url: Option<String>,
    pub usage_query: Option<UsageQuery>,
}

impl std::fmt::Debug for CodexProviderDraft {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CodexProviderDraft")
            .field("name", &self.name)
            .field("endpoint", &self.endpoint)
            .field("api_key", &crate::redact::REDACTED)
            .field("authentication", &self.authentication)
            .field("upstream", &self.upstream)
            .field("default_model", &self.default_model)
            .field("catalog", &self.catalog)
            .field("model_routes", &self.model_routes)
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}

/// The only on-disk Codex third-party provider format. Position and local UI
/// metadata are storage concerns; the embedded profile owns every upstream
/// routing, model, and capability fact.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexProviderFile {
    pub schema_version: u8,
    pub position: u64,
    pub profile: CodexProviderProfile,
    pub parameters: SettingsValues,
    pub notes: Option<String>,
    pub website_url: Option<String>,
    pub usage_query: Option<UsageQuery>,
}

impl std::fmt::Debug for CodexProviderFile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CodexProviderFile")
            .field("schema_version", &self.schema_version)
            .field("profile_id", &self.profile.id)
            .field("profile_name", &self.profile.name)
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexProviderRecord {
    pub profile: CodexProviderProfile,
    pub position: u64,
    pub parameters: SettingsValues,
    pub notes: Option<String>,
    pub website_url: Option<String>,
    pub usage_query: Option<UsageQuery>,
    pub file_hash: String,
}

impl std::fmt::Debug for CodexProviderProfile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CodexProviderProfile")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("route_mode", &self.route_mode)
            .field("endpoint", &self.endpoint)
            .field("api_key", &crate::redact::REDACTED)
            .field("upstream", &self.upstream)
            .field("default_model", &self.default_model)
            .field("catalog", &self.catalog)
            .field("model_routes", &self.model_routes)
            .field("capabilities", &self.capabilities)
            .finish()
    }
}

impl CodexProviderProfile {
    pub fn validate(&self) -> Result<(), String> {
        validate::profile(self)
    }

    /// Explicit aliases win; an unmapped catalog model keeps its identity
    /// for every protocol. The request boundary selects the default only
    /// when the client omitted a model.
    pub fn resolve_model(&self, requested: &str) -> Result<String, String> {
        resolve_catalog_model(&self.catalog, &self.model_routes, requested)
    }
}

fn resolve_catalog_model(
    catalog: &[CodexCatalogEntry],
    routes: &[CodexModelRoute],
    requested: &str,
) -> Result<String, String> {
    required(requested, "请求模型")?;
    if !catalog.iter().any(|entry| entry.id == requested) {
        return Err(format!("请求模型不在已激活 Codex 目录中：{requested}"));
    }
    Ok(routes
        .iter()
        .find(|route| route.client_model == requested)
        .map(|route| route.upstream_model.clone())
        .unwrap_or_else(|| requested.to_string()))
}

pub(super) fn required(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{field}不能为空"))
    } else {
        Ok(())
    }
}

mod file;
mod validate;

/// An immutable, credential-bearing snapshot created from a validated profile
/// at the instant a request is accepted. It is never persisted in gateway
/// state and therefore cannot be reconstructed from only a capability token.
#[derive(Clone)]
pub struct CodexRouteSnapshot {
    pub revision: String,
    pub provider_id: String,
    pub endpoint: CodexEndpoint,
    pub api_key: String,
    pub authentication: Option<AuthenticationScheme>,
    pub connection: ProviderConnectionOptions,
    pub upstream: CodexUpstream,
    pub request_mode: ResponsesRequestMode,
    pub default_model: String,
    pub catalog: Vec<CodexCatalogEntry>,
    pub model_routes: Vec<CodexModelRoute>,
    pub capabilities: CodexCapabilities,
}

impl CodexRouteSnapshot {
    pub fn from_profile(profile: &CodexProviderProfile, revision: String) -> Result<Self, String> {
        profile.validate()?;
        required(&revision, "路由修订")?;
        Ok(Self {
            revision,
            provider_id: profile.id.clone(),
            endpoint: profile.endpoint.clone(),
            api_key: profile.api_key.clone(),
            authentication: profile.authentication,
            connection: profile.connection.clone(),
            upstream: profile.upstream,
            request_mode: profile.request_mode,
            default_model: profile.default_model.clone(),
            catalog: profile.catalog.clone(),
            model_routes: profile.model_routes.clone(),
            capabilities: profile.capabilities.clone(),
        })
    }

    /// The official ChatGPT Codex backend as a takeover route source. Its
    /// model names never belong to an ASB catalog: admission keeps the
    /// client's model identity and the backend remains the capability
    /// authority. Credentials stay in the managed-account store, never here.
    pub fn official(provider_id: &str, endpoint: &str, revision: String) -> Result<Self, String> {
        required(provider_id, "供应商标识")?;
        required(endpoint, "官方端点")?;
        required(&revision, "路由修订")?;
        Ok(Self {
            revision,
            provider_id: provider_id.to_string(),
            endpoint: CodexEndpoint(endpoint.to_string()),
            api_key: String::new(),
            authentication: Some(AuthenticationScheme::Bearer),
            connection: ProviderConnectionOptions::default(),
            upstream: CodexUpstream::Responses,
            request_mode: ResponsesRequestMode::Standard,
            default_model: String::new(),
            catalog: Vec::new(),
            model_routes: Vec::new(),
            capabilities: CodexCapabilities {
                responses: true,
                compact: true,
                models: true,
                chat_completions: false,
                alpha_search: true,
                image_generation: true,
                image_edit: true,
                automatic_operations: Default::default(),
                function_tools: true,
                custom_tools: true,
                tool_search: true,
                reasoning: true,
                chat_reasoning: CodexChatReasoning::Unsupported,
            },
        })
    }

    pub fn resolve_model(&self, requested: &str) -> Result<String, String> {
        resolve_catalog_model(&self.catalog, &self.model_routes, requested)
    }
}

#[cfg(test)]
mod subagent_route_tests;
