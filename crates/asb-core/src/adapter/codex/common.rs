//! Read only the visual common-file keys from a complete live Codex document.
use crate::{
    adapter::AdapterError,
    contracts::{AppKind, ConfigValue, SettingValue, SettingsValues, MAX_EXACT_CONFIG_INTEGER},
    ownership,
};
use toml_edit::Value as TomlValue;

pub fn extract_client_settings(text: &str) -> Result<SettingsValues, AdapterError> {
    let document = super::parse(text)?;
    let mut settings = ownership::default_client_settings(AppKind::Codex);
    for (key, setting) in &mut settings.settings {
        let mut segments = key.split('.');
        let mut item = segments.next().and_then(|key| document.get(key));
        for segment in segments {
            item = item.and_then(|item| item.get(segment));
        }
        if let Some(item) = item {
            *setting = SettingValue::Explicit {
                value: item_value(item, key)?,
            };
        }
    }
    settings
        .validate_client_settings(AppKind::Codex)
        .map_err(|error| AdapterError {
            message: error.to_string(),
            line: None,
        })?;
    Ok(settings)
}

pub(super) fn item_value(item: &toml_edit::Item, key: &str) -> Result<ConfigValue, AdapterError> {
    if let Some(table) = item.as_table_like() {
        return table.iter().map(|(name, item)| Ok((name.to_string(), item_value(item, key)?)))
            .collect::<Result<std::collections::BTreeMap<_, _>, _>>().map(ConfigValue::Object);
    }
    item.as_value().ok_or_else(|| AdapterError {
        message: format!("配置 {key} 不是可读取的值"), line: None,
    }).and_then(|value| toml_value(value, key))
}

/// Decodes a TOML configuration value without losing integer precision.
pub fn toml_value(value: &TomlValue, key: &str) -> Result<ConfigValue, AdapterError> {
    let integer = ownership::setting_spec(AppKind::Codex, key)
        .is_some_and(|spec| matches!(spec.control, ownership::SettingControl::Integer { .. }));
    if integer && !matches!(value, TomlValue::Integer(_)) {
        return Err(AdapterError { message: format!("配置 {key} 必须是 TOML 整数"), line: None });
    }
    match value {
        TomlValue::Boolean(value) => Ok(ConfigValue::Bool(*value.value())),
        TomlValue::String(value) => Ok(ConfigValue::Str(value.value().clone())),
        TomlValue::Integer(value)
            if (-MAX_EXACT_CONFIG_INTEGER..=MAX_EXACT_CONFIG_INTEGER).contains(value.value()) =>
        {
            Ok(ConfigValue::Number(*value.value() as f64))
        }
        TomlValue::Float(value) if value.value().is_finite() => {
            Ok(ConfigValue::Number(*value.value()))
        }
        TomlValue::Array(items) => items.iter().map(|item| toml_value(item, key))
            .collect::<Result<Vec<_>, _>>().map(ConfigValue::Array),
        TomlValue::InlineTable(table) => table.iter().map(|(name, value)|
            Ok((name.to_string(), toml_value(value, key)?)))
            .collect::<Result<std::collections::BTreeMap<_, _>, _>>().map(ConfigValue::Object),
        _ => Err(AdapterError {
            message: format!("配置 {key} 必须是受支持的标量或数组值"),
            line: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extraction_does_not_copy_routing_mcp_agents_secrets_or_project_overrides() {
        let text = "approval_policy = 'on-request'\nmodel = 'private-model'\n[model_providers.third]\nexperimental_bearer_token = 'secret'\n[mcp_servers.test]\ncommand = 'node'\nargs = ['a','b']\n[projects.'C:/private']\ntrust_level = 'trusted'\n";
        let settings = extract_client_settings(text).unwrap();
        let encoded = serde_json::to_string(&settings).unwrap();
        assert!(encoded.contains("on-request"));
        for value in [
            "private-model",
            "secret",
            "mcp_servers",
            "C:/private",
            "experimental_bearer_token",
        ] {
            assert!(!encoded.contains(value));
        }
    }

    #[test]
    fn malformed_owned_values_fail_instead_of_silently_becoming_automatic() {
        assert!(extract_client_settings("approval_policy = ['on-request']").is_err());
        assert!(extract_client_settings("approval_policy = 'unsupported'").is_err());
    }
}
