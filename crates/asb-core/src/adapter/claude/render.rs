use crate::adapter::{AdapterError, OverlayEntry};
use crate::contracts::{SettingsValues, SwitchPlan};

use crate::adapter::claude::document::{parse, remove, set};
use crate::adapter::claude::overlay::{client_settings_overlay, overlay};

pub(crate) fn render(current: &str, plan: &SwitchPlan) -> Result<String, AdapterError> {
    let mut entries = overlay(plan);
    entries.extend(super::native::entries(current, plan)?);
    let rendered = render_entries(current, entries)?;
    let fragment_error = |message: String| AdapterError {
        message,
        line: None,
    };
    if !plan.writes_client_settings() {
        let profile = super::model_preferences::projected_profile(plan)?;
        return crate::claude_common::apply_profile(&rendered, &profile)
            .map_err(fragment_error);
    }
    let (shared, profile) = super::model_preferences::projected(plan)?;
    // Remove the old profile before the shared overlay can claim its leaves.
    let rendered = crate::claude_common::apply_profile(&rendered, &Default::default())
        .map_err(fragment_error)?;
    let rendered = crate::claude_common::apply(&rendered, &shared)
        .map_err(fragment_error)?;
    crate::claude_common::apply_profile(&rendered, &profile)
        .map_err(fragment_error)
}

pub(crate) fn render_gateway_base_url(
    current: &str,
    base_url: &str,
) -> Result<String, AdapterError> {
    render_entries(
        current,
        vec![(
            "env.ANTHROPIC_BASE_URL".to_string(),
            OverlayEntry::Set(crate::contracts::ConfigValue::Str(base_url.to_string())),
        )],
    )
}

pub(crate) fn render_client_settings_into_file(
    current: &str,
    client_settings: &SettingsValues,
) -> Result<String, AdapterError> {
    crate::claude_common::validate_shared_write(current, &client_settings.claude_extra)
        .map_err(|message| AdapterError { message, line: None })?;
    let entries = client_settings_overlay(client_settings);
    let rendered = render_entries(current, entries)?;
    crate::claude_common::apply(&rendered, &client_settings.claude_extra).map_err(|message| {
        AdapterError {
            message,
            line: None,
        }
    })
}

pub(crate) fn reset_client_settings(current: &str) -> Result<String, AdapterError> {
    let defaults = crate::ownership::default_client_settings(crate::AppKind::Claude);
    let entries = client_settings_overlay(&defaults);
    render_entries(current, entries)
}

pub(crate) fn render_entries(
    current: &str,
    entries: Vec<(String, OverlayEntry)>,
) -> Result<String, AdapterError> {
    let mut root = parse(current)?;
    let writes_preferences = entries.iter().any(|(key, _)| crate::ownership::setting_spec(crate::AppKind::Claude, key)
        .is_some_and(|spec| spec.control != crate::ownership::SettingControl::None));
    if entries.iter().any(|(key, _)| key.starts_with("attribution.")) {
        crate::claude_common::normalize_attribution(&mut root);
    }
    for (key, entry) in entries {
        match entry {
            OverlayEntry::Set(value) => set(&mut root, &key, value)?,
            OverlayEntry::Leave => {}
            OverlayEntry::RemoveIfPresent => {
                remove(&mut root, &key)?;
            }
            OverlayEntry::RemoveTableIfEmpty => {}
        }
    }
    if writes_preferences {
        crate::claude_common::reconcile_manifests(&mut root).map_err(|message| AdapterError { message, line: None })?;
    }
    serde_json::to_string_pretty(&root).map_err(|_| AdapterError {
        message: "配置序列化失败".to_string(),
        line: None,
    })
}
