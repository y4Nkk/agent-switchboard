//! Importing a source-wide shared snippet into the local visual/extra split.
use super::{import::env_text, import_filter, leaves, Extra};
use crate::contracts::{AppKind, ConfigValue, SettingValue, SettingsValues};
use crate::ownership::{setting_specs, SettingControl, SettingOwner};
use serde_json::Value;
use std::collections::BTreeMap;

/// One parsed shared snippet: visual client preferences move into the
/// application's typed settings, everything legal beyond them becomes extra,
/// and rejected keys (credentials, provider-owned routing, extension
/// families) are named instead of silently dropped or stored redacted.
pub struct SharedSnippet {
    pub visual: BTreeMap<String, ConfigValue>,
    pub extra: Extra,
    pub rejected: Vec<String>,
}

/// Parses one shared snippet. Provider-owned env keys such as credentials
/// never enter either store: the ownership directory is the redaction rule.
pub fn import_shared(text: &str) -> Result<SharedSnippet, String> {
    let mut root: Value =
        serde_json::from_str(text).map_err(|_| "Claude 通用配置片段不是有效 JSON")?;
    if !root.is_object() {
        return Err("Claude 通用配置片段根节点必须是对象".into());
    }
    coerce_env_scalars(&mut root);
    super::normalize_attribution(&mut root);
    let mut visual = BTreeMap::new();
    let mut rejected = Vec::new();
    for spec in setting_specs(AppKind::Claude) {
        if spec.owner != SettingOwner::Client || spec.control == SettingControl::None {
            continue;
        }
        let segments = spec.key.split('.').map(str::to_string).collect::<Vec<_>>();
        let path = super::pointer::encode(&segments);
        let Some(value) = root.pointer(&path).cloned() else {
            continue;
        };
        match serde_json::from_value::<ConfigValue>(value) {
            Ok(value) => {
                visual.insert(spec.key.to_string(), value);
            }
            Err(_) => rejected.push(spec.key.to_string()),
        }
        super::pointer::remove(&mut root, &segments)?;
    }
    let (extra, mut dropped) = import_filter(root.as_object().expect("object root"));
    rejected.append(&mut dropped);
    Ok(SharedSnippet {
        visual,
        extra,
        rejected,
    })
}

/// Env values are strings in rendered settings; sources may carry booleans
/// or numbers, so they are normalized before any ownership decision.
fn coerce_env_scalars(root: &mut Value) {
    let Some(env) = root.get_mut("env").and_then(Value::as_object_mut) else {
        return;
    };
    let names = env.keys().cloned().collect::<Vec<_>>();
    for name in names {
        if let Some(coerced) = env.get(&name).and_then(env_text) {
            env.insert(name, coerced);
        }
    }
}

/// Deep-merges an incoming extra into the current one; incoming leaves win.
/// Returns the merged store and how many leaf paths actually changed value
/// or were added.
pub fn merged_extra(current: &Extra, incoming: &Extra) -> (Extra, usize) {
    let mut root = Value::Object(current.clone());
    let mut changed = 0;
    let leaves = super::leaves(&Value::Object(incoming.clone()))
        .and_then(|leaves| {
            leaves
                .iter()
                .map(|(path, value)| {
                    super::pointer::decode(path)
                        .map(|segments| (path.clone(), segments, value.clone()))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .expect("imported extra is validated before merging");
    for (path, segments, value) in leaves {
        if root.pointer(&path) == Some(&value) {
            continue;
        }
        changed += 1;
        super::pointer::set(&mut root, &segments, value).expect("object parents are validated");
    }
    (root.as_object().expect("object root").clone(), changed)
}

/// Human-facing leaf paths of one extra, for previews and import summaries.
pub fn extra_leaf_paths(extra: &Extra) -> Vec<String> {
    leaves(&Value::Object(extra.clone()))
        .map(|leaves| leaves.keys().cloned().collect())
        .unwrap_or_default()
}

/// Applies the visual keys from one imported snippet onto a complete client
/// settings set. Returns how many keys changed value and how many already
/// matched. Validation stays with the store's save path.
pub fn apply_visual(
    settings: &mut SettingsValues,
    visual: &BTreeMap<String, ConfigValue>,
) -> (usize, usize) {
    let mut changed = 0;
    let mut unchanged = 0;
    for (key, value) in visual {
        let next = SettingValue::Explicit {
            value: value.clone(),
        };
        if settings.settings.get(key) == Some(&next) {
            unchanged += 1;
        } else {
            changed += 1;
        }
        settings.settings.insert(key.clone(), next);
    }
    (changed, unchanged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_visual_keys_extras_and_named_rejections() {
        let snippet = r#"{
            "spinnerTipsEnabled": false,
            "env": {
                "HTTP_PROXY": "http://127.0.0.1:1",
                "ANTHROPIC_AUTH_TOKEN": "source-secret",
                "DISABLE_TELEMETRY": 1,
                "ASB_CLAUDE_COMMON_KEYS": "[]"
            },
            "permissions": {"allow": ["Read"]},
            "mcpServers": {"x": {"command": "keep-out"}},
            "includeCoAuthoredBy": false
        }"#;
        let parsed = import_shared(snippet).unwrap();
        assert_eq!(
            parsed.visual.get("spinnerTipsEnabled"),
            Some(&ConfigValue::Bool(false))
        );
        assert_eq!(
            parsed.extra["env"]["HTTP_PROXY"],
            serde_json::json!("http://127.0.0.1:1")
        );
        assert_eq!(
            parsed.extra["env"]["DISABLE_TELEMETRY"],
            serde_json::json!("1")
        );
        assert_eq!(
            parsed.visual.get("permissions.allow"),
            Some(&ConfigValue::Array(vec![ConfigValue::Str("Read".into())]))
        );
        assert_eq!(
            parsed.extra["includeCoAuthoredBy"],
            serde_json::json!(false)
        );
        let mut rejected = parsed.rejected.clone();
        rejected.sort();
        assert_eq!(
            rejected,
            vec![
                "env.ANTHROPIC_AUTH_TOKEN".to_string(),
                "env.ASB_CLAUDE_COMMON_KEYS".to_string(),
                "mcpServers".to_string(),
            ]
        );
    }

    #[test]
    fn merged_extra_overwrites_incoming_leaves_and_counts_only_real_changes() {
        let current = serde_json::json!({
            "statusLine": {"command": "old"},
            "env": {"KEEP": "1"}
        })
        .as_object()
        .unwrap()
        .clone();
        let incoming = serde_json::json!({
            "statusLine": {"command": "new"},
            "env": {"KEEP": "1", "ADDED": "x"}
        })
        .as_object()
        .unwrap()
        .clone();
        let (merged, changed) = merged_extra(&current, &incoming);
        assert_eq!(changed, 2);
        assert_eq!(merged["statusLine"]["command"], serde_json::json!("new"));
        assert_eq!(merged["env"]["KEEP"], serde_json::json!("1"));
        assert_eq!(merged["env"]["ADDED"], serde_json::json!("x"));
    }
}
