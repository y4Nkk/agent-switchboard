use toml_edit::{DocumentMut, Item, TableLike, Value as TomlValue};

use crate::adapter::AdapterError;
use crate::contracts::{ConfigValue, MAX_EXACT_CONFIG_INTEGER};

pub(crate) fn parse(text: &str) -> Result<DocumentMut, AdapterError> {
    text.parse::<DocumentMut>()
        .map_err(|e: toml_edit::TomlError| {
            let line = e
                .span()
                .map(|span| text[..span.start].matches('\n').count() + 1);
            AdapterError {
                message: "TOML 格式无效".to_string(),
                line,
            }
        })
}

/// Textual repr of a scalar TOML value for diff comparison/display.
pub(super) fn scalar_repr(value: &TomlValue) -> Option<String> {
    match value {
        TomlValue::String(s) => Some(s.value().clone()),
        // Decoded values only: Display carries the decor (surrounding
        // whitespace), which would break exact value comparisons.
        TomlValue::Integer(i) => Some(i.value().to_string()),
        TomlValue::Float(f) => Some(f.value().to_string()),
        TomlValue::Boolean(b) => Some(b.value().to_string()),
        TomlValue::Array(_) => super::common::toml_value(value, "array").ok().map(|value| value.display()),
        _ => None,
    }
}

pub(super) fn item_repr(item: &Item) -> Option<String> {
    item.as_value().and_then(scalar_repr)
}

pub(super) fn item_at<'a>(doc: &'a DocumentMut, path: &str) -> Option<&'a Item> {
    let mut item = doc.as_item();
    for seg in path.split('.') {
        item = item.as_table_like()?.get(seg)?;
    }
    Some(item)
}

fn parent_conflict(path: &str, segment: &str) -> AdapterError {
    AdapterError {
        message: format!("受管路径 {path} 的父节点 {segment} 不是 TOML 表，无法安全修改"),
        line: None,
    }
}

fn leaf_conflict(path: &str) -> AdapterError {
    AdapterError {
        message: format!("受管键 {path} 当前是 TOML 表，无法覆盖其宿主内容"),
        line: None,
    }
}

pub(super) fn validate_path(doc: &DocumentMut, path: &str) -> Result<(), AdapterError> {
    if path == "approval_policy" {
        if let Some(item) = doc.get(path).filter(|item| item.as_table_like().is_some()) {
            let value = super::common::item_value(item, path)?;
            if !crate::validate::approval_policy::valid(&value) {
                return Err(AdapterError { message: "approval_policy 不是有效的当前审批策略；原值已保留".into(), line: None });
            }
        }
        return Ok(());
    }
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current = doc.as_item();
    let mut parent_path = String::new();
    for &seg in parents {
        let table = current
            .as_table_like()
            .ok_or_else(|| parent_conflict(path, &parent_path))?;
        if !parent_path.is_empty() {
            parent_path.push('.');
        }
        parent_path.push_str(seg);
        match table.get(seg) {
            Some(item) if item.as_table_like().is_some() => current = item,
            Some(_) => return Err(parent_conflict(path, &parent_path)),
            None => return Ok(()),
        }
    }
    let table = current
        .as_table_like()
        .ok_or_else(|| parent_conflict(path, &parent_path))?;
    if table
        .get(last)
        .is_some_and(|item| item.as_table_like().is_some())
    {
        return Err(leaf_conflict(path));
    }
    Ok(())
}

pub(super) fn set_path(
    doc: &mut DocumentMut,
    path: &str,
    value: ConfigValue,
) -> Result<(), AdapterError> {
    validate_path(doc, path)?;
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current = doc.as_item_mut();
    let mut parent_path = String::new();
    for &seg in parents {
        let table = current
            .as_table_like_mut()
            .ok_or_else(|| parent_conflict(path, &parent_path))?;
        if !parent_path.is_empty() {
            parent_path.push('.');
        }
        parent_path.push_str(seg);
        if !table.contains_key(seg) {
            table.insert(seg, Item::Table(toml_edit::Table::new()));
        }
        let next = table
            .get_mut(seg)
            .expect("inserted or pre-existing path segment");
        if next.as_table_like().is_none() {
            return Err(parent_conflict(path, &parent_path));
        }
        current = next;
    }
    let table = current
        .as_table_like_mut()
        .ok_or_else(|| parent_conflict(path, &parent_path))?;
    match table.get_mut(last) {
        // An existing scalar is updated in place: replacing the whole item
        // would reset the key's decor and drop live-file comments attached
        // to it (e.g. the leading `# live comment`).
        Some(item) => {
            if path == "approval_policy" && item.as_table_like().is_some() {
                *item = Item::Value(to_toml_value(value));
                return Ok(());
            }
            let value_slot = item
                .as_value_mut()
                .ok_or_else(|| leaf_conflict(path))?;
            *value_slot = to_toml_value(value);
        }
        None => {
            table.insert(last, Item::Value(to_toml_value(value)));
        }
    }
    Ok(())
}

fn to_toml_value(value: ConfigValue) -> TomlValue {
    match value {
        ConfigValue::Str(s) => TomlValue::from(s),
        ConfigValue::Bool(b) => TomlValue::from(b),
        ConfigValue::Number(n) => {
            if n.fract() == 0.0 && n.abs() <= MAX_EXACT_CONFIG_INTEGER as f64 {
                TomlValue::from(n as i64)
            } else {
                TomlValue::from(n)
            }
        }
        ConfigValue::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(to_toml_value(item));
            }
            TomlValue::Array(array)
        }
        ConfigValue::Object(items) => {
            let mut table = toml_edit::InlineTable::new();
            for (key, value) in items { table.insert(&key, to_toml_value(value)); }
            TomlValue::InlineTable(table)
        }
    }
}

pub(crate) fn check_syntax(text: &str) -> Result<(), AdapterError> {
    parse(text).map(|_| ())
}

pub(super) fn remove_path(doc: &mut DocumentMut, path: &str) -> Result<(), AdapterError> {
    validate_path(doc, path)?;
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current = doc.as_item_mut();
    for &seg in parents {
        let Some(table) = current.as_table_like_mut() else {
            return Err(parent_conflict(path, seg));
        };
        current = match table.get_mut(seg) {
            Some(item) => item,
            None => return Ok(()),
        };
    }
    if let Some(table) = current.as_table_like_mut() {
        table.remove(last);
    }
    Ok(())
}

pub(super) fn remove_empty_table_path(
    doc: &mut DocumentMut,
    path: &str,
) -> Result<(), AdapterError> {
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current = doc.as_item_mut();
    for &seg in parents {
        let Some(table) = current.as_table_like_mut() else {
            return Ok(());
        };
        let Some(next) = table.get_mut(seg) else {
            return Ok(());
        };
        current = next;
    }
    if let Some(table) = current.as_table_like_mut() {
        let is_empty = table
            .get(last)
            .and_then(Item::as_table_like)
            .is_some_and(TableLike::is_empty);
        if is_empty {
            table.remove(last);
        }
    }
    Ok(())
}
