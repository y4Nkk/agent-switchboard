use crate::contracts::ProviderConnectionOptions;
use super::{message, LocalizedMessage};

pub(super) fn blocked_auth(connection: &ProviderConnectionOptions) -> Option<(&'static str, &'static str)> {
    if connection.claude_native.is_some() {
        return Some(("blocked.native", "原生云 SDK 的认证和连接不能转换为另一客户端的静态 API 供应商"));
    }
    if connection.auth_binding.as_ref().is_some_and(|binding| binding.source == "managed_account")
        || connection.provider_type.as_deref().is_some_and(|kind|
            matches!(kind, "github_copilot" | "codex_oauth" | "xai_oauth"))
    {
        return Some(("blocked.managed", "托管账号的身份和占位密钥不能转换为静态 API 密钥"));
    }
    if connection.provider_type.is_some() || connection.auth_binding.as_ref().is_some_and(|binding|
        binding.source != "provider_config" || binding.auth_provider.is_some() || binding.account_id.is_some())
    {
        return Some(("blocked.connection", "来源认证类型没有对应的跨客户端静态连接契约"));
    }
    None
}

pub(super) fn portable_connection(source: &ProviderConnectionOptions) -> ProviderConnectionOptions {
    let mut custom_endpoints = source.custom_endpoints.clone();
    for endpoint in custom_endpoints.values_mut() { endpoint.last_used = None; }
    ProviderConnectionOptions {
        is_full_url: source.is_full_url, custom_endpoints,
        endpoint_auto_select: source.endpoint_auto_select,
        custom_user_agent: source.custom_user_agent.clone(),
        local_proxy_request_overrides: source.local_proxy_request_overrides.clone(),
        auth_binding: source.auth_binding.clone(), models_url: source.models_url.clone(),
        ..Default::default()
    }
}

pub(super) fn connection_notices(connection: &ProviderConnectionOptions, notices: &mut Vec<LocalizedMessage>) {
    if !connection.custom_endpoints.is_empty() {
        notices.push(message("notice.endpoints", "已复制备用端点；其历史使用时间不继承"));
    }
    if connection.local_proxy_request_overrides.as_ref().is_some_and(|overrides|
        overrides.body.as_object().is_some_and(|body| body.contains_key("model")))
    {
        notices.push(message("notice.bodyModel", "请求体覆盖包含 model，实际模型仍由该覆盖值决定，请核对"));
    }
}
