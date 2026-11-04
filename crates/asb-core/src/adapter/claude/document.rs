use serde_json::{Map, Value as Json};

use crate::adapter::AdapterError;
use crate::contracts::ConfigValue;

pub(crate) fn parse(text: &str) -> Result<Json, AdapterError> {
    let value = serde_json::from_str::<Json>(text).map_err(|e| AdapterError {
        message: "JSON 格式无效".to_string(),
        line: Some(e.line()),
    })?;
    if !value.is_object() {
        return Err(AdapterError {
            message: "settings.json 根节点必须是对象".to_string(),
            line: Some(1),
        });
    }
    Ok(value)
}

fn pointer(path: &str) -> String {
    let mut ptr = String::new();
    for seg in path.split('.') {
        ptr.push('/');
        // JSON Pointer escaping per RFC 6901.
        ptr.push_str(&seg.replace('~', "~0").replace('/', "~1"));
    }
    ptr
}

pub(super) fn scalar_repr(value: &Json) -> Option<String> {
    match value {
        Json::String(s) => Some(s.clone()),
        Json::Bool(b) => Some(b.to_string()),
        Json::Number(n) => Some(n.to_string()),
        Json::Array(_) => serde_json::to_string(value).ok(),
        Json::Null => None,
        _ => None,
    }
}

pub(super) fn get<'a>(root: &'a Json, path: &str) -> Option<&'a Json> {
    root.pointer(&pointer(path))
}

fn parent_conflict(path: &str, segment: &str) -> AdapterError {
    AdapterError {
        message: format!("受管路径 {path} 的父节点 {segment} 不是 JSON 对象，无法安全修改"),
        line: None,
    }
}

fn leaf_conflict(path: &str) -> AdapterError {
    AdapterError {
        message: format!("受管键 {path} 当前是 JSON 对象，无法覆盖其宿主内容"),
        line: None,
    }
}

pub(super) fn validate_path(root: &Json, path: &str) -> Result<(), AdapterError> {
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current = root;
    let mut parent_path = String::new();
    for &seg in parents {
        let map = current
            .as_object()
            .ok_or_else(|| parent_conflict(path, &parent_path))?;
        if !parent_path.is_empty() {
            parent_path.push('.');
        }
        parent_path.push_str(seg);
        match map.get(seg) {
            Some(child) if child.is_object() => current = child,
            Some(_) => return Err(parent_conflict(path, &parent_path)),
            None => return Ok(()),
        }
    }
    let map = current
        .as_object()
        .ok_or_else(|| parent_conflict(path, &parent_path))?;
    if map.get(last).is_some_and(Json::is_object) {
        return Err(leaf_conflict(path));
    }
    Ok(())
}

pub(super) fn set(root: &mut Json, path: &str, value: ConfigValue) -> Result<(), AdapterError> {
    validate_path(root, path)?;
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current: &mut Json = root;
    let mut parent_path = String::new();
    for &seg in parents {
        let map = current
            .as_object_mut()
            .ok_or_else(|| parent_conflict(path, &parent_path))?;
        if !parent_path.is_empty() {
            parent_path.push('.');
        }
        parent_path.push_str(seg);
        current = map
            .entry(seg.to_string())
            .or_insert_with(|| Json::Object(Map::new()));
        if !current.is_object() {
            return Err(parent_conflict(path, &parent_path));
        }
    }
    current
        .as_object_mut()
        .ok_or_else(|| parent_conflict(path, &parent_path))?
        .insert(last.to_string(), to_json(value));
    Ok(())
}

pub(super) fn remove(root: &mut Json, path: &str) -> Result<(), AdapterError> {
    validate_path(root, path)?;
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current: &mut Json = root;
    for &seg in parents {
        let Some(map) = current.as_object_mut() else {
            return Err(parent_conflict(path, seg));
        };
        match map.get_mut(seg) {
            Some(child) if child.is_object() => current = child,
            Some(_) => return Err(parent_conflict(path, seg)),
            None => return Ok(()),
        }
    }
    if let Some(map) = current.as_object_mut() {
        map.remove(last);
    }
    Ok(())
}

fn to_json(value: ConfigValue) -> Json {
    match value {
        ConfigValue::Str(s) => Json::String(s),
        ConfigValue::Bool(b) => Json::Bool(b),
        ConfigValue::Number(n) => serde_json::Number::from_f64(n)
            .map(Json::Number)
            .unwrap_or(Json::Null),
        ConfigValue::Array(items) => Json::Array(items.into_iter().map(to_json).collect()),
        ConfigValue::Object(items) => Json::Object(items.into_iter().map(|(key, value)| (key, to_json(value))).collect()),
    }
}

pub(crate) fn check_syntax(text: &str) -> Result<(), AdapterError> {
    parse(text).map(|_| ())
}
