use asb_core::contracts::{AppKind, ConfigValue, SettingValue, SettingsValues};
use asb_core::ownership::{self, SettingOwner};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn fill_automatic(node: &mut Value, app: AppKind, provider: bool) -> Result<bool, String> {
    let mut values: SettingsValues = serde_json::from_value(node.clone())
        .map_err(|error| format!("设置结构无法解析：{error}"))?;
    if app == AppKind::Codex {
        if provider {
            for key in ownership::RETIRED_CODEX_PROVIDER_KEYS { values.settings.remove(*key); }
        } else {
            for (old, current) in [("disable_paste_burst", "tui.disable_paste_burst"), ("tools.view_image", "features.view_image")] {
                if let Some(value) = values.settings.remove(old) {
                    values.settings.entry(current.into()).or_insert(value);
                }
            }
            values.settings.remove("features.unified_exec");
        }
    } else if !provider {
        let promoted = asb_core::claude_common::take_controlled(&mut values.claude_extra, SettingOwner::Client)?;
        merge_promoted(&mut values, promoted)?;
    }
    let defaults = if provider { ownership::default_provider_parameters(app) } else { ownership::default_client_settings(app) };
    for key in defaults.settings.into_keys() { values.settings.entry(key).or_insert(SettingValue::Automatic); }
    let repaired = serde_json::to_value(values).map_err(|error| error.to_string())?;
    let changed = &repaired != node;
    if changed { *node = repaired; }
    Ok(changed)
}

fn merge_promoted(values: &mut SettingsValues, promoted: BTreeMap<String, ConfigValue>) -> Result<(), String> {
    for (key, value) in promoted {
        let next = SettingValue::Explicit { value };
        if values.settings.get(&key).is_some_and(|current| *current != SettingValue::Automatic && current != &next) {
            return Err(format!("{key} 同时存在不同的可视化值和片段值；请明确保留哪一个，原数据未变"));
        }
        values.settings.insert(key, next);
    }
    Ok(())
}

pub(super) fn repair_claude_provider(node: &mut Value) -> Result<bool, String> {
    let original = node.clone();
    if let Some(options) = node.get_mut("modelOptions") {
        asb_core::contracts::upgrade_claude_model_options(options)?;
    }
    let parameters = node.get("parameters").ok_or("缺少供应商参数")?;
    let mut values: SettingsValues = serde_json::from_value(parameters.clone()).map_err(|error| error.to_string())?;
    let mut fragment = match node.get("claudeFragment") {
        Some(value) => value.as_object().cloned().ok_or("claudeFragment 必须是对象")?,
        None => Default::default(),
    };
    let promoted = asb_core::claude_common::take_controlled(&mut fragment, SettingOwner::Provider)?;
    merge_promoted(&mut values, promoted)?;
    if let Some(SettingValue::Explicit { value }) = values.settings.remove("effortLevel") {
        let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
        if fragment.get("effortLevel").is_some_and(|current| current != &value) {
            return Err("effortLevel 同时存在不同的参数值和片段值；原数据未变".into());
        }
        fragment.insert("effortLevel".into(), value);
    }
    asb_core::claude_common::validate(&fragment).map_err(|error| format!("供应商片段无法无损归属：{error}；客户端级配置需要先移到共用设置"))?;
    node["parameters"] = serde_json::to_value(values).map_err(|error| error.to_string())?;
    if fragment.is_empty() { node.as_object_mut().ok_or("供应商文件必须是对象")?.remove("claudeFragment"); }
    else { node["claudeFragment"] = Value::Object(fragment); }
    Ok(*node != original)
}
