use crate::contracts::AppKind;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ValidationError {
    #[error("Codex 请求选项不能用于其他客户端")]
    CodexOptionsRequireCodex,
    #[error("{0}")]
    ClaudeProviderOptions(String),
    #[error("{0}")]
    CodexProviderOptions(String),
    #[error("供应商名称不能为空")]
    EmptyName,
    #[error("供应商标识不能为空")]
    EmptyId,
    #[error("自定义供应商必须填写服务地址")]
    CustomRequiresBaseUrl,
    #[error("服务地址无效：{0}")]
    BadBaseUrl(String),
    #[error("完整服务地址无效：{0}")]
    BadFullUrl(String),
    #[error("必须填写 API 密钥")]
    EmptyApiKey,
    #[error("xAI 托管卡不能携带静态 API 密钥；凭据由登录的 xAI 账号按次提供")]
    XaiManagedCardRejectsStaticKey,
    #[error("API 密钥不能包含换行或其他控制字符")]
    InvalidApiKeyCharacters,
    #[error("自定义供应商必须选择上游 API 格式")]
    CustomRequiresProtocol,
    #[error("自定义 Responses 供应商必须明确配置请求模式")]
    ResponsesRequiresOptions,
    #[error("Responses 能力设置只适用于自定义 Responses 供应商")]
    UnexpectedResponsesOptions,
    #[error("Codex 转换到 Anthropic Messages 时必须配置正整数的最大输出 token 数")]
    CodexAnthropicRequiresMaxOutputTokens,
    #[error("最大输出 token 数只适用于 Codex 转换到 Anthropic Messages 的供应商")]
    UnexpectedMaxOutputTokens,
    #[error("API 密钥最长 {0} 个字符")]
    ApiKeyTooLong(usize),
    #[error("自定义 User-Agent 不能包含控制字符或为空")]
    InvalidCustomUserAgent,
    #[error("自定义请求头无效：{0}")]
    InvalidConnectionHeader(String),
    #[error("自定义 body 覆盖必须是 JSON 对象")]
    InvalidConnectionBody,
    #[error("官方登录不得携带服务地址、API 密钥、上游协议或模型覆盖；请先移除这些自定义路由字段")]
    OfficialRouteHasCustomFields,
    #[error("模型参数类型 {options_kind:?} 与供应商所属客户端 {app:?} 不一致")]
    ModelOptionsMismatch {
        options_kind: &'static str,
        app: AppKind,
    },
    #[error("键 {key} 不属于当前 {app:?} 设置作用域；供应商参数与客户端设置必须分别保存")]
    UnknownSettingKey { app: AppKind, key: String },
    #[error("子代理模型参数 agents.default_subagent_model 已由跨档案路由引用 subagentRoute 取代；请在「模型路由与网关」中重新配置子代理模型并移除旧参数")]
    RetiredSubagentModelParameter,
    #[error("设置缺少参数 {key:?} 的值；请重新加载后再保存")]
    MissingSettingKey { key: String },
    #[error("键 {key} 的值必须是 {allowed} 之一，当前值为 {value:?}")]
    BadSettingValue {
        key: String,
        value: String,
        allowed: String,
    },
    #[error("上下文窗口必须是正整数（token 数）")]
    BadContextWindow,
    #[error("availableModels 不能包含空行；请每行填写一个模型标识")]
    EmptyAvailableModel,
    #[error("模型显示名称不能为空或包含控制字符")]
    InvalidModelDisplayName,
    #[error("{field} 不能包含 1M 标记；请通过 1M 上下文复选框设置")]
    InlineOneMMarker { field: &'static str },
    #[error("{field} 已启用 1M 上下文，但未填写模型")]
    OneMRequiresModel { field: &'static str },
    #[error("键 {key:?} 的数值必须为有限数")]
    NonFiniteNumber { key: String },
    #[error("官网地址必须是 http(s) URL，当前值为 {0:?}；请填写包含协议头的完整地址，或留空")]
    BadWebsiteUrl(String),
    #[error("备注最长 {0} 个字符")]
    NotesTooLong(usize),
    #[error("用量查询地址不能为空")]
    EmptyUsageQueryUrl,
    #[error("用量查询地址必须以 http(s) 地址或 {{baseUrl}} 开头")]
    BadUsageQueryUrl,
    #[error("用量查询至少要配置一个提取路径（余额 / 已用 / 总量）")]
    UsageQueryExtractsNothing,
    #[error("用量查询的 {field} 不能为空")]
    EmptyUsageQueryField { field: &'static str },
    #[error("用量查询脚本不能为空")]
    EmptyUsageQueryScript,
    #[error("用量查询脚本最长 {0} 个字符")]
    UsageQueryScriptTooLong(usize),
    #[error("自动刷新间隔须为 0–{0} 分钟，0 表示关闭")]
    UsageQueryRefreshIntervalTooLarge(u32),
    #[error("订阅额度自动刷新间隔只适用于 Codex 官方登录供应商")]
    QuotaIntervalRequiresOfficialCodex,
    #[error("订阅额度自动刷新间隔须为 1–{0} 分钟；未设置表示关闭")]
    OfficialQuotaRefreshIntervalOutOfRange(u32),
    #[error("{0}")]
    ClaudeCommonOptions(String),
    #[error("{0}")]
    ClaudeFragmentInvalid(String),
    #[error("附加配置片段只适用于 Claude 档案")]
    ClaudeFragmentRequiresClaude,
    #[error("子 agent 的{field}必须是{allowed}，当前值为 {value:?}")]
    SubagentBadValue {
        field: &'static str,
        allowed: String,
        value: String,
    },
}

impl ValidationError {
    /// Renderer translation coordinates: the catalog key plus structured
    /// parameters (`serde_json::Value` object). The `Display` text stays the
    /// scrubbed diagnostic detail. Variants that wrap free-form messages
    /// from delegated validators pass them through `{detail}` until those
    /// producers adopt structured messages.
    pub fn message_parts(&self) -> (&'static str, serde_json::Value) {
        match self {
            Self::CodexOptionsRequireCodex => ("validate.codexOptionsRequireCodex", serde_json::json!({})),
            Self::ClaudeProviderOptions(detail) | Self::CodexProviderOptions(detail) | Self::ClaudeCommonOptions(detail) | Self::ClaudeFragmentInvalid(detail) => {
                ("validate.freeformDetail", serde_json::json!({ "detail": detail }))
            }
            Self::EmptyName => ("validate.emptyName", serde_json::json!({})),
            Self::EmptyId => ("validate.emptyId", serde_json::json!({})),
            Self::CustomRequiresBaseUrl => ("validate.customRequiresBaseUrl", serde_json::json!({})),
            Self::BadBaseUrl(detail) => ("validate.badBaseUrl", serde_json::json!({ "detail": detail })),
            Self::BadFullUrl(detail) => ("validate.badFullUrl", serde_json::json!({ "detail": detail })),
            Self::EmptyApiKey => ("validate.emptyApiKey", serde_json::json!({})),
            Self::XaiManagedCardRejectsStaticKey => ("validate.xaiManagedCardRejectsStaticKey", serde_json::json!({})),
            Self::InvalidApiKeyCharacters => ("validate.invalidApiKeyCharacters", serde_json::json!({})),
            Self::CustomRequiresProtocol => ("validate.customRequiresProtocol", serde_json::json!({})),
            Self::ResponsesRequiresOptions => ("validate.responsesRequiresOptions", serde_json::json!({})),
            Self::UnexpectedResponsesOptions => ("validate.unexpectedResponsesOptions", serde_json::json!({})),
            Self::CodexAnthropicRequiresMaxOutputTokens => ("validate.codexAnthropicRequiresMaxOutputTokens", serde_json::json!({})),
            Self::UnexpectedMaxOutputTokens => ("validate.unexpectedMaxOutputTokens", serde_json::json!({})),
            Self::ApiKeyTooLong(max) => ("validate.apiKeyTooLong", serde_json::json!({ "max": max })),
            Self::InvalidCustomUserAgent => ("validate.invalidCustomUserAgent", serde_json::json!({})),
            Self::InvalidConnectionHeader(detail) => ("validate.invalidConnectionHeader", serde_json::json!({ "detail": detail })),
            Self::InvalidConnectionBody => ("validate.invalidConnectionBody", serde_json::json!({})),
            Self::OfficialRouteHasCustomFields => ("validate.officialRouteHasCustomFields", serde_json::json!({})),
            Self::ModelOptionsMismatch { options_kind, app } => (
                "validate.modelOptionsMismatch",
                serde_json::json!({ "optionsKind": options_kind, "app": app }),
            ),
            Self::UnknownSettingKey { app, key } => (
                "validate.unknownSettingKey",
                serde_json::json!({ "key": key, "app": app }),
            ),
            Self::RetiredSubagentModelParameter => ("validate.retiredSubagentModelParameter", serde_json::json!({})),
            Self::MissingSettingKey { key } => ("validate.missingSettingKey", serde_json::json!({ "key": key })),
            Self::BadSettingValue { key, value, allowed } => (
                "validate.badSettingValue",
                serde_json::json!({ "key": key, "value": value, "allowed": allowed }),
            ),
            Self::BadContextWindow => ("validate.badContextWindow", serde_json::json!({})),
            Self::EmptyAvailableModel => ("validate.emptyAvailableModel", serde_json::json!({})),
            Self::InvalidModelDisplayName => ("validate.invalidModelDisplayName", serde_json::json!({})),
            Self::InlineOneMMarker { field } => ("validate.inlineOneMMarker", serde_json::json!({ "field": field })),
            Self::OneMRequiresModel { field } => ("validate.oneMRequiresModel", serde_json::json!({ "field": field })),
            Self::NonFiniteNumber { key } => ("validate.nonFiniteNumber", serde_json::json!({ "key": key })),
            Self::BadWebsiteUrl(value) => ("validate.badWebsiteUrl", serde_json::json!({ "value": value })),
            Self::NotesTooLong(max) => ("validate.notesTooLong", serde_json::json!({ "max": max })),
            Self::EmptyUsageQueryUrl => ("validate.emptyUsageQueryUrl", serde_json::json!({})),
            Self::BadUsageQueryUrl => ("validate.badUsageQueryUrl", serde_json::json!({})),
            Self::UsageQueryExtractsNothing => ("validate.usageQueryExtractsNothing", serde_json::json!({})),
            Self::EmptyUsageQueryField { field } => ("validate.emptyUsageQueryField", serde_json::json!({ "field": field })),
            Self::EmptyUsageQueryScript => ("validate.emptyUsageQueryScript", serde_json::json!({})),
            Self::UsageQueryScriptTooLong(max) => ("validate.usageQueryScriptTooLong", serde_json::json!({ "max": max })),
            Self::UsageQueryRefreshIntervalTooLarge(max) => ("validate.usageQueryRefreshIntervalTooLarge", serde_json::json!({ "max": max })),
            Self::QuotaIntervalRequiresOfficialCodex => ("validate.quotaIntervalRequiresOfficialCodex", serde_json::json!({})),
            Self::OfficialQuotaRefreshIntervalOutOfRange(max) => ("validate.officialQuotaRefreshIntervalOutOfRange", serde_json::json!({ "max": max })),
            Self::ClaudeFragmentRequiresClaude => ("validate.claudeFragmentRequiresClaude", serde_json::json!({})),
            Self::SubagentBadValue { field, allowed, value } => (
                "validate.subagentBadValue",
                serde_json::json!({ "field": field, "allowed": allowed, "value": value }),
            ),
        }
    }
}

/// Metadata fields shared by draft and profile; kept short and local-only.
pub(super) const MAX_NOTES_LEN: usize = 500;
/// Upper bound of every auto-refresh interval contract, in whole minutes
/// (a day). Both the usage-query interval and the official Codex quota
/// interval consume this one bound.
pub const MAX_AUTO_REFRESH_INTERVAL_MINUTES: u32 = 1440;
