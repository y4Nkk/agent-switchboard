use crate::adapter::OverlayEntry;
use crate::contracts::{
    AppKind, AuthenticationScheme, ConfigValue, ModelOptions, SettingValue, SettingsValues,
    SwitchPlan, UpstreamProtocol,
};
use crate::ownership::{
    provider_absent_action, setting_specs, ProviderAbsentAction, SettingControl, SettingOwner,
};

/// Deprecated Claude Code model key superseded by
/// `ANTHROPIC_DEFAULT_HAIKU_MODEL`. It remains a profile-owned cleanup key
/// in the ownership directory so it cannot survive a provider switch.
pub(super) const DEPRECATED_MODEL_KEY: &str = "env.ANTHROPIC_SMALL_FAST_MODEL";
/// Environment model key that silently overrides the top-level `model`;
/// removed whenever a profile declares a primary model.
pub(super) const ENV_MODEL_KEY: &str = "env.ANTHROPIC_MODEL";

fn absent_provider_entry(key: &str) -> OverlayEntry {
    match provider_absent_action(AppKind::Claude, key) {
        Some(ProviderAbsentAction::Remove) => OverlayEntry::RemoveIfPresent,
        None => unreachable!("Claude provider mapping must be declared in the ownership directory"),
    }
}

fn claude_settings(
    profile: &crate::contracts::ProviderProfile,
) -> Option<&crate::contracts::ClaudeModelSettings> {
    match &profile.model_options {
        Some(ModelOptions::Claude(settings)) => Some(settings),
        None => None,
        Some(ModelOptions::Codex(_)) => {
            unreachable!("profile validation rejects mismatched options")
        }
    }
}

fn rendered_model(value: Option<&String>, one_m: bool) -> Option<ConfigValue> {
    value.map(|model| ConfigValue::Str(crate::claude_model::render_model(model, one_m)))
}

fn provider_value(plan: &SwitchPlan, key: &str) -> Option<ConfigValue> {
    let profile = &plan.profile;
    if profile.route_mode == crate::contracts::RouteMode::Official {
        return None;
    }
    let settings = claude_settings(profile);
    if plan.is_gateway() {
        if let Some(value) = super::gateway::model_value(plan, key, settings) {
            return value;
        }
    }
    match key {
        crate::ownership::CLAUDE_GATEWAY_REVISION_KEY => plan
            .claude_route_revision()
            .map(|value| ConfigValue::Str(value.into())),
        "model" => rendered_model(
            profile.model.as_ref(),
            settings.is_some_and(|value| value.primary_one_m),
        ),
        "availableModels" => settings.and_then(|value| {
            value.available_models.as_ref().map(|models| {
                ConfigValue::Array(models.iter().cloned().map(ConfigValue::Str).collect())
            })
        }),
        "env.ANTHROPIC_BASE_URL" if profile.connection.claude_native.is_some() => None,
        "env.ANTHROPIC_BASE_URL" => plan
            .client_base_url()
            .map(|url| ConfigValue::Str(url.into())),
        "env.ANTHROPIC_AUTH_TOKEN" => (plan.client_authentication()
            == Some(AuthenticationScheme::Bearer))
        .then(|| ConfigValue::Str(plan.client_api_key().into())),
        "env.ANTHROPIC_API_KEY" => (plan.client_authentication()
            == Some(AuthenticationScheme::XApiKey))
        .then(|| ConfigValue::Str(plan.client_api_key().into())),
        "env.CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS" => profile
            .upstream_protocol
            .filter(|protocol| *protocol != UpstreamProtocol::native_for(AppKind::Claude))
            .map(|_| ConfigValue::Str("1".to_string())),
        // This older override must be removed for any current provider.
        ENV_MODEL_KEY | DEPRECATED_MODEL_KEY => None,
        "env.ANTHROPIC_DEFAULT_HAIKU_MODEL" => rendered_model(
            settings.and_then(|value| value.haiku_model.as_ref()),
            settings.is_some_and(|value| value.haiku_one_m),
        ),
        "env.ANTHROPIC_DEFAULT_SONNET_MODEL" => rendered_model(
            settings.and_then(|value| value.sonnet_model.as_ref()),
            settings.is_some_and(|value| value.sonnet_one_m),
        ),
        "env.ANTHROPIC_DEFAULT_OPUS_MODEL" => rendered_model(
            settings.and_then(|value| value.opus_model.as_ref()),
            settings.is_some_and(|value| value.opus_one_m),
        ),
        "env.ANTHROPIC_DEFAULT_FABLE_MODEL" => rendered_model(
            settings.and_then(|value| value.fable_model.as_ref()),
            settings.is_some_and(|value| value.fable_one_m),
        ),
        "env.CLAUDE_CODE_SUBAGENT_MODEL" => rendered_model(
            settings.and_then(|value| value.subagent_model.as_ref()),
            settings.is_some_and(|value| value.subagent_one_m),
        ),
        "env.ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME"
        | "env.ANTHROPIC_DEFAULT_SONNET_MODEL_NAME"
        | "env.ANTHROPIC_DEFAULT_OPUS_MODEL_NAME"
        | "env.ANTHROPIC_DEFAULT_FABLE_MODEL_NAME" => model_name(settings, key),
        _ => unreachable!("Claude provider mapping must be declared in the ownership directory"),
    }
}

fn model_name(
    settings: Option<&crate::contracts::ClaudeModelSettings>,
    key: &str,
) -> Option<ConfigValue> {
    let names = settings?.display_names.as_ref()?;
    let name = match key {
        "env.ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME" => &names.haiku,
        "env.ANTHROPIC_DEFAULT_SONNET_MODEL_NAME" => &names.sonnet,
        "env.ANTHROPIC_DEFAULT_OPUS_MODEL_NAME" => &names.opus,
        "env.ANTHROPIC_DEFAULT_FABLE_MODEL_NAME" => &names.fable,
        _ => unreachable!("model display name key"),
    };
    name.clone().map(ConfigValue::Str)
}

/// Setting intent is explicit: automatic removes a previously managed
/// key, while an explicit value is always written.
fn setting_entry(value: &SettingValue) -> OverlayEntry {
    match value {
        SettingValue::Automatic => OverlayEntry::RemoveIfPresent,
        SettingValue::Explicit { value } => OverlayEntry::Set(value.clone()),
    }
}

pub(super) fn client_settings_overlay(
    client_settings: &SettingsValues,
) -> Vec<(String, OverlayEntry)> {
    setting_specs(AppKind::Claude)
        .into_iter()
        .filter(|spec| spec.owner == SettingOwner::Client)
        .map(|spec| {
            let value = client_settings
                .value(spec.key)
                .expect("client-settings validation guarantees every catalog key");
            (spec.key.to_string(), setting_entry(value))
        })
        .collect()
}

/// Builds all changes by iterating the ownership directory. Literal keys only
/// map the declared slots to plan data; ownership and cleanup actions
/// themselves remain owned by that directory.
pub(super) fn overlay(plan: &SwitchPlan) -> Vec<(String, OverlayEntry)> {
    setting_specs(AppKind::Claude)
        .into_iter()
        .filter(|spec| spec.owner != SettingOwner::Client || plan.writes_client_settings())
        .map(|spec| {
            let entry = match spec.owner {
                SettingOwner::Provider if spec.control != SettingControl::None => {
                    let value = plan
                        .profile
                        .parameters
                        .value(spec.key)
                        .expect("plan validation guarantees complete provider parameters");
                    setting_entry(value)
                }
                SettingOwner::Provider => provider_value(plan, spec.key)
                    .map(OverlayEntry::Set)
                    .unwrap_or_else(|| absent_provider_entry(spec.key)),
                SettingOwner::Client => {
                    let value = plan
                        .client_settings
                        .value(spec.key)
                        .expect("plan validation guarantees complete client settings");
                    setting_entry(value)
                }
                SettingOwner::Host => unreachable!("host keys never appear in the directory"),
            };
            (spec.key.to_string(), entry)
        })
        .collect()
}
