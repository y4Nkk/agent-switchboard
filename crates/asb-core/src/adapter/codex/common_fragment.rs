//! Codex 通用配置片段：任意 TOML 片段层（CC 对齐 P09）。
//!
//! 片段是用户手写的原始 TOML，与 36 个可视化开关共享同一个逐档案启用
//! 策略（`common-config-policy.json`）。投影时启用即深合并进 config.toml，
//! 停用时不做任何写入——由于 ASB 每次都从当前实时文件重放投影，停用路径
//! 只需要剥离"片段曾经写入且值未被用户改动"的键，即可还原宿主内容。
//!
//! 冲突规则（fail-closed，绝不静默覆盖）：
//! 1. 片段键与所有权目录（供应商档案键 + 客户端可视化键）相交即拒绝；
//! 2. 片段不能定义内置 `model_providers.openai`（ASB 路由标识）；
//! 3. 合并遇到"标量与表互相覆盖"的结构冲突时直接报错。
//!
//! 注释与宿主键通过 `toml_edit` 原位编辑保留：替换值只替换 Item，
//! 键的装饰（前缀注释）与位置保持不变。
use toml_edit::{DocumentMut, Item, TableLike, Value as TomlValue};

use crate::adapter::AdapterError;
use crate::ownership::{setting_specs, SettingOwner};
use crate::AppKind;

/// ASB 的 Codex 路由固定写入 `model_provider = "openai"`；片段不得覆盖该
/// 内置供应商标识，否则会劫持网关/直连路由。
const RESERVED_BUILTIN_PROVIDER_PATH: &str = "model_providers.openai";

fn parse_fragment(text: &str) -> Result<DocumentMut, AdapterError> {
    super::parse(text).map_err(|e| AdapterError {
        message: format!("通用配置片段{}", e.message.trim_start()),
        line: e.line,
    })
}

pub(super) fn declared_paths(text: &str) -> Result<std::collections::BTreeSet<Vec<String>>, AdapterError> {
    validate_fragment(text)?;
    let fragment = parse_fragment(text)?;
    let mut paths = std::collections::BTreeSet::new();
    for_each_leaf(fragment.as_table(), &[], &mut |path, _| { paths.insert(path.to_vec()); });
    Ok(paths)
}

/// 校验片段文本：解析失败、与所有权目录冲突或触碰保留路径时返回具名错误。
/// 空片段合法（表示清除）。
pub fn validate_fragment(text: &str) -> Result<(), AdapterError> {
    if text.trim().is_empty() {
        return Ok(());
    }
    let fragment = parse_fragment(text)?;
    let mut error: Option<AdapterError> = None;
    for_each_leaf(fragment.as_table(), &[], &mut |segments, item| {
        let path = crate::config_path::from_keys(segments);
        if error.is_some() {
            return;
        }
        if crate::ownership::RETIRED_CODEX_CLIENT_KEYS.iter()
            .chain(crate::ownership::RETIRED_CODEX_PROVIDER_KEYS.iter())
            .any(|key| path == *key || path.starts_with(&format!("{key}."))) {
            error = Some(AdapterError { message: format!("通用配置片段不能包含已移除的配置项 {path}，请使用当前客户端设置"), line: None });
            return;
        }
        if path == RESERVED_BUILTIN_PROVIDER_PATH
            || path.starts_with(&format!("{RESERVED_BUILTIN_PROVIDER_PATH}."))
        {
            error = Some(AdapterError {
                message: "通用配置片段不能定义内置 openai 供应商（model_providers.openai）：该路由标识由 ASB 管理".into(),
                line: None,
            });
            return;
        }
        // 叶子若是空表，它只是结构占位，合并时与受管表递归合并后互不影响；
        // 值叶子才会破坏受管子树，因此父路径冲突只对值叶子生效。
        let is_table_leaf = item.as_table_like().is_some();
        for spec in setting_specs(AppKind::Codex) {
            let managed = spec.key;
            let reason: Option<String> = if managed == path {
                Some(
                    match spec.owner {
                        SettingOwner::Provider => "该键由供应商档案管理，切换时会覆盖片段值",
                        SettingOwner::Client => "该键由客户端设置管理，切换时会覆盖片段值",
                        SettingOwner::Host => "该键由 ASB 管理",
                    }
                    .to_string(),
                )
            } else if !is_table_leaf && managed.starts_with(&format!("{path}.")) {
                Some(format!("它是受管键 {managed} 的父路径，会破坏受管结构"))
            } else if path.starts_with(&format!("{managed}.")) {
                Some(format!("它嵌套在受管键 {managed} 内部，切换时会被覆盖"))
            } else {
                None
            };
            if let Some(reason) = reason {
                error = Some(AdapterError {
                    message: format!("通用配置片段不能包含 {path}：{reason}"),
                    line: None,
                });
                return;
            }
        }
    });
    error.map_or(Ok(()), Err)
}

/// 收集片段文本中 `mcp_servers` 表声明的顶层键，供片段存储与扩展执行器
/// 两侧的争写守卫共用（E02：同一键不允许两边同时管理）。只读取直接子键；
/// 嵌套内容不构成新的键名。解析失败按原样报错（fail-closed）。
pub fn declared_mcp_server_keys(text: &str) -> Result<Vec<String>, AdapterError> {
    let mut keys = Vec::new();
    if text.trim().is_empty() {
        return Ok(keys);
    }
    let fragment = parse_fragment(text)?;
    if let Some(table) = fragment.get("mcp_servers").and_then(Item::as_table_like) {
        for (key, _) in table.iter() {
            keys.push(key.to_string());
        }
    }
    Ok(keys)
}

/// 深合并：表递归、标量与数组整体替换（与 CC 语义一致）。
/// 替换通过原位 Item 交换完成，键的注释与位置保持不变。
pub(crate) fn merge_fragment(target_text: &str, fragment_text: &str) -> Result<String, AdapterError> {
    if fragment_text.trim().is_empty() {
        return Ok(target_text.to_string());
    }
    let fragment = parse_fragment(fragment_text)?;
    let mut doc = super::parse(target_text)?;
    merge_table(doc.as_table_mut(), fragment.as_table(), "")?;
    Ok(doc.to_string())
}

fn merge_table(
    target: &mut dyn TableLike,
    source: &dyn TableLike,
    path: &str,
) -> Result<(), AdapterError> {
    for (key, source_item) in source.iter() {
        let child_path = if path.is_empty() {
            key.to_string()
        } else {
            format!("{path}.{key}")
        };
        let case = match target.get(key) {
            None => 0,
            Some(existing) => {
                match (
                    existing.as_table_like().is_some(),
                    source_item.as_table_like().is_some(),
                ) {
                    (true, true) => 1,
                    (false, false) => 2,
                    _ => {
                        return Err(AdapterError {
                            message: format!(
                                "通用配置片段 {child_path} 与现有配置结构冲突：标量与表不能互相覆盖"
                            ),
                            line: None,
                        })
                    }
                }
            }
        };
        match case {
            0 => {
                target.insert(key, source_item.clone());
            }
            1 => {
                let sub_target = target
                    .get_mut(key)
                    .and_then(Item::as_table_like_mut)
                    .expect("table case checked above");
                let sub_source = source_item
                    .as_table_like()
                    .expect("table case checked above");
                merge_table(sub_target, sub_source, &child_path)?;
            }
            _ => {
                if let Some(slot) = target.get_mut(key) {
                    *slot = source_item.clone();
                }
            }
        }
    }
    Ok(())
}

/// 停用路径：剥离片段曾经写入、且当前值仍与片段一致的键（值被用户改过的
/// 保留）。剥离后只剩片段内容的空父表一并移除（与 CC 的深移除语义一致）。
pub(crate) fn remove_applied_fragment(
    target_text: &str,
    fragment_text: &str,
) -> Result<String, AdapterError> {
    if fragment_text.trim().is_empty() {
        return Ok(target_text.to_string());
    }
    let fragment = parse_fragment(fragment_text)?;
    let mut doc = super::parse(target_text)?;
    remove_applied_table(doc.as_table_mut(), fragment.as_table());
    Ok(doc.to_string())
}

fn remove_applied_table(target: &mut dyn TableLike, source: &dyn TableLike) {
    for (key, source_item) in source.iter() {
        let case = target
            .get(key)
            .map(|existing| existing.as_table_like().is_some());
        let source_is_table = source_item.as_table_like().is_some();
        match (case, source_is_table) {
            (Some(true), true) => {
                let sub_target = target
                    .get_mut(key)
                    .and_then(Item::as_table_like_mut)
                    .expect("table case checked above");
                let sub_source = source_item
                    .as_table_like()
                    .expect("table case checked above");
                remove_applied_table(sub_target, sub_source);
            }
            (Some(_), false) | (Some(false), true) => {
                let applied = target
                    .get(key)
                    .is_some_and(|existing| item_equals(existing, source_item));
                if applied {
                    target.remove(key);
                }
            }
            (None, _) => {}
        }
    }
    // 剥离后仅剩片段结构的空父表按 CC 深移除语义一并收敛。
    for (key, source_item) in source.iter() {
        if source_item.as_table_like().is_some()
            && target
                .get(key)
                .is_some_and(|existing| existing.as_table_like().is_some_and(TableLike::is_empty))
        {
            target.remove(key);
        }
    }
}

/// 当前实时文件是否已包含片段（逐叶子的结构子集判定，CC 子集语义）。
pub fn fragment_is_applied(live_text: &str, fragment_text: &str) -> Result<bool, AdapterError> {
    if fragment_text.trim().is_empty() {
        return Ok(true);
    }
    let fragment = parse_fragment(fragment_text)?;
    let live = super::parse(live_text)?;
    Ok(table_is_subset(fragment.as_table(), live.as_table()))
}

fn table_is_subset(source: &dyn TableLike, target: &dyn TableLike) -> bool {
    source.iter().all(|(key, source_item)| {
        target.get(key).map_or(false, |target_item| {
            match (
                source_item.as_table_like(),
                target_item.as_table_like(),
            ) {
                (Some(s), Some(t)) => table_is_subset(s, t),
                (None, None) => item_equals(source_item, target_item),
                _ => false,
            }
        })
    })
}

fn item_equals(a: &Item, b: &Item) -> bool {
    match (a.as_table_like(), b.as_table_like()) {
        (Some(ta), Some(tb)) => table_equals(ta, tb),
        (None, None) => match (a.as_value(), b.as_value()) {
            (Some(va), Some(vb)) => value_equals(va, vb),
            _ => match (a.as_array_of_tables(), b.as_array_of_tables()) {
                (Some(xa), Some(xb)) => {
                    xa.len() == xb.len()
                        && xa.iter().zip(xb.iter()).all(|(ta, tb)| table_equals(ta, tb))
                }
                _ => false,
            },
        },
        _ => false,
    }
}

fn table_equals(a: &dyn TableLike, b: &dyn TableLike) -> bool {
    a.len() == b.len()
        && a.iter().all(|(key, item)| {
            b.get(key)
                .map_or(false, |other| item_equals(item, other))
        })
}

/// 结构相等：只比较解码后的值，忽略装饰（注释/空白/字符串引号风格）。
fn value_equals(a: &TomlValue, b: &TomlValue) -> bool {
    match (a, b) {
        (TomlValue::String(x), TomlValue::String(y)) => x.value() == y.value(),
        (TomlValue::Integer(x), TomlValue::Integer(y)) => x.value() == y.value(),
        (TomlValue::Float(x), TomlValue::Float(y)) => x.value() == y.value(),
        (TomlValue::Boolean(x), TomlValue::Boolean(y)) => x.value() == y.value(),
        (TomlValue::Datetime(x), TomlValue::Datetime(y)) => x == y,
        (TomlValue::Array(x), TomlValue::Array(y)) => {
            x.len() == y.len()
                && x.iter().zip(y.iter()).all(|(u, v)| value_equals(u, v))
        }
        (TomlValue::InlineTable(x), TomlValue::InlineTable(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(key, item)| y.get(key).map_or(false, |other| value_equals(item, other)))
        }
        _ => false,
    }
}

/// 遍历片段的叶子路径：非空表递归；值、数组、数组表与空表都是叶子。
fn for_each_leaf(table: &dyn TableLike, prefix: &[String], visit: &mut dyn FnMut(&[String], &Item)) {
    for (key, item) in table.iter() {
        let mut path = prefix.to_vec();
        path.push(key.to_string());
        match item.as_table_like() {
            Some(sub) if !sub.is_empty() => for_each_leaf(sub, &path, visit),
            _ => visit(&path, item),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_keys_and_reserved_paths_are_rejected_with_named_conflicts() {
        // 供应商档案键
        assert_eq!(
            validate_fragment("model = 'x'\n").unwrap_err().message,
            "通用配置片段不能包含 model：该键由供应商档案管理，切换时会覆盖片段值"
        );
        assert!(validate_fragment("openai_base_url = 'x'\n").is_err());
        assert!(validate_fragment("model_catalog_json = 'x'\n").is_err());
        // 客户端可视化键
        assert!(validate_fragment("approval_policy = 'on-request'\n").is_err());
        // 嵌套客户端键
        assert!(validate_fragment("[agents]\ndefault_subagent_model = 'x'\n").is_err());
        // 值叶子破坏受管子树
        assert!(validate_fragment("agents = 1\n").is_err());
        // 受管键内部的更深键
        assert!(validate_fragment("[agents.default_subagent_model]\nx = 1\n").is_err());
        // 保留路径
        assert_eq!(
            validate_fragment("[model_providers.openai]\nbase_url = 'x'\n")
                .unwrap_err()
                .message,
            "通用配置片段不能定义内置 openai 供应商（model_providers.openai）：该路由标识由 ASB 管理"
        );
        // 无效 TOML fail-closed
        assert!(validate_fragment("model = [unclosed\n").is_err());
    }

    #[test]
    fn host_keys_and_unknown_namespaces_are_allowed() {
        assert!(validate_fragment("[mcp_servers.custom]\ncommand = 'node'\nargs = ['a']\n").is_ok());
        assert!(validate_fragment("custom_flag = true\n").is_ok());
        assert!(validate_fragment("[model_providers.relay]\nname = 'relay'\n").is_ok());
        assert!(validate_fragment("[projects.'C:/work']\ntrust_level = 'trusted'\n").is_ok());
        // 与受管键同前缀但不同段：agents 表的兄弟键
        assert!(validate_fragment("[agents]\ncustom_subagent = 'x'\n").is_ok());
        // 空表占位与受管键前缀相容
        assert!(validate_fragment("[agents]\n").is_ok());
        assert!(validate_fragment("").is_ok());
        assert!(validate_fragment("   \n").is_ok());
    }

    #[test]
    fn merge_deep_unions_tables_and_replaces_scalars_keeping_comments() {
        let target = "# 顶部注释\n[owner]\nname = 'asb' # 行尾注释\n[mcp_servers.kept]\ncommand = 'node'\n";
        let fragment = "[owner]\nage = 3\n[mcp_servers.kept]\nenv = { KEY = 'v' }\n[mcp_servers.new]\ncommand = 'deno'\n";
        let merged = merge_fragment(target, fragment).unwrap();
        assert!(merged.contains("# 顶部注释"));
        assert!(merged.contains("name = 'asb'"));
        assert!(merged.contains("age = 3"));
        assert!(merged.contains("command = 'node'"));
        assert!(merged.contains("KEY = 'v'"));
        assert!(merged.contains("[mcp_servers.new]"));
        // 幂等：重复合并结果不变
        assert_eq!(merge_fragment(&merged, fragment).unwrap(), merged);
        // 空片段原样返回
        assert_eq!(merge_fragment(target, "  \n").unwrap(), target);
    }

    #[test]
    fn merge_scalar_table_conflicts_fail_closed() {
        let target = "[host]\nkey = 'v'\n";
        assert!(merge_fragment(target, "host = 'scalar'\n").is_err());
        let scalar_target = "host = 'v'\n";
        assert!(merge_fragment(scalar_target, "[host]\nkey = 'v'\n").is_err());
    }

    #[test]
    fn remove_strips_applied_values_keeps_user_edits_and_prunes_empties() {
        let fragment = "[mcp_servers.tools]\ncommand = 'node'\nargs = ['a', 'b']\n[shared]\nflag = true\n";
        // 完整应用过的文档：剥离后片段表消失
        let applied = "[mcp_servers.tools]\ncommand = 'node'\nargs = ['a', 'b']\n[shared]\nflag = true\n[user]\nkey = 1\n";
        let stripped = remove_applied_fragment(applied, fragment).unwrap();
        assert!(!stripped.contains("mcp_servers"));
        assert!(!stripped.contains("flag"));
        assert!(stripped.contains("[user]"));
        // 用户改过值：保留
        let edited = "[shared]\nflag = false\nother = 'kept'\n";
        assert_eq!(remove_applied_fragment(edited, fragment).unwrap(), edited);
        // 合并过的表里混有用户补充键：只剥片段键
        let mixed = "[mcp_servers.tools]\ncommand = 'node'\nuser_extra = 'mine'\n";
        let result = remove_applied_fragment(mixed, fragment).unwrap();
        assert!(result.contains("user_extra = 'mine'"));
        assert!(!result.contains("command"));
        // 从未应用：原样返回
        assert_eq!(remove_applied_fragment("[user]\nkey = 1\n", fragment).unwrap(), "[user]\nkey = 1\n");
    }

    #[test]
    fn applied_check_reflects_subset_semantics() {
        let fragment = "[mcp_servers.tools]\ncommand = 'node'\n";
        assert!(fragment_is_applied("[mcp_servers.tools]\ncommand = \"node\"\n", fragment).unwrap());
        assert!(!fragment_is_applied("[mcp_servers.tools]\ncommand = 'deno'\n", fragment).unwrap());
        assert!(!fragment_is_applied("", fragment).unwrap());
        // 表内多出宿主键不影响"已应用"判定
        assert!(fragment_is_applied(
            "[mcp_servers.tools]\ncommand = 'node'\nextra = 1\n",
            fragment
        )
        .unwrap());
    }
}
