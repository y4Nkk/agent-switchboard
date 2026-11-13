use crate::contracts::{
    AuthenticationScheme, CodexCapabilities, CodexCatalogEntry, CodexEndpoint, CodexModelRoute,
    CodexProviderDraft, CodexReasoningLevel, CodexSubagentRoute, CodexUpstream,
    ProviderConnectionOptions, ResponsesRequestMode, SettingsValues, UsageQuery,
};
use serde::Serialize;

#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionCatalogEntry {
    pub id: String,
    pub context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub function_tools: bool,
    pub custom_tools: bool,
    pub tool_search: bool,
    pub reasoning: bool,
    pub default_reasoning_level: CodexReasoningLevel,
    pub supported_reasoning_levels: Vec<CodexReasoningLevel>,
    pub images: bool,
    pub compact: bool,
}

impl ConversionCatalogEntry {
    pub(super) fn unknown(id: String) -> Self {
        Self {
            id, context_window: None, max_output_tokens: None, function_tools: false,
            custom_tools: false, tool_search: false, reasoning: false,
            default_reasoning_level: CodexReasoningLevel::None,
            supported_reasoning_levels: vec![CodexReasoningLevel::None], images: false, compact: false,
        }
    }

    fn into_entry(self) -> Result<CodexCatalogEntry, String> {
        let positive = |value: Option<u64>| value.filter(|value| *value > 0)
            .ok_or_else(|| "转换的 Codex 模型必须明确填写正整数上下文窗口和输出上限".to_string());
        Ok(CodexCatalogEntry {
            id: self.id, context_window: positive(self.context_window)?,
            max_output_tokens: positive(self.max_output_tokens)?, function_tools: self.function_tools,
            custom_tools: self.custom_tools, tool_search: self.tool_search, reasoning: self.reasoning,
            default_reasoning_level: self.default_reasoning_level,
            supported_reasoning_levels: self.supported_reasoning_levels, images: self.images,
            compact: self.compact, display_name: None, description: None,
            base_instructions: None, supports_parallel_tool_calls: None,
        })
    }
}

/// Nullable model facts belong only to this editing contract, never the store.
#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexConversionDraft {
    pub name: String,
    pub endpoint: CodexEndpoint,
    pub api_key: String,
    pub authentication: Option<AuthenticationScheme>,
    pub connection: ProviderConnectionOptions,
    pub upstream: CodexUpstream,
    pub request_mode: ResponsesRequestMode,
    pub default_model: String,
    pub catalog: Vec<ConversionCatalogEntry>,
    pub model_routes: Vec<CodexModelRoute>,
    pub subagent_route: Option<CodexSubagentRoute>,
    pub capabilities: CodexCapabilities,
    pub parameters: SettingsValues,
    pub notes: Option<String>,
    pub website_url: Option<String>,
    pub usage_query: Option<UsageQuery>,
}

impl CodexConversionDraft {
    /// Used after explicit model input; persistence still performs current validation.
    pub fn into_draft(self) -> Result<CodexProviderDraft, String> {
        Ok(CodexProviderDraft {
            name: self.name, endpoint: self.endpoint, api_key: self.api_key,
            authentication: self.authentication, connection: self.connection, upstream: self.upstream,
            request_mode: self.request_mode, default_model: self.default_model,
            catalog: self.catalog.into_iter().map(ConversionCatalogEntry::into_entry).collect::<Result<_, _>>()?,
            model_routes: self.model_routes, subagent_route: self.subagent_route,
            capabilities: self.capabilities, parameters: self.parameters, notes: self.notes,
            website_url: self.website_url, usage_query: self.usage_query,
        })
    }
}
