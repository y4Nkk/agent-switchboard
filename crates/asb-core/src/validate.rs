//! Early, loud validation for provider profiles and configuration patches.
//!
//! Invalid shapes fail here with an explanation of how to fix them; the
//! switch executor refuses to receive anything that has not passed.

use crate::contracts::{
    AppKind, ClaudeModelSettings, CodexModelSettings, CommonConfigPatch, ModelOptions, PatchValue,
    ProviderDraft, ProviderProfile,
};
use crate::ownership::{choice_spec, is_owned, is_profile_exclusive, toggle_spec};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ValidationError {
    #[error("供应商名称不能为空")]
    EmptyName,
    #[error("供应商标识不能为空")]
    EmptyId,
    #[error("必须填写服务地址；官方登录不再作为供应商档案管理，客户端自身登录即为官方路由")]
    CustomRequiresBaseUrl,
    #[error("base_url 必须是 http(s) URL，当前值为 {0:?}；请填写包含协议头的完整地址")]
    BadBaseUrl(String),
    #[error("必须填写 API 密钥")]
    EmptyApiKey,
    #[error("API 密钥最长 {0} 个字符")]
    ApiKeyTooLong(usize),
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
    #[error("键 {key} 的值必须是 {allowed} 之一（或留空移除该行），当前值为 {value:?}")]
    BadCommonValue {
        key: String,
        value: String,
        allowed: String,
    },
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
    #[error("官网地址必须是 http(s) URL，当前值为 {0:?}；请填写包含协议头的完整地址，或留空")]
    BadWebsiteUrl(String),
    #[error("备注最长 {0} 个字符")]
    NotesTooLong(usize),
}

/// Metadata fields shared by draft and profile; kept short and local-only.
const MAX_NOTES_LEN: usize = 500;

impl ProviderProfile {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.id.trim().is_empty() {
            return Err(ValidationError::EmptyId);
        }
        validate_profile_fields(ProfileFields {
            app: self.app,
            name: &self.name,
            base_url: self.base_url.as_deref(),
            api_key: &self.api_key,
            model_options: self.model_options.as_ref(),
            notes: self.notes.as_deref(),
            website_url: self.website_url.as_deref(),
        })
    }
}

impl ProviderDraft {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_profile_fields(ProfileFields {
            app: self.app,
            name: &self.name,
            base_url: self.base_url.as_deref(),
            api_key: &self.api_key,
            model_options: self.model_options.as_ref(),
            notes: self.notes.as_deref(),
            website_url: self.website_url.as_deref(),
        })
    }
}

struct ProfileFields<'a> {
    app: AppKind,
    name: &'a str,
    base_url: Option<&'a str>,
    api_key: &'a str,
    model_options: Option<&'a ModelOptions>,
    notes: Option<&'a str>,
    website_url: Option<&'a str>,
}

fn validate_profile_fields(fields: ProfileFields<'_>) -> Result<(), ValidationError> {
    let ProfileFields {
        app,
        name,
        base_url,
        api_key,
        model_options,
        notes,
        website_url,
    } = fields;

    if name.trim().is_empty() {
        return Err(ValidationError::EmptyName);
    }
    {
        let Some(url) = base_url else {
            return Err(ValidationError::CustomRequiresBaseUrl);
        };
        let ok = url.starts_with("https://") || url.starts_with("http://");
        if !ok {
            return Err(ValidationError::BadBaseUrl(url.to_string()));
        }
    }
    if api_key.trim().is_empty() {
        return Err(ValidationError::EmptyApiKey);
    }
    if api_key.chars().count() > 4_096 {
        return Err(ValidationError::ApiKeyTooLong(4_096));
    }
    if let Some(options) = model_options {
        validate_model_options(app, options)?;
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
            // Catalog keys are value-constrained: a toggle carries exactly its
            // applied bool, a choice exactly one of its official values. An
            // absent value (null) removes the line and is always valid.
            if let Some(value) = &entry.value {
                if let Some(spec) = choice_spec(self.app, &entry.key) {
                    let allowed: Vec<&str> =
                        spec.options.iter().map(|option| option.value).collect();
                    if let PatchValue::Str(text) = value {
                        if !one_of(text, &allowed) {
                            return Err(ValidationError::BadCommonValue {
                                key: entry.key.clone(),
                                value: text.clone(),
                                allowed: join_allowed(&allowed),
                            });
                        }
                    } else {
                        return Err(ValidationError::BadCommonValue {
                            key: entry.key.clone(),
                            value: value.display(),
                            allowed: join_allowed(&allowed),
                        });
                    }
                } else if let Some(spec) = toggle_spec(self.app, &entry.key) {
                    match value {
                        PatchValue::Bool(flag) if *flag == spec.applied => {}
                        other => {
                            return Err(ValidationError::BadCommonValue {
                                key: entry.key.clone(),
                                value: other.display(),
                                allowed: spec.applied.to_string(),
                            });
                        }
                    }
                }
            }
            if let Some(PatchValue::Number(n)) = entry.value {
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
            name: "Relay A".into(),
            model: Some("m-1".into()),
            base_url: Some("https://example.internal/v1".into()),
            api_key: "test-api-key".into(),
            model_options: None,
            notes: None,
            website_url: None,
        }
    }

    fn patch(app: AppKind, key: &str) -> CommonConfigPatch {
        CommonConfigPatch {
            app,
            entries: vec![PatchEntry {
                key: key.into(),
                value: Some(PatchValue::Bool(true)),
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
    fn rejects_zero_context_window_and_blank_available_models() {
        let mut p = profile(AppKind::Codex);
        p.model_options = Some(ModelOptions::Codex(CodexModelSettings {
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
        let err = patch(AppKind::Codex, "model_context_window")
            .validate()
            .unwrap_err();
        assert!(matches!(err, ValidationError::ProfileExclusiveKey { .. }));
        assert!(err.to_string().contains("供应商档案"));
    }

    #[test]
    fn choice_patch_accepts_catalog_values_and_rejects_unknown_ones() {
        let mut entries = patch(AppKind::Codex, "model_reasoning_effort").entries;
        entries[0].value = Some(PatchValue::Str("xhigh".into()));
        assert!(CommonConfigPatch {
            app: AppKind::Codex,
            entries
        }
        .validate()
        .is_ok());

        let mut entries = patch(AppKind::Codex, "model_reasoning_effort").entries;
        entries[0].value = Some(PatchValue::Str("extreme".into()));
        let err = CommonConfigPatch {
            app: AppKind::Codex,
            entries,
        }
        .validate()
        .unwrap_err();
        assert!(matches!(err, ValidationError::BadCommonValue { .. }));
        assert!(err.to_string().contains("minimal"));

        // A non-string value on a choice key is equally invalid.
        let mut entries = patch(AppKind::Codex, "model_reasoning_effort").entries;
        entries[0].value = Some(PatchValue::Bool(true));
        assert!(matches!(
            CommonConfigPatch {
                app: AppKind::Codex,
                entries,
            }
            .validate(),
            Err(ValidationError::BadCommonValue { .. })
        ));

        // An absent value (null) removes the line and is always valid.
        let mut entries = patch(AppKind::Codex, "model_reasoning_effort").entries;
        entries[0].value = None;
        assert!(CommonConfigPatch {
            app: AppKind::Codex,
            entries
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn toggle_patch_must_carry_exactly_its_applied_value() {
        let mut entries = patch(AppKind::Claude, "spinnerTipsEnabled").entries;
        entries[0].value = Some(PatchValue::Bool(false));
        assert!(CommonConfigPatch {
            app: AppKind::Claude,
            entries
        }
        .validate()
        .is_ok());

        let mut entries = patch(AppKind::Claude, "spinnerTipsEnabled").entries;
        entries[0].value = Some(PatchValue::Bool(true));
        assert!(matches!(
            CommonConfigPatch {
                app: AppKind::Claude,
                entries,
            }
            .validate(),
            Err(ValidationError::BadCommonValue { .. })
        ));
    }

    #[test]
    fn accepts_general_patch_keys() {
        assert!(patch(AppKind::Codex, "disable_response_storage")
            .validate()
            .is_ok());
        // tui.animations carries `false` when checked; the catalog decides.
        let mut entries = patch(AppKind::Codex, "tui.animations").entries;
        entries[0].value = Some(PatchValue::Bool(false));
        assert!(CommonConfigPatch {
            app: AppKind::Codex,
            entries
        }
        .validate()
        .is_ok());
        assert!(patch(AppKind::Claude, "alwaysThinkingEnabled")
            .validate()
            .is_ok());
    }

    #[test]
    fn rejects_app_mismatch_between_profile_and_patch() {
        // Claude patches have no general keys left (every owned Claude key is
        // profile-exclusive), so the mismatch is exercised Codex↔Claude with
        // the one valid Codex general key.
        let claude_profile = profile(AppKind::Claude);
        let err = validate_plan(
            &claude_profile,
            &patch(AppKind::Codex, "disable_response_storage"),
        )
        .unwrap_err();
        assert!(matches!(err, ValidationError::AppMismatch { .. }));
    }
}
