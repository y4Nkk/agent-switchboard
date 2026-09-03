//! Early, loud validation for provider profiles and general settings.
//!
//! Invalid shapes fail here with an explanation of how to fix them; the
//! switch executor refuses to receive anything that has not passed.

use crate::contracts::{
    AppKind, ClaudeModelSettings, CodexModelSettings, CommonSettingValue, CommonSettings,
    ConfigValue, ModelOptions, ProviderDraft, ProviderProfile, RouteMode, UsageQuery,
};
use crate::ownership::{setting_spec, SettingControl, SettingOwner};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ValidationError {
    #[error("供应商名称不能为空")]
    EmptyName,
    #[error("供应商标识不能为空")]
    EmptyId,
    #[error("自定义供应商必须填写服务地址")]
    CustomRequiresBaseUrl,
    #[error("base_url 必须是 http(s) URL，当前值为 {0:?}；请填写包含协议头的完整地址")]
    BadBaseUrl(String),
    #[error("必须填写 API 密钥")]
    EmptyApiKey,
    #[error("API 密钥最长 {0} 个字符")]
    ApiKeyTooLong(usize),
    #[error("官方登录不得携带服务地址、API 密钥或模型覆盖；请先移除这些自定义路由字段")]
    OfficialRouteHasCustomFields,
    #[error("模型参数类型 {options_kind:?} 与供应商所属客户端 {app:?} 不一致")]
    ModelOptionsMismatch {
        options_kind: &'static str,
        app: AppKind,
    },
    #[error("键 {key} 不是 {app:?} 的通用设置参数；通用设置文件只能包含设置页列出的参数")]
    UnknownCommonKey { app: AppKind, key: String },
    #[error("通用设置缺少参数 {key:?} 的值；请重新加载后再保存")]
    MissingCommonKey { key: String },
    #[error("键 {key} 的值必须是 {allowed} 之一，当前值为 {value:?}")]
    BadCommonValue {
        key: String,
        value: String,
        allowed: String,
    },
    #[error("上下文窗口必须是正整数（token 数）")]
    BadContextWindow,
    #[error("availableModels 不能包含空行；请每行填写一个模型标识")]
    EmptyAvailableModel,
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
}

/// Metadata fields shared by draft and profile; kept short and local-only.
const MAX_NOTES_LEN: usize = 500;
/// Upper bound of a usage query's auto-refresh interval, in minutes (a day).
pub const MAX_USAGE_QUERY_REFRESH_INTERVAL: u32 = 1440;

impl ProviderProfile {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.id.trim().is_empty() {
            return Err(ValidationError::EmptyId);
        }
        validate_profile_fields(ProfileFields {
            app: self.app,
            route_mode: self.route_mode,
            name: &self.name,
            model: self.model.as_deref(),
            base_url: self.base_url.as_deref(),
            api_key: &self.api_key,
            model_options: self.model_options.as_ref(),
            notes: self.notes.as_deref(),
            website_url: self.website_url.as_deref(),
            usage_query: self.usage_query.as_ref(),
        })
    }
}

impl ProviderDraft {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_profile_fields(ProfileFields {
            app: self.app,
            route_mode: self.route_mode,
            name: &self.name,
            model: self.model.as_deref(),
            base_url: self.base_url.as_deref(),
            api_key: &self.api_key,
            model_options: self.model_options.as_ref(),
            notes: self.notes.as_deref(),
            website_url: self.website_url.as_deref(),
            usage_query: self.usage_query.as_ref(),
        })
    }
}

struct ProfileFields<'a> {
    app: AppKind,
    route_mode: RouteMode,
    name: &'a str,
    model: Option<&'a str>,
    base_url: Option<&'a str>,
    api_key: &'a str,
    model_options: Option<&'a ModelOptions>,
    notes: Option<&'a str>,
    website_url: Option<&'a str>,
    usage_query: Option<&'a UsageQuery>,
}

fn validate_profile_fields(fields: ProfileFields<'_>) -> Result<(), ValidationError> {
    let ProfileFields {
        app,
        route_mode,
        name,
        model,
        base_url,
        api_key,
        model_options,
        notes,
        website_url,
        usage_query,
    } = fields;

    if name.trim().is_empty() {
        return Err(ValidationError::EmptyName);
    }
    match route_mode {
        RouteMode::Official => {
            if base_url.is_some()
                || !api_key.trim().is_empty()
                || model.is_some()
                || model_options.is_some()
                || usage_query.is_some()
            {
                return Err(ValidationError::OfficialRouteHasCustomFields);
            }
        }
        RouteMode::Custom => {
            let Some(url) = base_url else {
                return Err(ValidationError::CustomRequiresBaseUrl);
            };
            let ok = url.starts_with("https://") || url.starts_with("http://");
            if !ok {
                return Err(ValidationError::BadBaseUrl(url.to_string()));
            }
            if api_key.trim().is_empty() {
                return Err(ValidationError::EmptyApiKey);
            }
            if api_key.chars().count() > 4_096 {
                return Err(ValidationError::ApiKeyTooLong(4_096));
            }
        }
    }
    validate_model_identifier(model, "主模型")?;
    if let Some(options) = model_options {
        validate_model_options(app, options, model)?;
    }
    if let Some(notes) = notes {
        if notes.chars().count() > MAX_NOTES_LEN {
            return Err(ValidationError::NotesTooLong(MAX_NOTES_LEN));
        }
    }
    if let Some(url) = website_url {
        let ok = url.starts_with("https://") || url.starts_with("http://");
        if !ok {
            return Err(ValidationError::BadWebsiteUrl(url.to_string()));
        }
    }
    if let Some(query) = usage_query {
        validate_usage_query(query)?;
    }
    Ok(())
}

const MAX_USAGE_QUERY_SOURCE_LEN: usize = 65_536;

/// Checks the serializable usage-query contract. Loading the two JavaScript
/// functions is intentionally desktop-runtime work, because this core crate
/// owns no JavaScript engine; the store runs that additional validation for
/// every persisted provider before it accepts or writes a file.
pub fn validate_usage_query(query: &UsageQuery) -> Result<(), ValidationError> {
    if query.refresh_interval_minutes() > MAX_USAGE_QUERY_REFRESH_INTERVAL {
        return Err(ValidationError::UsageQueryRefreshIntervalTooLarge(
            MAX_USAGE_QUERY_REFRESH_INTERVAL,
        ));
    }
    match query {
        UsageQuery::Declarative {
            url,
            remaining_path,
            used_path,
            total_path,
            unit,
            ..
        } => {
            let url = url.trim();
            if url.is_empty() {
                return Err(ValidationError::EmptyUsageQueryUrl);
            }
            if !(url.starts_with("https://")
                || url.starts_with("http://")
                || url.starts_with("{{baseUrl}}"))
                || url.chars().any(char::is_control)
            {
                return Err(ValidationError::BadUsageQueryUrl);
            }
            if remaining_path.is_none() && used_path.is_none() && total_path.is_none() {
                return Err(ValidationError::UsageQueryExtractsNothing);
            }
            for (field, value) in [
                ("remainingPath", remaining_path),
                ("usedPath", used_path),
                ("totalPath", total_path),
                ("unit", unit),
            ] {
                if value.as_ref().is_some_and(|value| {
                    value.trim().is_empty() || value.chars().any(char::is_control)
                }) {
                    return Err(ValidationError::EmptyUsageQueryField { field });
                }
            }
            Ok(())
        }
        UsageQuery::Script { source, .. } => {
            if source.trim().is_empty() {
                return Err(ValidationError::EmptyUsageQueryScript);
            }
            if source.chars().count() > MAX_USAGE_QUERY_SOURCE_LEN {
                return Err(ValidationError::UsageQueryScriptTooLong(
                    MAX_USAGE_QUERY_SOURCE_LEN,
                ));
            }
            Ok(())
        }
    }
}

fn validate_model_options(
    app: AppKind,
    options: &ModelOptions,
    primary_model: Option<&str>,
) -> Result<(), ValidationError> {
    let reject = |kind: &'static str| {
        Err(ValidationError::ModelOptionsMismatch {
            options_kind: kind,
            app,
        })
    };
    match (app, options) {
        (AppKind::Codex, ModelOptions::Codex(settings)) => validate_codex_settings(settings),
        (AppKind::Claude, ModelOptions::Claude(settings)) => {
            validate_claude_settings(settings, primary_model)
        }
        (AppKind::Codex, ModelOptions::Claude(_)) => reject("claude"),
        (AppKind::Claude, ModelOptions::Codex(_)) => reject("codex"),
    }
}

fn validate_model_identifier(
    model: Option<&str>,
    field: &'static str,
) -> Result<(), ValidationError> {
    let Some(model) = model else {
        return Ok(());
    };
    if crate::claude_model::contains_one_m_marker(model) {
        return Err(ValidationError::InlineOneMMarker { field });
    }
    Ok(())
}

fn validate_codex_settings(settings: &CodexModelSettings) -> Result<(), ValidationError> {
    if let Some(window) = settings.context_window {
        if window == 0 {
            return Err(ValidationError::BadContextWindow);
        }
    }
    Ok(())
}

fn validate_claude_settings(
    settings: &ClaudeModelSettings,
    primary_model: Option<&str>,
) -> Result<(), ValidationError> {
    validate_model_identifier(settings.haiku_model.as_deref(), "Haiku 档")?;
    validate_model_identifier(settings.sonnet_model.as_deref(), "Sonnet 档")?;
    validate_model_identifier(settings.opus_model.as_deref(), "Opus 档")?;
    validate_one_m_enabled(settings.primary_one_m, primary_model, "主模型")?;
    validate_one_m_enabled(
        settings.sonnet_one_m,
        settings.sonnet_model.as_deref(),
        "Sonnet 档",
    )?;
    validate_one_m_enabled(
        settings.opus_one_m,
        settings.opus_model.as_deref(),
        "Opus 档",
    )?;
    if let Some(models) = settings.available_models.as_deref() {
        for model in models {
            if model.trim().is_empty() {
                return Err(ValidationError::EmptyAvailableModel);
            }
            validate_model_identifier(Some(model), "可选模型列表")?;
        }
    }
    Ok(())
}

fn validate_one_m_enabled(
    enabled: bool,
    model: Option<&str>,
    field: &'static str,
) -> Result<(), ValidationError> {
    if enabled && model.is_none_or(|model| model.trim().is_empty()) {
        return Err(ValidationError::OneMRequiresModel { field });
    }
    Ok(())
}

impl CommonSettings {
    /// Validates the complete common settings for one client. The key set
    /// must be exactly the ownership directory's common parameters, and every
    /// value must match its control's shape and allowed values.
    pub fn validate_for(&self, app: AppKind) -> Result<(), ValidationError> {
        for (key, setting) in &self.settings {
            let Some(spec) = setting_spec(app, key) else {
                return Err(ValidationError::UnknownCommonKey {
                    app,
                    key: key.clone(),
                });
            };
            if spec.owner != SettingOwner::Common {
                return Err(ValidationError::UnknownCommonKey {
                    app,
                    key: key.clone(),
                });
            }
            let CommonSettingValue::Explicit { value } = setting else {
                continue;
            };
            if let ConfigValue::Number(number) = value {
                if !number.is_finite() {
                    return Err(ValidationError::NonFiniteNumber { key: key.clone() });
                }
            }
            match spec.control {
                SettingControl::Toggle => {
                    if !matches!(value, ConfigValue::Bool(_)) {
                        return Err(ValidationError::BadCommonValue {
                            key: key.clone(),
                            value: value.display(),
                            allowed: "true 或 false".to_string(),
                        });
                    }
                }
                SettingControl::Choice { .. } => {
                    let allowed: Vec<&str> = spec
                        .allowed_values
                        .iter()
                        .map(|option| option.value)
                        .collect();
                    let ok = match value {
                        ConfigValue::Str(text) => allowed.contains(&text.as_str()),
                        _ => false,
                    };
                    if !ok {
                        return Err(ValidationError::BadCommonValue {
                            key: key.clone(),
                            value: value.display(),
                            allowed: allowed.join("、"),
                        });
                    }
                }
                SettingControl::None => {
                    unreachable!("common settings always declare an editor control")
                }
            }
        }
        for spec in crate::ownership::setting_specs(app) {
            if spec.owner == SettingOwner::Common && !self.settings.contains_key(spec.key) {
                return Err(ValidationError::MissingCommonKey {
                    key: spec.key.to_string(),
                });
            }
        }
        Ok(())
    }
}

/// Validates the only current planning contract. Client selection comes from
/// the provider profile, so the common settings cannot carry a second,
/// conflicting `app` value.
pub fn validate_plan(
    profile: &ProviderProfile,
    common: &CommonSettings,
) -> Result<(), ValidationError> {
    profile.validate()?;
    common.validate_for(profile.app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ownership::default_common_settings;

    fn profile(app: AppKind) -> ProviderProfile {
        ProviderProfile {
            id: "p1".into(),
            app,
            route_mode: RouteMode::Custom,
            name: "Relay A".into(),
            model: Some("m-1".into()),
            base_url: Some("https://example.internal/v1".into()),
            api_key: "test-api-key".into(),
            model_options: None,
            notes: None,
            website_url: None,
            usage_query: None,
        }
    }

    #[test]
    fn rejects_empty_names() {
        let mut p = profile(AppKind::Codex);
        p.name = "  ".into();
        assert_eq!(p.validate(), Err(ValidationError::EmptyName));
    }

    #[test]
    fn custom_mode_requires_a_base_url() {
        let mut p = profile(AppKind::Codex);
        p.base_url = None;
        assert_eq!(p.validate(), Err(ValidationError::CustomRequiresBaseUrl));
    }

    #[test]
    fn missing_base_url_is_rejected_regardless_of_client() {
        let mut codex = profile(AppKind::Codex);
        codex.base_url = None;
        assert_eq!(
            codex.validate(),
            Err(ValidationError::CustomRequiresBaseUrl)
        );
        let mut claude = profile(AppKind::Claude);
        claude.base_url = None;
        assert_eq!(
            claude.validate(),
            Err(ValidationError::CustomRequiresBaseUrl)
        );
    }

    #[test]
    fn rejects_non_http_base_url() {
        let mut p = profile(AppKind::Codex);
        p.base_url = Some("example.internal".into());
        assert!(matches!(p.validate(), Err(ValidationError::BadBaseUrl(_))));
    }

    #[test]
    fn website_url_must_be_http_or_empty() {
        let mut p = profile(AppKind::Codex);
        p.website_url = Some("https://provider.example".into());
        assert!(p.validate().is_ok());
        p.website_url = Some("provider.example".into());
        assert!(matches!(
            p.validate(),
            Err(ValidationError::BadWebsiteUrl(_))
        ));
        p.website_url = None;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn notes_have_a_length_cap() {
        let mut p = profile(AppKind::Claude);
        p.notes = Some("短备注".into());
        assert!(p.validate().is_ok());
        p.notes = Some("长".repeat(MAX_NOTES_LEN + 1));
        assert!(matches!(
            p.validate(),
            Err(ValidationError::NotesTooLong(_))
        ));
    }

    #[test]
    fn rejects_empty_api_key() {
        let mut p = profile(AppKind::Codex);
        p.api_key.clear();
        assert_eq!(p.validate(), Err(ValidationError::EmptyApiKey));
    }

    #[test]
    fn accepts_api_keys_for_both_clients() {
        let mut codex = profile(AppKind::Codex);
        codex.api_key = "sk-live-codex".into();
        assert!(codex.validate().is_ok());
        let mut claude = profile(AppKind::Claude);
        claude.api_key = "sk-ant-claude".into();
        assert!(claude.validate().is_ok());
    }

    #[test]
    fn rejects_model_options_that_do_not_match_the_app() {
        let mut p = profile(AppKind::Codex);
        p.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
            primary_one_m: false,
            haiku_model: None,
            sonnet_model: None,
            sonnet_one_m: false,
            opus_model: None,
            opus_one_m: false,
            available_models: None,
        }));
        assert!(matches!(
            p.validate(),
            Err(ValidationError::ModelOptionsMismatch { .. })
        ));
    }

    #[test]
    fn rejects_zero_context_window_and_blank_available_models() {
        let mut p = profile(AppKind::Codex);
        p.model_options = Some(ModelOptions::Codex(CodexModelSettings {
            context_window: Some(0),
        }));
        assert_eq!(p.validate(), Err(ValidationError::BadContextWindow));

        let mut c = profile(AppKind::Claude);
        c.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
            primary_one_m: false,
            haiku_model: None,
            sonnet_model: None,
            sonnet_one_m: false,
            opus_model: None,
            opus_one_m: false,
            available_models: Some(vec!["claude-opus-4".into(), "  ".into()]),
        }));
        assert_eq!(c.validate(), Err(ValidationError::EmptyAvailableModel));
    }

    #[test]
    fn claude_one_m_is_explicit_and_model_identifiers_are_marker_free() {
        let mut primary = profile(AppKind::Claude);
        primary.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
            primary_one_m: true,
            haiku_model: None,
            sonnet_model: None,
            sonnet_one_m: false,
            opus_model: None,
            opus_one_m: false,
            available_models: None,
        }));
        assert!(primary.validate().is_ok());

        primary.model = Some("opus[1m]".into());
        assert_eq!(
            primary.validate(),
            Err(ValidationError::InlineOneMMarker { field: "主模型" })
        );

        primary.model = Some("opus[1M]".into());
        assert_eq!(
            primary.validate(),
            Err(ValidationError::InlineOneMMarker { field: "主模型" })
        );

        let mut missing_primary = profile(AppKind::Claude);
        missing_primary.model = None;
        missing_primary.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
            primary_one_m: true,
            haiku_model: None,
            sonnet_model: None,
            sonnet_one_m: false,
            opus_model: None,
            opus_one_m: false,
            available_models: None,
        }));
        assert_eq!(
            missing_primary.validate(),
            Err(ValidationError::OneMRequiresModel { field: "主模型" })
        );

        let mut mappings = profile(AppKind::Claude);
        mappings.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
            primary_one_m: false,
            haiku_model: Some("haiku[1m]".into()),
            sonnet_model: Some("sonnet".into()),
            sonnet_one_m: true,
            opus_model: Some("opus".into()),
            opus_one_m: true,
            available_models: Some(vec!["opus[1m]".into()]),
        }));
        assert_eq!(
            mappings.validate(),
            Err(ValidationError::InlineOneMMarker { field: "Haiku 档" })
        );

        mappings.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
            primary_one_m: false,
            haiku_model: None,
            sonnet_model: None,
            sonnet_one_m: true,
            opus_model: Some("opus".into()),
            opus_one_m: false,
            available_models: None,
        }));
        assert_eq!(
            mappings.validate(),
            Err(ValidationError::OneMRequiresModel {
                field: "Sonnet 档"
            })
        );
    }

    #[test]
    fn default_settings_validate_and_reject_unknown_keys_loudly() {
        for app in [AppKind::Codex, AppKind::Claude] {
            assert!(default_common_settings(app).validate_for(app).is_ok());
        }

        let mut settings = default_common_settings(AppKind::Codex);
        settings.settings.insert(
            "threads".to_string(),
            CommonSettingValue::Explicit {
                value: ConfigValue::Bool(true),
            },
        );
        let error = settings.validate_for(AppKind::Codex).unwrap_err();
        assert!(matches!(error, ValidationError::UnknownCommonKey { .. }));
        assert!(error.to_string().contains("threads"));

        let mut provider_key = default_common_settings(AppKind::Claude);
        provider_key.settings.insert(
            "model".to_string(),
            CommonSettingValue::Explicit {
                value: ConfigValue::Str("m".into()),
            },
        );
        assert!(matches!(
            provider_key.validate_for(AppKind::Claude),
            Err(ValidationError::UnknownCommonKey { .. })
        ));
    }

    #[test]
    fn incomplete_settings_are_rejected_with_the_missing_key() {
        let mut settings = default_common_settings(AppKind::Codex);
        settings.settings.remove("model_reasoning_effort");
        let error = settings.validate_for(AppKind::Codex).unwrap_err();
        assert_eq!(
            error,
            ValidationError::MissingCommonKey {
                key: "model_reasoning_effort".to_string()
            }
        );
    }

    #[test]
    fn choice_values_must_be_catalog_values_and_toggles_must_be_bools() {
        let mut settings = default_common_settings(AppKind::Codex);
        settings.settings.insert(
            "model_reasoning_effort".to_string(),
            CommonSettingValue::Explicit {
                value: ConfigValue::Str("xhigh".into()),
            },
        );
        assert!(settings.validate_for(AppKind::Codex).is_ok());

        settings.settings.insert(
            "model_reasoning_effort".to_string(),
            CommonSettingValue::Explicit {
                value: ConfigValue::Str("extreme".into()),
            },
        );
        let error = settings.validate_for(AppKind::Codex).unwrap_err();
        assert!(matches!(error, ValidationError::BadCommonValue { .. }));
        assert!(error.to_string().contains("minimal"));

        settings.settings.insert(
            "model_reasoning_effort".to_string(),
            CommonSettingValue::Explicit {
                value: ConfigValue::Bool(true),
            },
        );
        assert!(matches!(
            settings.validate_for(AppKind::Codex),
            Err(ValidationError::BadCommonValue { .. })
        ));

        let mut toggled = default_common_settings(AppKind::Claude);
        toggled.settings.insert(
            "autoCompactEnabled".to_string(),
            CommonSettingValue::Explicit {
                value: ConfigValue::Str("on".into()),
            },
        );
        assert!(matches!(
            toggled.validate_for(AppKind::Claude),
            Err(ValidationError::BadCommonValue { .. })
        ));

        // Both polarities are legal: the parameter is a plain value.
        let mut both = default_common_settings(AppKind::Claude);
        both.settings.insert(
            "autoCompactEnabled".to_string(),
            CommonSettingValue::Explicit {
                value: ConfigValue::Bool(false),
            },
        );
        assert!(both.validate_for(AppKind::Claude).is_ok());
        both.settings.insert(
            "autoCompactEnabled".to_string(),
            CommonSettingValue::Explicit {
                value: ConfigValue::Bool(true),
            },
        );
        assert!(both.validate_for(AppKind::Claude).is_ok());
    }

    #[test]
    fn plan_uses_the_profile_app_for_the_fixed_common_settings() {
        let claude_profile = profile(AppKind::Claude);
        assert!(validate_plan(&claude_profile, &default_common_settings(AppKind::Claude),).is_ok());
        assert!(matches!(
            validate_plan(&claude_profile, &default_common_settings(AppKind::Codex),),
            Err(ValidationError::UnknownCommonKey { .. })
        ));
    }

    #[test]
    fn usage_query_contract_rejects_empty_paths_and_scripts_before_persistence() {
        let valid = UsageQuery::Declarative {
            url: "{{baseUrl}}/balance".to_string(),
            remaining_path: Some("data/balance".to_string()),
            used_path: None,
            total_path: None,
            unit: Some("USD".to_string()),
            refresh_interval_minutes: 0,
        };
        assert!(validate_usage_query(&valid).is_ok());

        let empty_paths = UsageQuery::Declarative {
            url: "https://relay.example/balance".to_string(),
            remaining_path: None,
            used_path: None,
            total_path: None,
            unit: None,
            refresh_interval_minutes: 0,
        };
        assert_eq!(
            validate_usage_query(&empty_paths),
            Err(ValidationError::UsageQueryExtractsNothing)
        );

        let blank_script = UsageQuery::Script {
            source: " \n".to_string(),
            refresh_interval_minutes: 0,
        };
        assert_eq!(
            validate_usage_query(&blank_script),
            Err(ValidationError::EmptyUsageQueryScript)
        );
    }

    #[test]
    fn usage_query_contract_bounds_the_auto_refresh_interval() {
        let too_large = UsageQuery::Script {
            source: "({})".to_string(),
            refresh_interval_minutes: 1441,
        };
        assert_eq!(
            validate_usage_query(&too_large),
            Err(ValidationError::UsageQueryRefreshIntervalTooLarge(1440))
        );

        let at_bound = UsageQuery::Script {
            source: "({})".to_string(),
            refresh_interval_minutes: 1440,
        };
        assert!(validate_usage_query(&at_bound).is_ok());
    }
}
