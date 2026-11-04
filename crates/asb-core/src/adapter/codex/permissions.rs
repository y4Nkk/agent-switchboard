use crate::adapter::AdapterError;
use serde::Serialize;
use toml_edit::DocumentMut;

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

fn status(document: &DocumentMut) -> PermissionProfileStatus {
    PermissionProfileStatus {
        selected: document.get("default_permissions").and_then(|item| item.as_str()).map(str::to_owned),
        has_profiles: document.contains_key("permissions") || document.contains_key("default_permissions"),
        legacy_settings: ["sandbox_mode", "sandbox_workspace_write"].into_iter()
            .filter(|key| document.contains_key(key)).map(str::to_owned).collect(),
    }
}

pub(super) fn validate_candidate(document: &DocumentMut) -> Result<(), AdapterError> {
    let state = status(document);
    if state.has_profiles && !state.legacy_settings.is_empty() {
        return Err(AdapterError {
            message: "Codex 权限档案不能与 sandbox_mode / sandbox_workspace_write 混写。请将旧沙箱设置恢复自动，并在原文中移除其余旧沙箱字段；权限档案已保留。".into(),
            line: None,
        });
    }
    Ok(())
}
