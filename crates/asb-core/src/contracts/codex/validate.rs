use std::collections::HashSet;

use super::{
    required, CodexCapabilities, CodexCatalogEntry, CodexChatReasoning, CodexProviderProfile,
    CodexReasoningLevel, CodexRouteMode, CodexUpstream,
};
use crate::contracts::ProviderConnectionOptions;

pub(super) fn profile(profile: &CodexProviderProfile) -> Result<(), String> {
    validate_identity(profile)?;
    validate_capabilities(&profile.capabilities, profile.upstream)?;
    validate_catalog(profile)?;
    validate_model_routes(profile)?;
    validate_subagent_route(profile)
}

fn validate_identity(profile: &CodexProviderProfile) -> Result<(), String> {
    crate::validate::validate_codex_connection(
        &profile.connection,
        profile.upstream.protocol(),
        profile.authentication,
    )
    .map_err(|error| error.to_string())?;
    required(&profile.id, "供应商标识")?;
    required(&profile.name, "供应商名称")?;
    required(&profile.api_key, "API 密钥")?;
    let expected_route = CodexRouteMode::for_profile(
        profile.upstream,
        profile.request_mode,
        &profile.connection,
        profile.authentication,
        Some(profile.endpoint.0.as_str()),
        profile.subagent_route.as_ref(),
        !profile.model_routes.is_empty(),
    );
    if profile.route_mode != expected_route {
        return Err("Codex 路由模式与上游协议、请求模式、认证或连接覆盖不一致".to_string());
    }
    validate_endpoint(
        &profile.endpoint.0,
        profile.upstream,
        profile.connection.is_full_url,
    )?;
    validate_connection(&profile.connection, profile.upstream)?;
    Ok(())
}

fn validate_endpoint(
    endpoint: &str,
    upstream: CodexUpstream,
    is_full_url: bool,
) -> Result<(), String> {
    if is_full_url {
        return crate::endpoint::validate_full_url(endpoint);
    }
    crate::endpoint::validate_base_url(endpoint, upstream.protocol())
}

fn validate_connection(
    connection: &ProviderConnectionOptions,
    upstream: CodexUpstream,
) -> Result<(), String> {
    if connection.claude_native.is_some()
        || connection.claude_billing.is_some()
        || connection.claude_prompt_cache_key.is_some()
    {
        return Err("Claude 专用连接设置不能进入 Codex 档案".into());
    }
    if let Some(options) = &connection.codex {
        options.validate(upstream)?;
    }
    if let Some(agent) = connection.custom_user_agent.as_deref() {
        if agent.trim().is_empty() || agent.chars().any(char::is_control) {
            return Err("Codex 自定义 User-Agent 无效".to_string());
        }
    }
    if let Some(overrides) = connection.local_proxy_request_overrides.as_ref() {
        for (name, value) in &overrides.headers {
            if name.trim().is_empty() || name.chars().any(char::is_control) {
                return Err("Codex 自定义请求头名称无效".to_string());
            }
            if value.chars().any(char::is_control) {
                return Err(format!("Codex 自定义请求头值无效：{name}"));
            }
            if protected_header(name) {
                return Err(format!("Codex 自定义请求头不能覆盖受保护字段：{name}"));
            }
        }
        if !overrides.body.is_null() && !overrides.body.is_object() {
            return Err("Codex request body override 必须是 JSON 对象".to_string());
        }
    }
    for (key, endpoint) in &connection.custom_endpoints {
        if key.trim() != key || key != &endpoint.url {
            return Err("Codex 自定义端点索引与 URL 不一致".to_string());
        }
        if connection.is_full_url {
            crate::endpoint::validate_full_url(&endpoint.url)?;
        } else {
            validate_endpoint(&endpoint.url, upstream, false)?;
        }
    }
    Ok(())
}

fn protected_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "host"
            | "content-length"
            | "content-type"
            | "transfer-encoding"
            | "connection"
            | "accept-encoding"
            | "authorization"
            | "x-api-key"
            | "proxy-authorization"
            | "proxy-authenticate"
            | "te"
            | "trailer"
            | "upgrade"
    )
}

fn validate_capabilities(
    capabilities: &CodexCapabilities,
    upstream: CodexUpstream,
) -> Result<(), String> {
    if !capabilities.responses {
        return Err("Codex 能力必须声明 responses".to_string());
    }
    if !capabilities.reasoning
        && !matches!(capabilities.chat_reasoning, CodexChatReasoning::Unsupported)
    {
        return Err("未启用 reasoning 时 chatReasoning 必须为 unsupported".to_string());
    }
    if upstream != CodexUpstream::ChatCompletions
        && !matches!(capabilities.chat_reasoning, CodexChatReasoning::Unsupported)
    {
        return Err("非 Chat Completions 上游不能声明 chatReasoning".to_string());
    }
    Ok(())
}

fn validate_catalog(profile: &CodexProviderProfile) -> Result<(), String> {
    if profile.catalog.is_empty() {
        return Err("Codex 模型目录不能为空".to_string());
    }
    let mut ids = HashSet::new();
    for entry in &profile.catalog {
        validate_catalog_entry(entry, &profile.capabilities)?;
        if !ids.insert(entry.id.as_str()) {
            return Err(format!("Codex 模型目录含有重复模型：{}", entry.id));
        }
    }
    required(&profile.default_model, "Codex 默认模型")?;
    if !ids.contains(profile.default_model.as_str()) {
        return Err("Codex 默认模型必须存在于模型目录".to_string());
    }
    Ok(())
}

fn validate_catalog_entry(
    entry: &CodexCatalogEntry,
    capabilities: &CodexCapabilities,
) -> Result<(), String> {
    required(&entry.id, "目录模型标识")?;
    if entry.context_window == 0 || entry.max_output_tokens == 0 {
        return Err("Codex 模型目录的上下文窗口和输出上限必须大于零".to_string());
    }
    if entry.function_tools && !capabilities.function_tools {
        return Err(format!("模型 functionTools 超出供应商能力：{}", entry.id));
    }
    if entry.custom_tools && !capabilities.custom_tools {
        return Err(format!("模型 customTools 超出供应商能力：{}", entry.id));
    }
    if entry.tool_search && !capabilities.tool_search {
        return Err(format!("模型 toolSearch 超出供应商能力：{}", entry.id));
    }
    if entry.compact && !capabilities.compact {
        return Err(format!("模型 compact 超出供应商能力：{}", entry.id));
    }
    if entry.reasoning && !capabilities.reasoning {
        return Err(format!("模型 reasoning 超出供应商能力：{}", entry.id));
    }
    validate_reasoning_levels(entry)
}

fn validate_reasoning_levels(entry: &CodexCatalogEntry) -> Result<(), String> {
    if entry.supported_reasoning_levels.is_empty() {
        return Err(format!("Codex 模型目录未声明推理档位：{}", entry.id));
    }
    let levels = entry
        .supported_reasoning_levels
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    if levels.len() != entry.supported_reasoning_levels.len() {
        return Err(format!("Codex 模型目录含有重复推理档位：{}", entry.id));
    }
    if !levels.contains(&entry.default_reasoning_level) {
        return Err(format!("Codex 模型默认推理档位未声明：{}", entry.id));
    }
    if !entry.reasoning
        && (entry.default_reasoning_level != CodexReasoningLevel::None
            || levels.len() != 1
            || !levels.contains(&CodexReasoningLevel::None))
    {
        return Err(format!(
            "不支持推理的 Codex 模型只能声明 none 档位：{}",
            entry.id
        ));
    }
    Ok(())
}

fn validate_model_routes(profile: &CodexProviderProfile) -> Result<(), String> {
    let catalog = profile
        .catalog
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<HashSet<_>>();
    let mut sources = HashSet::new();
    for route in &profile.model_routes {
        required(&route.client_model, "客户端模型标识")?;
        required(&route.upstream_model, "上游模型标识")?;
        if !catalog.contains(route.client_model.as_str()) {
            return Err(format!(
                "模型映射引用了目录外客户端模型：{}",
                route.client_model
            ));
        }
        if !sources.insert(route.client_model.as_str()) {
            return Err(format!(
                "Codex 模型映射含有重复客户端模型：{}",
                route.client_model
            ));
        }
    }
    Ok(())
}

/// The route's shape is validated here; whether the referenced profile still
/// exists is a store-level check owned by the application.
fn validate_subagent_route(profile: &CodexProviderProfile) -> Result<(), String> {
    let Some(route) = &profile.subagent_route else {
        return Ok(());
    };
    if !route.has_canonical_profile_id() {
        return Err("Codex 子代理路由的供应商标识必须是标准 UUID".to_string());
    }
    required(&route.model, "Codex 子代理路由模型")?;
    if route.model.contains('/') && route.model.split('/').any(|part| part.trim().is_empty()) {
        return Err("Codex 子代理路由模型不能包含空路径段".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::codex::CodexSubagentRoute;
    use crate::contracts::ResponsesRequestMode;

    #[test]
    fn minimal_responses_requires_the_gateway() {
        assert_eq!(
            CodexRouteMode::for_profile(
                CodexUpstream::Responses,
                ResponsesRequestMode::Minimal,
                &ProviderConnectionOptions::default(),
                None,
                None,
                None,
                false,
            ),
            CodexRouteMode::Gateway,
        );
    }

    #[test]
    fn standard_native_responses_remains_direct() {
        assert_eq!(
            CodexRouteMode::for_profile(
                CodexUpstream::Responses,
                ResponsesRequestMode::Standard,
                &ProviderConnectionOptions::default(),
                None,
                None,
                None,
                false,
            ),
            CodexRouteMode::Direct,
        );
    }

    #[test]
    fn a_subagent_route_always_requires_the_gateway() {
        assert_eq!(
            CodexRouteMode::for_profile(
                CodexUpstream::Responses,
                ResponsesRequestMode::Standard,
                &ProviderConnectionOptions::default(),
                None,
                None,
                Some(&CodexSubagentRoute {
                    profile_id: "01234567-89ab-cdef-0123-456789abcdef".to_string(),
                    model: "gpt-5-codex".to_string(),
                }),
                false,
            ),
            CodexRouteMode::Gateway,
        );
    }
}
