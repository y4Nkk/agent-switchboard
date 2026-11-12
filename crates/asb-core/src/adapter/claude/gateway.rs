//! Stable client model roles for an already-running Claude process.

use crate::contracts::ClaudeModelRole;
use crate::contracts::{ClaudeModelSettings, ConfigValue, SwitchPlan};

/// Resolves only internal wire roles, using the profile's configured models.
/// Projection and request dispatch must use the same model identity.
pub fn gateway_model_source<'a>(
    role: &str,
    primary: Option<&'a str>,
    settings: Option<&'a ClaudeModelSettings>,
) -> Result<Option<&'a str>, String> {
    let selected = match role {
        "asb-claude-primary" => primary,
        "asb-claude-haiku" => settings
            .and_then(|s| s.model(ClaudeModelRole::Haiku))
            .or(primary),
        "asb-claude-sonnet" => settings
            .and_then(|s| s.model(ClaudeModelRole::Sonnet))
            .or(primary),
        "asb-claude-opus" => settings
            .and_then(|s| s.model(ClaudeModelRole::Opus))
            .or(primary),
        "asb-claude-fable" => settings
            .and_then(|s| {
                s.effective_route(ClaudeModelRole::Fable)
                    .map(|route| route.model())
            })
            .or(primary),
        "asb-claude-subagent" => settings
            .and_then(|s| s.model(ClaudeModelRole::Subagent))
            .or(primary),
        _ if role.starts_with("asb-claude-") => {
            return Err(format!("未知 Claude 网关模型角色 {role}"))
        }
        _ => return Ok(None),
    };
    selected
        .map(Some)
        .ok_or_else(|| format!("Claude 网关模型角色 {role} 没有配置对应模型"))
}

pub(super) fn model_value(
    plan: &SwitchPlan,
    key: &str,
    settings: Option<&ClaudeModelSettings>,
) -> Option<Option<ConfigValue>> {
    let has_default = plan.profile.model.is_some();
    let (role, one_m) = match key {
        "model" if plan.profile.model.is_some() => (
            "asb-claude-primary",
            settings.is_some_and(|s| s.primary_one_m),
        ),
        "env.ANTHROPIC_DEFAULT_HAIKU_MODEL"
            if has_default
                || settings.is_some_and(|s| s.route(ClaudeModelRole::Haiku).is_some()) =>
        {
            (
                "asb-claude-haiku",
                settings.is_some_and(|s| s.one_m(ClaudeModelRole::Haiku)),
            )
        }
        "env.ANTHROPIC_DEFAULT_SONNET_MODEL"
            if has_default
                || settings.is_some_and(|s| s.route(ClaudeModelRole::Sonnet).is_some()) =>
        {
            (
                "asb-claude-sonnet",
                settings.is_some_and(|s| s.one_m(ClaudeModelRole::Sonnet)),
            )
        }
        "env.ANTHROPIC_DEFAULT_OPUS_MODEL"
            if has_default
                || settings.is_some_and(|s| s.route(ClaudeModelRole::Opus).is_some()) =>
        {
            (
                "asb-claude-opus",
                settings.is_some_and(|s| s.one_m(ClaudeModelRole::Opus)),
            )
        }
        "env.ANTHROPIC_DEFAULT_FABLE_MODEL"
            if has_default
                || settings.is_some_and(|s| s.route(ClaudeModelRole::Fable).is_some()) =>
        {
            (
                "asb-claude-fable",
                settings.is_some_and(|s| s.one_m(ClaudeModelRole::Fable)),
            )
        }
        "env.CLAUDE_CODE_SUBAGENT_MODEL"
            if settings.is_some_and(|s| s.route(ClaudeModelRole::Subagent).is_some()) =>
        {
            (
                "asb-claude-subagent",
                settings.is_some_and(|s| s.one_m(ClaudeModelRole::Subagent)),
            )
        }
        _ => return None,
    };
    Some(Some(ConfigValue::Str(crate::claude_model::render_model(
        role, one_m,
    ))))
}
