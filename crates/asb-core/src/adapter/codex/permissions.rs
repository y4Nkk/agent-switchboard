use crate::adapter::AdapterError;
use crate::contracts::{ConfigValue, SettingValue};
use serde::Serialize;
use toml_edit::DocumentMut;

pub const DEFAULT_PERMISSIONS_KEY: &str = "default_permissions";
pub const BUILTIN_PERMISSION_PROFILES: &[&str] = &[":read-only", ":workspace", ":danger-full-access"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionProfileStatus {
    pub selected: Option<String>,
    pub has_profiles: bool,
    pub legacy_settings: Vec<String>,
}

pub fn permission_profile_status(text: &str) -> Result<PermissionProfileStatus, AdapterError> {
    let document = super::document::parse(text)?;
    Ok(status(&document))
}

pub fn validate_permission_configuration(text: &str) -> Result<(), AdapterError> {
    validate_candidate(&super::document::parse(text)?)
}

fn status(document: &DocumentMut) -> PermissionProfileStatus {
    PermissionProfileStatus {
        selected: document.get(DEFAULT_PERMISSIONS_KEY).and_then(|item| item.as_str()).map(str::to_owned),
        has_profiles: document.contains_key("permissions") || document.contains_key(DEFAULT_PERMISSIONS_KEY),
        legacy_settings: ["sandbox_mode", "sandbox_workspace_write"].into_iter()
            .filter(|key| document.contains_key(key)).map(str::to_owned).collect(),
    }
}

pub(crate) fn validate_permission_selection(value: &SettingValue) -> Result<(), AdapterError> {
    match value {
        SettingValue::Automatic => Ok(()),
        SettingValue::Explicit { value: ConfigValue::Str(name) }
            if !name.is_empty() && name == name.trim() && !name.chars().any(char::is_control) => Ok(()),
        _ => Err(AdapterError { message: "Codex 权限档案名必须是非空文本".into(), line: None }),
    }
}

pub(super) fn validate_candidate(document: &DocumentMut) -> Result<(), AdapterError> {
    if let Some(item) = document.get(DEFAULT_PERMISSIONS_KEY) {
        let name = item.as_str().ok_or_else(|| AdapterError { message: "default_permissions 必须是档案名".into(), line: None })?;
        validate_permission_selection(&SettingValue::Explicit { value: ConfigValue::Str(name.into()) })?;
    }
    let state = status(document);
    if state.has_profiles && !state.legacy_settings.is_empty() {
        return Err(AdapterError {
            message: "Codex 权限档案不能与 sandbox_mode / sandbox_workspace_write 混写。请将旧沙箱设置恢复自动，并在原文中移除其余旧沙箱字段；权限档案已保留。".into(),
            line: None,
        });
    }
    Ok(())
}
