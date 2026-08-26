//! Early, loud validation for provider profiles and configuration patches.
//!
//! Invalid shapes fail here with an explanation of how to fix them; the
//! switch executor refuses to receive anything that has not passed.

use crate::contracts::{
    AppKind, ClaudeModelSettings, CodexModelSettings, CommonConfigPatch, ModelOptions, PatchValue,
    ProviderDraft, ProviderProfile, RouteMode,
};
use crate::ownership::{is_owned, is_profile_exclusive};
use thiserror::Error;

pub const CODEX_REASONING_EFFORTS: &[&str] = &["minimal", "low", "medium", "high", "xhigh"];
pub const CODEX_REASONING_SUMMARIES: &[&str] = &["none", "auto", "concise", "detailed"];
pub const CODEX_VERBOSITIES: &[&str] = &["low", "medium", "high"];

#[derive(Debug, Error, PartialEq)]
pub enum ValidationError {
    #[error("供应商名称不能为空")]
    EmptyName,
    #[error("供应商标识不能为空")]
    EmptyId,
    #[error("自定义服务模式必须填写服务地址；若想使用官方登录，请将路由模式改为“官方登录”")]
    CustomRequiresBaseUrl,
    #[error("官方登录模式不能设置服务地址；请改用“自定义服务”模式")]
    OfficialForbidsBaseUrl,
    #[error("官方登录模式不能设置环境变量名；官方登录凭据由客户端自己管理")]
    OfficialForbidsEnvKey,
    #[error("base_url 必须是 http(s) URL，当前值为 {0:?}；请填写包含协议头的完整地址")]
    BadBaseUrl(String),
    #[error("环境变量名 {0:?} 无效；请填写标准环境变量名，例如 OPENAI_API_KEY")]
    InvalidEnvKey(String),
    #[error("环境变量名 {0:?} 看起来像凭证值；请填写变量名，而不是密钥")]
    EnvKeyLooksLikeSecret(String),
    #[error("Claude Code 的凭据由现有登录或环境管理，供应商配置不能设置环境变量名")]
    ClaudeEnvKeyUnsupported,
    #[error("供应商所属客户端 {profile_app:?} 与通用配置所属客户端 {patch_app:?} 不一致")]
    AppMismatch {
        profile_app: AppKind,
        patch_app: AppKind,
    },
    #[error("模型参数类型 {options_kind:?} 与供应商所属客户端 {app:?} 不一致")]
    ModelOptionsMismatch {
        options_kind: &'static str,
        app: AppKind,
    },
    #[error("model_reasoning_effort 必须是 {allowed} 之一，当前值为 {value:?}")]
    BadReasoningEffort { value: String, allowed: String },
    #[error("model_reasoning_summary 必须是 {allowed} 之一，当前值为 {value:?}")]
    BadReasoningSummary { value: String, allowed: String },
    #[error("model_verbosity 必须是 {allowed} 之一，当前值为 {value:?}")]
    BadVerbosity { value: String, allowed: String },
    #[error("上下文窗口必须是正整数（token 数）")]
    BadContextWindow,
    #[error("availableModels 不能包含空行；请每行填写一个模型标识")]
    EmptyAvailableModel,
    #[error("键 {key:?} 属于 {app:?} 的宿主配置；请从覆盖配置中移除，Agent Switchboard 只能修改应用托管的键")]
    HostOwnedKey { app: AppKind, key: String },
    #[error("键 {key:?} 由供应商档案管理；请在“供应商”页的模型映射中设置，而不是通用配置")]
    ProfileExclusiveKey { key: String },
    #[error("配置项键不能为空")]
    EmptyKey,
    #[error("键 {key:?} 的数值必须为有限数")]
    NonFiniteNumber { key: String },
}

impl ProviderProfile {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.id.trim().is_empty() {
            return Err(ValidationError::EmptyId);
        }
        validate_profile_fields(ProfileFields {
            app: self.app,
            mode: self.mode,
            name: &self.name,
            base_url: self.base_url.as_deref(),
            env_key: self.env_key.as_deref(),
            model_options: self.model_options.as_ref(),
        })
    }
}

impl ProviderDraft {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_profile_fields(ProfileFields {
            app: self.app,
            mode: self.mode,
            name: &self.name,
            base_url: self.base_url.as_deref(),
            env_key: self.env_key.as_deref(),
            model_options: self.model_options.as_ref(),
        })
    }
}

struct ProfileFields<'a> {
    app: AppKind,
    mode: RouteMode,
    name: &'a str,
    base_url: Option<&'a str>,
    env_key: Option<&'a str>,
    model_options: Option<&'a ModelOptions>,
}

fn validate_profile_fields(fields: ProfileFields<'_>) -> Result<(), ValidationError> {
    let ProfileFields {
        app,
        mode,
        name,
        base_url,
        env_key,
        model_options,
    } = fields;

    if name.trim().is_empty() {
        return Err(ValidationError::EmptyName);
    }
    match mode {
        RouteMode::Custom => {
            let Some(url) = base_url else {
                return Err(ValidationError::CustomRequiresBaseUrl);
            };
            let ok = url.starts_with("https://") || url.starts_with("http://");
            if !ok {
                return Err(ValidationError::BadBaseUrl(url.to_string()));
            }
        }
        RouteMode::Official => {
            if base_url.is_some() {
                return Err(ValidationError::OfficialForbidsBaseUrl);
            }
            if env_key.is_some() {
                return Err(ValidationError::OfficialForbidsEnvKey);
            }
        }
    }
    if let Some(env_key) = env_key {
        if app == AppKind::Claude {
            return Err(ValidationError::ClaudeEnvKeyUnsupported);
        }
        let looks_like_secret = env_key.contains("sk-") || env_key.len() > 128;
        if looks_like_secret {
            return Err(ValidationError::EnvKeyLooksLikeSecret(env_key.to_string()));
        }
        let mut chars = env_key.chars();
        let valid = matches!(chars.next(), Some('_') | Some('A'..='Z') | Some('a'..='z'))
            && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric());
        if !valid {
            return Err(ValidationError::InvalidEnvKey(env_key.to_string()));
        }
    }
    if let Some(options) = model_options {
        validate_model_options(app, options)?;
    }
    Ok(())
}

fn validate_model_options(app: AppKind, options: &ModelOptions) -> Result<(), ValidationError> {
    let reject = |kind: &'static str| {
        Err(ValidationError::ModelOptionsMismatch {
            options_kind: kind,
            app,
        })
    };
    match (app, options) {
        (AppKind::Codex, ModelOptions::Codex(settings)) => validate_codex_settings(settings),
        (AppKind::Claude, ModelOptions::Claude(settings)) => validate_claude_settings(settings),
        (AppKind::Codex, ModelOptions::Claude(_)) => reject("claude"),
        (AppKind::Claude, ModelOptions::Codex(_)) => reject("codex"),
    }
}

fn one_of(value: &str, allowed: &[&str]) -> bool {
    allowed.contains(&value)
}

fn join_allowed(allowed: &[&str]) -> String {
    allowed.join("、")
}

fn validate_codex_settings(settings: &CodexModelSettings) -> Result<(), ValidationError> {
    if let Some(effort) = settings.reasoning_effort.as_deref() {
        if !one_of(effort, CODEX_REASONING_EFFORTS) {
            return Err(ValidationError::BadReasoningEffort {
                value: effort.to_string(),
                allowed: join_allowed(CODEX_REASONING_EFFORTS),
            });
        }
    }
    if let Some(summary) = settings.reasoning_summary.as_deref() {
        if !one_of(summary, CODEX_REASONING_SUMMARIES) {
            return Err(ValidationError::BadReasoningSummary {
                value: summary.to_string(),
                allowed: join_allowed(CODEX_REASONING_SUMMARIES),
            });
        }
    }
    if let Some(verbosity) = settings.verbosity.as_deref() {
        if !one_of(verbosity, CODEX_VERBOSITIES) {
            return Err(ValidationError::BadVerbosity {
                value: verbosity.to_string(),
                allowed: join_allowed(CODEX_VERBOSITIES),
            });
        }
    }
    if let Some(window) = settings.context_window {
        if window == 0 {
            return Err(ValidationError::BadContextWindow);
        }
    }
    Ok(())
}

fn validate_claude_settings(settings: &ClaudeModelSettings) -> Result<(), ValidationError> {
    if let Some(models) = settings.available_models.as_deref() {
        if models.iter().any(|model| model.trim().is_empty()) {
            return Err(ValidationError::EmptyAvailableModel);
        }
    }
    Ok(())
}

impl CommonConfigPatch {
    pub fn validate(&self) -> Result<(), ValidationError> {
        for entry in &self.entries {
            if entry.key.trim().is_empty() {
                return Err(ValidationError::EmptyKey);
            }
            if !is_owned(self.app, &entry.key) {
                return Err(ValidationError::HostOwnedKey {
                    app: self.app,
                    key: entry.key.clone(),
                });
            }
            if is_profile_exclusive(&entry.key) {
                return Err(ValidationError::ProfileExclusiveKey {
                    key: entry.key.clone(),
                });
            }
            if let PatchValue::Number(n) = entry.value {
                if !n.is_finite() {
                    return Err(ValidationError::NonFiniteNumber {
                        key: entry.key.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Validates that a plan is internally consistent: profile app matches patch
/// app and both shapes are valid.
pub fn validate_plan(
    profile: &ProviderProfile,
    patch: &CommonConfigPatch,
) -> Result<(), ValidationError> {
    profile.validate()?;
    patch.validate()?;
    if profile.app != patch.app {
        return Err(ValidationError::AppMismatch {
            profile_app: profile.app,
            patch_app: patch.app,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::PatchEntry;

    fn profile(app: AppKind) -> ProviderProfile {
        ProviderProfile {
            id: "p1".into(),
            app,
            mode: RouteMode::Custom,
            name: "Relay A".into(),
            model: Some("m-1".into()),
            base_url: Some("https://example.internal/v1".into()),
            env_key: None,
            model_options: None,
        }
    }

    fn patch(app: AppKind, key: &str) -> CommonConfigPatch {
        CommonConfigPatch {
            app,
            entries: vec![PatchEntry {
                key: key.into(),
                value: PatchValue::Bool(true),
            }],
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
    fn official_mode_rejects_base_url_and_env_key() {
        let mut p = profile(AppKind::Codex);
        p.mode = RouteMode::Official;
        assert_eq!(p.validate(), Err(ValidationError::OfficialForbidsBaseUrl));
        p.base_url = None;
        p.env_key = Some("OPENAI_API_KEY".into());
        assert_eq!(p.validate(), Err(ValidationError::OfficialForbidsEnvKey));
        p.env_key = None;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn official_mode_is_valid_for_claude_without_credentials() {
        let mut p = profile(AppKind::Claude);
        p.mode = RouteMode::Official;
        p.base_url = None;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn rejects_non_http_base_url() {
        let mut p = profile(AppKind::Codex);
        p.base_url = Some("example.internal".into());
        assert!(matches!(p.validate(), Err(ValidationError::BadBaseUrl(_))));
    }

    #[test]
    fn rejects_env_key_that_is_a_secret_value() {
        let mut p = profile(AppKind::Codex);
        p.env_key = Some("sk-live-9f3bca...".into());
        assert!(matches!(
            p.validate(),
            Err(ValidationError::EnvKeyLooksLikeSecret(_))
        ));
    }

    #[test]
    fn rejects_env_key_for_claude() {
        let mut p = profile(AppKind::Claude);
        p.env_key = Some("ANTHROPIC_API_KEY".into());
        assert_eq!(p.validate(), Err(ValidationError::ClaudeEnvKeyUnsupported));
    }

    #[test]
    fn rejects_model_options_that_do_not_match_the_app() {
        let mut p = profile(AppKind::Codex);
        p.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
            haiku_model: None,
            sonnet_model: None,
            opus_model: None,
            available_models: None,
        }));
        assert!(matches!(
            p.validate(),
            Err(ValidationError::ModelOptionsMismatch { .. })
        ));
    }

    #[test]
    fn accepts_xhigh_effort_and_rejects_unknown_values() {
        let mut p = profile(AppKind::Codex);
        p.model_options = Some(ModelOptions::Codex(CodexModelSettings {
            reasoning_effort: Some("xhigh".into()),
            reasoning_summary: Some("concise".into()),
            verbosity: Some("low".into()),
            context_window: Some(272_000),
        }));
        assert!(p.validate().is_ok());

        p.model_options = Some(ModelOptions::Codex(CodexModelSettings {
            reasoning_effort: Some("extreme".into()),
            reasoning_summary: None,
            verbosity: None,
            context_window: None,
        }));
        assert!(matches!(
            p.validate(),
            Err(ValidationError::BadReasoningEffort { .. })
        ));
    }

    #[test]
    fn rejects_zero_context_window_and_blank_available_models() {
        let mut p = profile(AppKind::Codex);
        p.model_options = Some(ModelOptions::Codex(CodexModelSettings {
            reasoning_effort: None,
            reasoning_summary: None,
            verbosity: None,
            context_window: Some(0),
        }));
        assert_eq!(p.validate(), Err(ValidationError::BadContextWindow));

        let mut c = profile(AppKind::Claude);
        c.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
            haiku_model: None,
            sonnet_model: None,
            opus_model: None,
            available_models: Some(vec!["claude-opus-4".into(), "  ".into()]),
        }));
        assert_eq!(c.validate(), Err(ValidationError::EmptyAvailableModel));
    }

    #[test]
    fn rejects_host_owned_patch_keys() {
        let err = patch(AppKind::Codex, "threads").validate().unwrap_err();
        assert!(matches!(err, ValidationError::HostOwnedKey { .. }));
        assert!(err.to_string().contains("宿主配置"));
    }

    #[test]
    fn rejects_profile_exclusive_patch_keys() {
        let err = patch(AppKind::Codex, "model_reasoning_effort")
            .validate()
            .unwrap_err();
        assert!(matches!(err, ValidationError::ProfileExclusiveKey { .. }));
        assert!(err.to_string().contains("供应商档案"));
    }

    #[test]
    fn accepts_general_patch_keys() {
        assert!(patch(AppKind::Codex, "disable_response_storage")
            .validate()
            .is_ok());
    }

    #[test]
    fn rejects_app_mismatch_between_profile_and_patch() {
        // Claude patches have no general keys left (every owned Claude key is
        // profile-exclusive), so the mismatch is exercised Codex↔Claude with
        // the one valid Codex general key.
        let mut claude_profile = profile(AppKind::Claude);
        claude_profile.mode = RouteMode::Official;
        claude_profile.base_url = None;
        let err = validate_plan(
            &claude_profile,
            &patch(AppKind::Codex, "disable_response_storage"),
        )
        .unwrap_err();
        assert!(matches!(err, ValidationError::AppMismatch { .. }));
    }
}
