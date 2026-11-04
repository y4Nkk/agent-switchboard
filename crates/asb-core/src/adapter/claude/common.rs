//! Read only the visual client-preference keys from live Claude settings.
use crate::{
    adapter::AdapterError,
    contracts::{AppKind, ConfigValue, SettingValue, SettingsValues, MAX_EXACT_CONFIG_INTEGER},
    ownership,
};

pub fn extract_client_settings(text: &str) -> Result<SettingsValues, AdapterError> {
    let mut document = super::parse(text)?;
    crate::claude_common::normalize_attribution(&mut document);
    let mut settings = ownership::default_client_settings(AppKind::Claude);
    for (key, setting) in &mut settings.settings {
        if let Some(value) = value_at(&document, key)? {
            *setting = SettingValue::Explicit {
                value: json_value(value, key)?,
            };
        }
    }
    settings
        .validate_client_settings(AppKind::Claude)
        .map_err(|error| AdapterError {
            message: error.to_string(),
            line: None,
        })?;
    Ok(settings)
}

fn value_at<'a>(document: &'a serde_json::Value, key: &str) -> Result<Option<&'a serde_json::Value>, AdapterError> {
    let mut current = document;
    for segment in key.split('.') {
        let object = current.as_object().ok_or_else(|| unsupported_value(key))?;
        let Some(value) = object.get(segment) else { return Ok(None); };
        current = value;
    }
    Ok(Some(current))
}

pub(crate) fn json_value(value: &serde_json::Value, key: &str) -> Result<ConfigValue, AdapterError> {
    match value {
        serde_json::Value::Bool(value) => Ok(ConfigValue::Bool(*value)),
        serde_json::Value::String(value) => Ok(ConfigValue::Str(value.clone())),
        serde_json::Value::Number(value) => json_number(value, key),
        serde_json::Value::Array(items) => items.iter().map(|item| json_value(item, key))
            .collect::<Result<Vec<_>, _>>().map(ConfigValue::Array),
        serde_json::Value::Null | serde_json::Value::Object(_) => {
            Err(unsupported_value(key))
        }
    }
}

fn json_number(value: &serde_json::Number, key: &str) -> Result<ConfigValue, AdapterError> {
    if let Some(integer) = value.as_i64() {
        return exact_integer(integer, key);
    }
    if let Some(integer) = value.as_u64() {
        return (integer <= MAX_EXACT_CONFIG_INTEGER as u64)
            .then_some(ConfigValue::Number(integer as f64))
            .ok_or_else(|| unsupported_value(key));
    }
    value
        .as_f64()
        .filter(|number| number.is_finite())
        .map(ConfigValue::Number)
        .ok_or_else(|| unsupported_value(key))
}

fn exact_integer(value: i64, key: &str) -> Result<ConfigValue, AdapterError> {
    (-MAX_EXACT_CONFIG_INTEGER..=MAX_EXACT_CONFIG_INTEGER)
        .contains(&value)
        .then_some(ConfigValue::Number(value as f64))
        .ok_or_else(|| unsupported_value(key))
}

fn unsupported_value(key: &str) -> AdapterError {
    AdapterError {
        message: format!("通用配置 {key} 的结构或值类型不受支持"),
        line: None,
    }
}
