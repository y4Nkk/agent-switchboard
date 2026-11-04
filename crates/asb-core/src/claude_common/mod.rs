//! Additional Claude shared configuration. Visual preferences retain their own keys.
mod import;
mod pointer;
mod projection;
mod shared;
mod restore;
pub use restore::restore_profile_models;
mod validation;
mod ownership;
pub use ownership::take_controlled;
pub(crate) use ownership::normalize_attribution;
pub(crate) use projection::reconcile_manifests;
pub mod model_preferences;
pub use import::import_fragment;
pub use projection::{
    apply, apply_profile, diff_documents, import_filter,
    owned_key_paths, owned_paths, validate_scopes, validate_shared_write,
};
use serde_json::{Map, Value};
pub use shared::{apply_visual, extra_leaf_paths, import_shared, merged_extra, SharedSnippet};
use std::collections::BTreeMap;
pub use validation::validate;
pub const MANIFEST: &str = "ASB_CLAUDE_COMMON_KEYS";
/// Ownership manifest for the active provider profile's fragment. It travels
/// through the same declared-path mechanism as the shared extra so a profile
/// switch removes exactly the fragment it installed.
pub const PROFILE_MANIFEST: &str = "ASB_CLAUDE_PROFILE_KEYS";
pub type Extra = Map<String, Value>;

/// Parses Claude's ASB-managed extra configuration without accepting any
/// visual client setting, provider setting, credential, or extension field.
pub fn parse_extra(text: &str) -> Result<Extra, String> {
    let extra: Value =
        serde_json::from_str(text).map_err(|_| "Claude 额外通用配置不是有效 JSON")?;
    if !extra.is_object() {
        return Err("Claude 额外通用配置根节点必须是对象".into());
    }
    let mut canonical = Value::Object(Map::new());
    for (path, value) in leaves(&extra)? {
        pointer::set(&mut canonical, &pointer::decode(&path)?, value)?;
    }
    let extra = canonical.as_object().expect("object").clone();
    validate(&extra)?;
    Ok(extra)
}

fn leaves(value: &Value) -> Result<BTreeMap<String, Value>, String> {
    let mut output = BTreeMap::new();
    collect(value, &mut Vec::new(), &mut output)?;
    Ok(output)
}
fn collect(
    value: &Value,
    path: &mut Vec<String>,
    output: &mut BTreeMap<String, Value>,
) -> Result<(), String> {
    if path.len() > 32 {
        return Err("Claude 通用配置嵌套层级超过 32".into());
    }
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            path.push(key.clone());
            collect(value, path, output)?;
            path.pop();
        }
    } else {
        output.insert(pointer::encode(path), value.clone());
        if output.len() > 512 {
            return Err("Claude 通用配置最多包含 512 个额外字段".into());
        }
    }
    Ok(())
}
fn declared(root: &Value, manifest: &str) -> Result<Vec<String>, String> {
    let Some(value) = root.pointer(&format!("/env/{manifest}")) else {
        return Ok(Vec::new());
    };
    let text = value
        .as_str()
        .ok_or("Claude 通用配置所有权标记无效，原文件已保留")?;
    let paths: Vec<String> = serde_json::from_str(text)
        .map_err(|_| "Claude 通用配置所有权标记无法解析，原文件已保留")?;
    if paths.len() > 512 {
        return Err("Claude 通用配置所有权标记超过限制".into());
    }
    let mut current = Vec::new();
    for path in paths {
        let segments = pointer::decode(&path)?;
        // The typed catalog owns these leaves now. A manifest is never a
        // second owner; projection will rewrite its remaining claims.
        if ownership::controlled(&segments) { continue; }
        validation::path(&segments)?;
        current.push(path);
    }
    Ok(current)
}
