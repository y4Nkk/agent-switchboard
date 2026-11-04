use crate::adapter::OverlayEntry;
use crate::contracts::{
    AppKind, ConfigValue, ModelOptions, SettingValue, SettingsValues, SwitchPlan,
};
use crate::ownership::{
    provider_absent_action, setting_specs, ProviderAbsentAction, SettingControl, SettingOwner,
    CODEX_MODEL_CATALOG_KEY, CODEX_PROVIDER_BASE_URL_KEY, CODEX_PROVIDER_ID,
    CODEX_SUBAGENT_MODEL_KEY, CODEX_WEB_SEARCH_KEY,
};

fn absent_provider_entry(key: &str) -> OverlayEntry {
    match provider_absent_action(AppKind::Codex, key) {
        Some(ProviderAbsentAction::Remove) => OverlayEntry::RemoveIfPresent,
        None => unreachable!("Codex provider mapping must be declared in the ownership directory"),
    }
}

fn provider_value(plan: &SwitchPlan, key: &str) -> Option<ConfigValue> {
    let profile = &plan.profile;
    match key {
        "model" => profile.model.clone().map(ConfigValue::Str),
        "model_provider" => Some(ConfigValue::Str(CODEX_PROVIDER_ID.into())),
        CODEX_PROVIDER_BASE_URL_KEY => plan
            .client_base_url()
            .map(|url| ConfigValue::Str(url.into())),
        CODEX_MODEL_CATALOG_KEY => plan
            .codex_model_catalog()
            .map(|pointer| ConfigValue::Str(pointer.to_string())),
        CODEX_SUBAGENT_MODEL_KEY => match &profile.model_options {
            Some(ModelOptions::Codex(settings)) => settings
                .subagent_route
                .as_ref()
                .map(|route| ConfigValue::Str(route.wire_id())),
            None => None,
            Some(ModelOptions::Claude(_)) => {
                unreachable!("profile validation rejects mismatched options")
            }
        },
        "model_context_window" => match &profile.model_options {
            Some(ModelOptions::Codex(settings)) => settings
                .context_window
                .map(|tokens| ConfigValue::Number(tokens as f64)),
            None => None,
            Some(ModelOptions::Claude(_)) => {
                unreachable!("profile validation rejects mismatched options")
            }
        },
        _ => unreachable!("Codex provider mapping must be declared in the ownership directory"),
    }
}

/// Automatic removes the managed key; explicit values are always written.
fn setting_entry(value: &SettingValue) -> OverlayEntry {
    match value {
        SettingValue::Automatic => OverlayEntry::RemoveIfPresent,
        SettingValue::Explicit { value } => OverlayEntry::Set(value.clone()),
    }
}

/// Resolves a stored provider parameter against the selected provider's
/// protocol capability. Codex server-side web search is a Responses-only
/// service tool; a Chat Completions or Anthropic upstream cannot execute it.
/// The client file therefore receives an explicit disabled value only for the
/// routed configuration, while the stored provider parameter remains intact
/// and is restored on the next native Responses projection.
fn effective_parameter_entry(
    profile: &crate::contracts::ProviderProfile,
    key: &str,
    value: &SettingValue,
) -> OverlayEntry {
    let requires_protocol_guard = profile.requires_protocol_translation();
    if key == CODEX_WEB_SEARCH_KEY && requires_protocol_guard {
        return OverlayEntry::Set(ConfigValue::Str("disabled".to_string()));
    }
    setting_entry(value)
}

pub(super) fn client_settings_overlay(
    client_settings: &SettingsValues,
) -> Vec<(String, OverlayEntry)> {
    let mut entries: Vec<_> = setting_specs(AppKind::Codex)
        .into_iter()
        .filter(|spec| spec.owner == SettingOwner::Client)
        .map(|spec| {
            let value = client_settings
                .value(spec.key)
                .expect("client-settings validation guarantees every catalog key");
            (spec.key.to_string(), setting_entry(value))
        })
        .collect();
    entries.push((super::subagents::RETIRED_MAX_THREADS_KEY.into(), OverlayEntry::RemoveIfPresent));
    entries.extend(crate::ownership::RETIRED_CODEX_CLIENT_KEYS.iter()
        .map(|key| ((*key).into(), OverlayEntry::RemoveIfPresent)));
    entries
}

/// Derives all changes by iterating the ownership directory. The only literal
/// names below map declared slots to plan data; no second managed-key list
/// exists in the adapter.
pub(super) fn overlay(plan: &SwitchPlan) -> Vec<(String, OverlayEntry)> {
    let mut entries: Vec<(String, OverlayEntry)> = setting_specs(AppKind::Codex)
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
                    effective_parameter_entry(&plan.profile, spec.key, value)
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
        .collect();
    if plan.writes_client_settings() {
        entries.push((super::subagents::RETIRED_MAX_THREADS_KEY.into(), OverlayEntry::RemoveIfPresent));
        entries.extend(crate::ownership::RETIRED_CODEX_CLIENT_KEYS.iter()
            .map(|key| ((*key).into(), OverlayEntry::RemoveIfPresent)));
    }
    entries.extend(crate::ownership::RETIRED_CODEX_PROVIDER_KEYS.iter()
        .map(|key| ((*key).into(), OverlayEntry::RemoveIfPresent)));
    entries
}
