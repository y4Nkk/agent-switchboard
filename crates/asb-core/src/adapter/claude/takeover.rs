//! Restore Claude routing and its model preferences, preserving shared preferences.

use super::document::{get, parse, remove, set, validate_path};
use crate::ownership::{setting_specs, SettingControl, SettingOwner};
use crate::{
    adapter::AdapterError,
    contracts::{AppKind, ConfigValue},
};

pub fn restore_gateway_overlay(current: &str, before: &str) -> Result<String, AdapterError> {
    let mut current = parse(current)?;
    let before_root = parse(before)?;
    for spec in setting_specs(AppKind::Claude)
        .into_iter()
        .filter(|spec| spec.owner == SettingOwner::Provider && spec.control == SettingControl::None)
    {
        validate_path(&current, spec.key)?;
        match get(&before_root, spec.key).filter(|value| !value.is_null()) {
            Some(value) => {
                let value: ConfigValue =
                    serde_json::from_value(value.clone()).map_err(|_| AdapterError {
                        message: format!("接管前的 {} 不是有效配置值", spec.key),
                        line: None,
                    })?;
                set(&mut current, spec.key, value)?;
            }
            None => remove(&mut current, spec.key)?,
        }
    }
    let rendered = serde_json::to_string_pretty(&current).map_err(|_| AdapterError {
        message: "无法生成 Claude 接管恢复配置".into(),
        line: None,
    })?;
    crate::claude_common::restore_profile_models(&rendered, before)
        .map_err(|message| AdapterError { message, line: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    #[test]
    fn restores_connection_but_preserves_preferences_extensions_and_external_keys() {
        let before = json!({"model":"original", "env":{"ANTHROPIC_BASE_URL":"https://original.example", "ANTHROPIC_API_KEY":"fake-key"}});
        let current = json!({"model":"asb-claude-primary", "env":{"ANTHROPIC_BASE_URL":"http://127.0.0.1:47821", "ANTHROPIC_AUTH_TOKEN":"local", "ASB_CLAUDE_ROUTE_REVISION":"r", "HOST":"keep"}, "modelSettings":{"native.model":{"effortLevel":"high"}}, "language":"zh", "hooks":{"other":true}});
        let restored: Value = serde_json::from_str(
            &restore_gateway_overlay(&current.to_string(), &before.to_string()).unwrap(),
        )
        .unwrap();
        assert_eq!(restored["model"], "original");
        assert_eq!(restored["env"]["ANTHROPIC_API_KEY"], "fake-key");
        assert!(restored["env"].get("ANTHROPIC_AUTH_TOKEN").is_none());
        assert!(restored["env"].get("ASB_CLAUDE_ROUTE_REVISION").is_none());
        assert_eq!(restored["env"]["HOST"], "keep");
        assert_eq!(restored["language"], "zh");
        assert_eq!(restored["modelSettings"], current["modelSettings"]);
        assert_eq!(restored["hooks"], current["hooks"]);
    }
}
