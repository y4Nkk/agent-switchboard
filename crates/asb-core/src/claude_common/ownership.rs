use super::{pointer, Extra};
use crate::contracts::{AppKind, ConfigValue};
use crate::ownership::{setting_specs, SettingControl, SettingOwner};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn controlled(segments: &[String]) -> bool {
    // attribution=false is a current native spelling of both typed leaves.
    if segments.len() == 1 && segments[0] == "attribution" { return true; }
    setting_specs(AppKind::Claude).iter().any(|spec| spec.control != SettingControl::None
        && spec.key.split('.').eq(segments.iter().map(String::as_str)))
}

/// Current native shorthand for disabling both kinds of attribution.
pub(crate) fn normalize_attribution(root: &mut Value) {
    if root.get("attribution") == Some(&Value::Bool(false)) {
        root["attribution"] = serde_json::json!({"commit":"", "pr":""});
    }
}

/// Used only by explicit store repair: move native leaves into their one
/// current typed owner, rejecting values that cannot retain their meaning.
pub fn take_controlled(extra: &mut Extra, owner: SettingOwner) -> Result<BTreeMap<String, ConfigValue>, String> {
    let mut root = Value::Object(extra.clone());
    if owner == SettingOwner::Client { normalize_attribution(&mut root); }
    let mut values = BTreeMap::new();
    for spec in setting_specs(AppKind::Claude).iter().filter(|spec| spec.owner == owner && spec.control != SettingControl::None) {
        let segments = spec.key.split('.').map(str::to_owned).collect::<Vec<_>>();
        let Some(value) = root.pointer(&pointer::encode(&segments)).cloned() else { continue; };
        let value: ConfigValue = serde_json::from_value(value).map_err(|_| format!("{} 无法转为可视化设置", spec.key))?;
        crate::validate::validate_setting_value(spec, &value).map_err(|error| error.to_string())?;
        values.insert(spec.key.into(), value);
        super::projection::remove_pruning(&mut root, &segments)?;
    }
    *extra = root.as_object().expect("object root").clone();
    Ok(values)
}
