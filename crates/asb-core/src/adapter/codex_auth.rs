//! Codex file-backed API-key credential adapter.
//!
//! This module owns the small JSON shape written by `codex login
//! --with-api-key`. It is deliberately pure: the switch executor is the only
//! caller allowed to put the rendered text into the real `auth.json`.

use crate::adapter::AdapterError;
use crate::contracts::{ChangeKind, KeyChange, RouteMode, SwitchPlan};
use crate::redact::redact;
use serde_json::{Map, Value};

const AUTH_MODE_KEY: &str = "auth_mode";
const API_KEY_KEY: &str = "OPENAI_API_KEY";
const API_KEY_MODE: &str = "apikey";
const CHATGPT_MODE: &str = "chatgpt";

pub(crate) fn matches_provider_identity(
    current: Option<&str>,
    profile: &crate::contracts::ProviderProfile,
) -> Result<bool, AdapterError> {
    let root = parse(current.unwrap_or_default())?;
    let mode = root.get(AUTH_MODE_KEY).and_then(Value::as_str);
    let key = root.get(API_KEY_KEY).and_then(Value::as_str);
    Ok(match profile.route_mode {
        RouteMode::Official => mode != Some(API_KEY_MODE)
            && (mode == Some(CHATGPT_MODE) || key.is_none_or(str::is_empty)),
        RouteMode::Custom => mode == Some(API_KEY_MODE)
            && !profile.api_key.is_empty()
            && key == Some(profile.api_key.as_str()),
    })
}

fn parse(current: &str) -> Result<Map<String, Value>, AdapterError> {
    if current.is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(current) {
        Ok(Value::Object(root)) => Ok(root),
        Ok(_) => Err(AdapterError {
            message: "Codex 登录缓存必须是 JSON 对象".to_string(),
            line: None,
        }),
        Err(_) => Err(AdapterError {
            message: "Codex 登录缓存 JSON 格式无效".to_string(),
            line: None,
        }),
    }
}

fn desired_values(plan: &SwitchPlan) -> Result<[(String, Value); 2], AdapterError> {
    if plan.app() != crate::AppKind::Codex {
        return Err(AdapterError {
            message: "只有 Codex 切换可以更新 Codex 登录缓存".to_string(),
            line: None,
        });
    }
    Ok(match plan.profile.route_mode {
        RouteMode::Custom => [
            (
                AUTH_MODE_KEY.to_string(),
                Value::String(API_KEY_MODE.to_string()),
            ),
            (
                API_KEY_KEY.to_string(),
                Value::String(plan.profile.api_key.clone()),
            ),
        ],
        RouteMode::Official => [
            (
                AUTH_MODE_KEY.to_string(),
                Value::String(CHATGPT_MODE.to_string()),
            ),
            (API_KEY_KEY.to_string(), Value::Null),
        ],
    })
}

#[derive(Debug)]
struct RootField {
    key: String,
    value_start: usize,
    value_end: usize,
}

#[derive(Debug)]
struct RootObject {
    closing_brace: usize,
    fields: Vec<RootField>,
}

fn malformed_cache() -> AdapterError {
    AdapterError {
        message: "Codex 登录缓存 JSON 格式无效".to_string(),
        line: None,
    }
}

fn skip_whitespace(text: &str, mut index: usize) -> usize {
    while text
        .as_bytes()
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        index += 1;
    }
    index
}

fn scan_string(text: &str, start: usize) -> Result<usize, AdapterError> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'\"') {
        return Err(malformed_cache());
    }
    let mut index = start + 1;
    while let Some(byte) = bytes.get(index) {
        match byte {
            b'\\' => index += 2,
            b'\"' => return Ok(index + 1),
            _ => index += 1,
        }
    }
    Err(malformed_cache())
}

fn scan_value(text: &str, start: usize) -> Result<usize, AdapterError> {
    let bytes = text.as_bytes();
    let mut index = start;
    let mut depth = 0usize;
    let mut in_string = false;
    while let Some(byte) = bytes.get(index) {
        if in_string {
            match byte {
                b'\\' => index += 2,
                b'\"' => {
                    in_string = false;
                    index += 1;
                }
                _ => index += 1,
            }
            continue;
        }
        match byte {
            b'\"' => {
                in_string = true;
                index += 1;
            }
            b'{' | b'[' => {
                depth += 1;
                index += 1;
            }
            b'}' | b']' if depth > 0 => {
                depth -= 1;
                index += 1;
            }
            b',' | b'}' if depth == 0 => return Ok(index),
            _ => index += 1,
        }
    }
    Err(malformed_cache())
}

fn root_object(current: &str) -> Result<RootObject, AdapterError> {
    parse(current)?;
    let bytes = current.as_bytes();
    let mut index = skip_whitespace(current, 0);
    if bytes.get(index) != Some(&b'{') {
        return Err(AdapterError {
            message: "Codex 登录缓存必须是 JSON 对象".to_string(),
            line: None,
        });
    }
    index += 1;
    let mut fields = Vec::new();
    loop {
        index = skip_whitespace(current, index);
        if bytes.get(index) == Some(&b'}') {
            return Ok(RootObject {
                closing_brace: index,
                fields,
            });
        }
        let key_start = index;
        let key_end = scan_string(current, key_start)?;
        let key =
            serde_json::from_str(&current[key_start..key_end]).map_err(|_| malformed_cache())?;
        index = skip_whitespace(current, key_end);
        if bytes.get(index) != Some(&b':') {
            return Err(malformed_cache());
        }
        index = skip_whitespace(current, index + 1);
        let value_start = index;
        let value_boundary = scan_value(current, value_start)?;
        let value_end = current[..value_boundary].trim_end().len();
        fields.push(RootField {
            key,
            value_start,
            value_end,
        });
        index = skip_whitespace(current, value_boundary);
        match bytes.get(index) {
            Some(b',') => index += 1,
            Some(b'}') => {
                return Ok(RootObject {
                    closing_brace: index,
                    fields,
                });
            }
            _ => return Err(malformed_cache()),
        }
    }
}

fn render_value(value: &Value) -> Result<String, AdapterError> {
    serde_json::to_string(value).map_err(|_| AdapterError {
        message: "Codex 登录缓存序列化失败".to_string(),
        line: None,
    })
}

fn replace_managed_fields(
    current: &str,
    root: RootObject,
    desired: &[(String, Value); 2],
) -> Result<String, AdapterError> {
    let mut edits = Vec::new();
    let mut missing = Vec::new();
    for (key, value) in desired {
        let matches = root
            .fields
            .iter()
            .filter(|field| field.key == *key)
            .collect::<Vec<_>>();
        if matches.len() > 1 {
            return Err(AdapterError {
                message: format!("Codex 登录缓存包含重复的受管字段：{key}"),
                line: None,
            });
        }
        if let Some(field) = matches.first() {
            edits.push((field.value_start, field.value_end, render_value(value)?));
        } else {
            missing.push((key, render_value(value)?));
        }
    }
    if !missing.is_empty() {
        if root.fields.is_empty() {
            return Ok(format!(
                "{{\n  \"{}\": {},\n  \"{}\": {}\n}}",
                desired[0].0,
                render_value(&desired[0].1)?,
                desired[1].0,
                render_value(&desired[1].1)?,
            ));
        }
        let insertion = current[..root.closing_brace].trim_end().len();
        let separator = if current.contains('\n') || current.contains('\r') {
            "\n  "
        } else {
            " "
        };
        let added = missing
            .iter()
            .map(|(key, value)| format!("\"{key}\": {value}"))
            .collect::<Vec<_>>()
            .join(&format!(",{separator}"));
        edits.push((insertion, insertion, format!(",{separator}{added}")));
    }
    edits.sort_by_key(|(start, _, _)| *start);
    let mut rendered = String::with_capacity(current.len() + 64);
    let mut cursor = 0;
    for (start, end, replacement) in edits {
        rendered.push_str(&current[cursor..start]);
        rendered.push_str(&replacement);
        cursor = end;
    }
    rendered.push_str(&current[cursor..]);
    Ok(rendered)
}

fn display_value(value: Option<&Value>) -> Option<String> {
    value.map(|value| match value {
        Value::String(value) => value.clone(),
        value => value.to_string(),
    })
}

fn change(key: &str, before: Option<&Value>, after: Option<&Value>) -> Option<KeyChange> {
    let before = display_value(before);
    let after = display_value(after);
    if before == after {
        return None;
    }
    let redact_value = |value: String| {
        if key == AUTH_MODE_KEY {
            value
        } else {
            redact(&format!("auth.json.{key}"), &value)
        }
    };
    Some(KeyChange {
        key: format!("auth.json.{key}"),
        kind: ChangeKind::Set,
        before: before.map(redact_value),
        after: after.map(redact_value),
    })
}

pub fn preview(current: &str, plan: &SwitchPlan) -> Result<Vec<KeyChange>, AdapterError> {
    let before = parse(current)?;
    let desired = desired_values(plan)?;
    if current.is_empty() && plan.profile.route_mode == RouteMode::Official {
        return Ok(Vec::new());
    }
    Ok(desired
        .iter()
        .filter_map(|(key, value)| change(key, before.get(key), Some(value)))
        .collect())
}

pub fn render(current: &str, plan: &SwitchPlan) -> Result<String, AdapterError> {
    if current.is_empty() && plan.profile.route_mode == RouteMode::Official {
        return Ok(String::new());
    }
    let desired = desired_values(plan)?;
    if current.is_empty() {
        return Ok(format!(
            "{{\n  \"{}\": {},\n  \"{}\": {}\n}}",
            desired[0].0,
            render_value(&desired[0].1)?,
            desired[1].0,
            render_value(&desired[1].1)?,
        ));
    }
    replace_managed_fields(current, root_object(current)?, &desired)
}

pub fn validate(text: &str) -> Result<(), AdapterError> {
    parse(text).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{AppKind, ProviderProfile};
    use crate::ownership::default_common_settings;

    fn plan(mode: RouteMode) -> SwitchPlan {
        SwitchPlan {
            profile: ProviderProfile {
                id: "p".to_string(),
                app: AppKind::Codex,
                route_mode: mode,
                name: "Relay".to_string(),
                model: (mode == RouteMode::Custom).then(|| "gpt-5".to_string()),
                base_url: (mode == RouteMode::Custom)
                    .then(|| "https://relay.example/v1".to_string()),
                api_key: (mode == RouteMode::Custom)
                    .then(|| "TEST_API_KEY".to_string())
                    .unwrap_or_default(),
                model_options: None,
                notes: None,
                website_url: None,
                usage_query: None,
            },
            common: default_common_settings(AppKind::Codex),
        }
    }

    #[test]
    fn custom_route_uses_codex_api_key_login_shape_and_redacts_preview() {
        let current = r#"{"tokens":{"access_token":"official"},"host_note":"keep"}"#;
        let changes = preview(current, &plan(RouteMode::Custom)).expect("preview");
        assert!(changes
            .iter()
            .any(|change| change.key == "auth.json.auth_mode"));
        let key = changes
            .iter()
            .find(|change| change.key == "auth.json.OPENAI_API_KEY")
            .expect("API-key change");
        assert_eq!(key.after.as_deref(), Some(crate::redact::REDACTED));

        let rendered = render(current, &plan(RouteMode::Custom)).expect("render");
        let value: Value = serde_json::from_str(&rendered).expect("JSON");
        assert_eq!(value[AUTH_MODE_KEY], API_KEY_MODE);
        assert_eq!(value[API_KEY_KEY], "TEST_API_KEY");
        assert_eq!(value["tokens"]["access_token"], "official");
        assert_eq!(value["host_note"], "keep");
    }

    #[test]
    fn official_route_restores_chatgpt_mode_without_discarding_tokens() {
        let current = r#"{"auth_mode":"apikey","OPENAI_API_KEY":"third-party","tokens":{"access_token":"official"}}"#;
        let rendered = render(current, &plan(RouteMode::Official)).expect("render");
        let value: Value = serde_json::from_str(&rendered).expect("JSON");
        assert_eq!(value[AUTH_MODE_KEY], CHATGPT_MODE);
        assert!(value[API_KEY_KEY].is_null());
        assert_eq!(value["tokens"]["access_token"], "official");
    }

    #[test]
    fn malformed_auth_cache_is_rejected_without_a_candidate() {
        assert!(render("{not-json", &plan(RouteMode::Custom)).is_err());
    }

    #[test]
    fn official_route_keeps_a_missing_auth_cache_missing() {
        assert_eq!(render("", &plan(RouteMode::Official)).unwrap(), "");
        assert!(preview("", &plan(RouteMode::Official)).unwrap().is_empty());
    }

    #[test]
    fn render_replaces_only_the_two_owned_root_values() {
        let current = concat!(
            "{\r\n",
            "  \"host_note\" : \"keep\\/this spelling\",\r\n",
            "  \"auth_mode\" : \"apikey\",\r\n",
            "  \"tokens\" : { \"access_token\" : \"official\\u0020token\" },\r\n",
            "  \"OPENAI_API_KEY\" : \"old-key\"\r\n",
            "}"
        );

        let rendered = render(current, &plan(RouteMode::Custom)).expect("render");

        assert!(rendered.contains("\"host_note\" : \"keep\\/this spelling\""));
        assert!(rendered.contains("\"tokens\" : { \"access_token\" : \"official\\u0020token\" }"));
        assert!(rendered.contains("\"auth_mode\" : \"apikey\""));
        assert!(rendered.contains("\"OPENAI_API_KEY\" : \"TEST_API_KEY\""));
        assert!(!rendered.contains("old-key"));
        serde_json::from_str::<Value>(&rendered).expect("valid JSON");
    }

    #[test]
    fn render_inserts_a_missing_owned_field_before_trailing_root_whitespace() {
        let current = "{\n  \"auth_mode\": \"apikey\"\n}\n";
        let rendered = render(current, &plan(RouteMode::Custom)).expect("render");
        let value: Value = serde_json::from_str(&rendered).expect("valid JSON");
        assert_eq!(value[AUTH_MODE_KEY], API_KEY_MODE);
        assert_eq!(value[API_KEY_KEY], "TEST_API_KEY");
    }
}
