use super::{blocked, manual_query, message, CodexConversionDraft, ConversionCatalogEntry,
    ConversionSource, ConversionStatus, ProviderConversion, ProviderConversionSeed};
use super::connection::{blocked_auth, connection_notices, portable_connection};
use crate::contracts::{AppKind, ClaudeModelRole, ClaudeModelSettings, CodexCapabilities,
    CodexChatReasoning, CodexEndpoint, CodexProviderRecord, CodexUpstream, ModelOptions,
    ProviderDraft, ProviderRecord, ResponsesOptions, ResponsesRequestMode, RouteMode};

pub fn from_codex(record: &CodexProviderRecord) -> Result<ProviderConversion, String> {
    let profile = &record.profile;
    profile.validate()?;
    let source = source(AppKind::Codex, &profile.id, &profile.name, &record.file_hash);
    if let Some((key, text)) = blocked_auth(&profile.connection) {
        return Ok(blocked(source, AppKind::Claude, key, text));
    }
    let main = profile.resolve_model(&profile.default_model)?;
    let normalize = |model: String| if profile.upstream == CodexUpstream::AnthropicMessages {
        strip_one_m(&model).to_string()
    } else { model };
    let target_main = normalize(main.clone());
    let models = profile.catalog.iter().map(|entry| profile.resolve_model(&entry.id).map(&normalize))
        .collect::<Result<Vec<_>, _>>()?;
    let mut available_models = Vec::new();
    for model in models { if !available_models.contains(&model) { available_models.push(model); } }
    let model_options = ClaudeModelSettings { primary_one_m: target_main != main,
        available_models: Some(available_models), ..Default::default() };
    let draft = ProviderDraft {
        app: AppKind::Claude, route_mode: RouteMode::Custom, name: profile.name.clone(),
        base_url: Some(profile.endpoint.0.clone()), connection: portable_connection(&profile.connection),
        api_key: profile.api_key.clone(),
        authentication: Some(profile.authentication.unwrap_or(profile.upstream.protocol().authentication_scheme())),
        upstream_protocol: Some(profile.upstream.protocol()),
        responses_options: (profile.upstream == CodexUpstream::Responses)
            .then_some(ResponsesOptions { request_mode: profile.request_mode }),
        max_output_tokens: None.into(), model: Some(target_main),
        model_options: Some(ModelOptions::Claude(model_options)),
        parameters: crate::ownership::default_provider_parameters(AppKind::Claude),
        claude_fragment: Default::default(), notes: record.notes.clone(),
        website_url: record.website_url.clone(), display: None, usage_query: None,
        official_quota_refresh_interval_minutes: None,
    };
    draft.validate().map_err(|error| error.to_string())?;
    let mut notices = vec![message("notice.parameters", "使用目标客户端的默认运行参数，不复制来源的客户端专属参数"),
        message("notice.codexBehavior", "Codex 的能力声明、压缩、搜索和推理方言不转换为 Claude 的等价能力")];
    if profile.subagent_route.is_some() { notices.push(dependency_notice()); }
    connection_notices(&profile.connection, &mut notices);
    Ok(ProviderConversion {
        source, target_app: AppKind::Claude, status: ConversionStatus::Ready,
        blocked_reason: None, notices, seed: Some(ProviderConversionSeed::Claude { draft }),
        usage_query: manual_query(&record.usage_query),
    })
}

pub fn from_provider(record: &ProviderRecord) -> Result<ProviderConversion, String> {
    let profile = &record.profile;
    profile.validate().map_err(|error| error.to_string())?;
    let target = if profile.app == AppKind::Codex { AppKind::Claude } else { AppKind::Codex };
    let source = source(profile.app, &profile.id, &profile.name, &record.file_hash);
    if profile.route_mode == RouteMode::Official {
        return Ok(blocked(source, target, "blocked.official", "官方登录由原客户端管理，不能复制为另一客户端的供应商"));
    }
    if profile.app != AppKind::Claude { return Err("Codex 第三方供应商必须使用专用档案".into()); }
    if let Some((key, text)) = blocked_auth(&profile.connection) {
        return Ok(blocked(source, target, key, text));
    }
    let Some(upstream) = profile.upstream_protocol.and_then(CodexUpstream::from_protocol) else {
        return Ok(blocked(source, target, "blocked.protocol", "当前 Codex 不支持此来源上游协议"));
    };
    let settings = match &profile.model_options { Some(ModelOptions::Claude(value)) => Some(value), _ => None };
    let (models, main) = claude_models(profile, settings);
    let draft = CodexConversionDraft {
        name: profile.name.clone(), endpoint: CodexEndpoint(profile.base_url.clone().ok_or("来源缺少服务地址")?),
        api_key: profile.api_key.clone(), authentication: Some(profile.authentication.unwrap_or(upstream.protocol().authentication_scheme())),
        connection: portable_connection(&profile.connection), upstream,
        request_mode: profile.responses_options.map(|options| options.request_mode).unwrap_or(ResponsesRequestMode::Standard),
        default_model: main.unwrap_or_default(), catalog: models.into_iter().map(ConversionCatalogEntry::unknown).collect(),
        model_routes: Vec::new(), subagent_route: None, capabilities: conservative_capabilities(),
        parameters: crate::ownership::default_provider_parameters(AppKind::Codex),
        notes: profile.notes.clone(), website_url: profile.website_url.clone(), usage_query: None,
    };
    let mut notices = vec![message("notice.parameters", "使用目标客户端的默认运行参数，不复制来源的客户端专属参数"),
        message("notice.capabilities", "请明确填写模型上下文、输出上限并确认 Codex 能力；未声明能力初始关闭"),
        message("notice.claudeBehavior", "Claude 的用途、1M 标记、计费、缓存与专属片段不自动转换为 Codex 设置"),
        message("notice.modelAliases", "客户端模型别名仅在能解析到本连接的实际模型时继承；无法解析的别名需重新选择模型")];
    if settings.is_some_and(|settings| settings.has_provider_routes()) { notices.push(dependency_notice()); }
    if profile.display.is_some() { notices.push(message("notice.display", "目标 Codex 档案不保存来源图标和分组元信息")); }
    connection_notices(&profile.connection, &mut notices);
    Ok(ProviderConversion { source, target_app: target, status: ConversionStatus::NeedsInput,
        blocked_reason: None, notices, seed: Some(ProviderConversionSeed::Codex { draft }),
        usage_query: manual_query(&profile.usage_query) })
}

fn source(app: AppKind, id: &str, name: &str, file_hash: &str) -> ConversionSource {
    ConversionSource { app, id: id.into(), name: name.into(), file_hash: file_hash.into() }
}

fn dependency_notice() -> crate::contracts::LocalizedMessage {
    message("notice.dependencies", "未复制跨供应商用途或子代理引用，请在目标端独立配置")
}

fn conservative_capabilities() -> CodexCapabilities {
    CodexCapabilities { responses: true, compact: false, models: false, chat_completions: false,
        alpha_search: false, image_generation: false, image_edit: false, function_tools: false,
        custom_tools: false, tool_search: false, reasoning: false, chat_reasoning: CodexChatReasoning::Unsupported }
}

fn claude_models(profile: &crate::contracts::ProviderProfile, settings: Option<&ClaudeModelSettings>) -> (Vec<String>, Option<String>) {
    let clean = |value: &str| claude_candidate(value, profile, settings);
    let main = profile.model.as_deref().and_then(clean);
    let mut models = main.iter().cloned().collect::<Vec<_>>();
    if let Some(settings) = settings {
        for id in settings.available_models.iter().flatten() {
            if let Some(id) = clean(id) { if !models.contains(&id) { models.push(id); } }
        }
        for role in ClaudeModelRole::ALL {
            if let Some(route) = settings.route(role).filter(|route|
                route.profile_id().is_none_or(|id| id == profile.id)) {
                if let Some(id) = clean(route.model()) { if !models.contains(&id) { models.push(id); } }
            }
        }
    }
    (models, main)
}

fn claude_candidate(value: &str, profile: &crate::contracts::ProviderProfile, settings: Option<&ClaudeModelSettings>) -> Option<String> {
    let value = strip_one_m(value);
    if matches!(value, "default" | "asb-claude-primary") {
        return profile.model.as_deref().and_then(actual_model);
    }
    if let Some(role) = ClaudeModelRole::ALL.into_iter().find(|role|
        value == role.name() || value == role.wire()) {
        let route = settings.and_then(|settings| settings.effective_route(role))?;
        if route.profile_id().is_some_and(|id| id != profile.id) { return None; }
        let resolved = crate::adapter::claude::gateway_model_source(role.wire(), profile.model.as_deref(), settings)
            .ok().flatten()?;
        return actual_model(resolved);
    }
    actual_model(value)
}

fn actual_model(value: &str) -> Option<String> {
    let value = strip_one_m(value);
    (!value.is_empty() && value != "default" && !value.starts_with("asb-claude-")
        && !ClaudeModelRole::ALL.into_iter().any(|role| value == role.name()))
        .then(|| value.to_string())
}

fn strip_one_m(value: &str) -> &str {
    if value.len() >= 4 && value.as_bytes()[value.len() - 4..].eq_ignore_ascii_case(b"[1m]") {
        value[..value.len() - 4].trim_end()
    } else { value }
}
