use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) type Attributes = BTreeMap<String, Value>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Attribute { key: String, #[serde(default)] value: Value }

pub(super) fn attributes(value: Option<&Value>) -> Result<Attributes, String> {
    let Some(value) = value else { return Ok(BTreeMap::new()); };
    let items: Vec<Attribute> = serde_json::from_value(value.clone())
        .map_err(|_| "OTLP attributes 必须是键值数组".to_string())?;
    let mut result = BTreeMap::new();
    for item in items {
        if result.contains_key(&item.key) { return Err("OTLP attributes 包含重复键".into()); }
        if item.value.is_null() { continue; }
        let object = item.value.as_object().ok_or("OTLP AnyValue 无效")?;
        if object.is_empty() { continue; }
        if object.len() != 1 { return Err("OTLP AnyValue 必须只有一种值类型".into()); }
        let (kind, value) = object.iter().next().ok_or("OTLP AnyValue 为空")?;
        if matches!(kind.as_str(), "stringValue" | "intValue" | "doubleValue" | "boolValue") {
            result.insert(item.key, value.clone());
        }
    }
    Ok(result)
}

pub(super) fn text(fields: &Attributes, key: &str) -> Option<String> {
    fields.get(key).and_then(Value::as_str).filter(|s| !s.is_empty() && s.len() <= 512
        && !s.chars().any(char::is_control)).map(str::to_string)
}

pub(super) fn number(fields: &Attributes, key: &str) -> Option<f64> {
    let value = fields.get(key)?;
    let result = value.as_f64().or_else(|| value.as_str()?.parse().ok())?;
    (result.is_finite() && result >= 0.0).then_some(result)
}

pub(super) fn integer(fields: &Attributes, key: &str) -> Option<u64> {
    let value = fields.get(key)?;
    value.as_u64().or_else(|| value.as_str()?.parse().ok())
}

pub(super) fn boolean(fields: &Attributes, key: &str) -> Option<bool> {
    match fields.get(key)? {
        Value::Bool(value) => Some(*value),
        Value::String(value) if value == "true" => Some(true),
        Value::String(value) if value == "false" => Some(false),
        _ => None,
    }
}

pub(super) fn nanos(value: Option<&Value>) -> Result<u64, String> {
    let value = value.and_then(Value::as_str).and_then(|text| text.parse::<u64>().ok())
        .filter(|value| *value > 0).ok_or("OTLP 时间必须是有效的纳秒十进制字符串")?;
    Ok(value)
}

pub(super) fn log_time(log: &Value) -> Result<(u64, super::NativeTimeSource), String> {
    // OTLP permits an unset event timestamp; the SDK's observed timestamp is a separate native fact.
    if let Some(value) = log.get("timeUnixNano").filter(|value| value.as_str() != Some("0") && !value.is_null()) {
        return nanos(Some(value)).map(|time| (time, super::NativeTimeSource::Event));
    }
    nanos(log.get("observedTimeUnixNano")).map(|time| (time, super::NativeTimeSource::Observed))
}

pub(super) fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    let object = value.as_object().ok_or("OTLP 消息必须是对象")?;
    match object.get(key) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(items)) => Ok(items),
        _ => Err(format!("OTLP {key} 必须是数组")),
    }
}

pub(super) fn identity(value: &Value, key: &str, length: usize) -> Option<String> {
    let text = value.get(key)?.as_str()?;
    (text.len() == length && text.bytes().all(|b| b.is_ascii_hexdigit())
        && text.bytes().any(|b| b != b'0')).then(|| text.to_ascii_lowercase())
}
