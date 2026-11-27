//! Selection of unmanaged fields for a Claude reset.

use std::collections::BTreeSet;

use serde_json::{Map, Value as Json};

use crate::adapter::AdapterError;
use crate::contracts::AppKind;
use crate::ownership::is_owned;

pub(crate) fn select_unmanaged(
    text: &str, selected: &BTreeSet<String>, candidates: &mut BTreeSet<String>,
) -> Result<String, AdapterError> {
    let mut root = super::document::parse(text)?;
    let native = super::native::owned_paths(&root)?;
    let common = crate::claude_common::owned_key_paths(&root)
        .map_err(|message| AdapterError { message, line: None })?;
    if !prune_object(root.as_object_mut().expect("validated configuration object"), &[], &native, &common, selected, candidates) {
        return Ok(text.to_string());
    }
    serde_json::to_string_pretty(&root).map_err(|_| AdapterError {
        message: "Claude 配置无法编码".to_string(),
        line: None,
    })
}

fn preserved(path: &[String], is_table: bool, native: &BTreeSet<String>, common: &BTreeSet<Vec<String>>) -> bool {
    let key = crate::config_path::from_keys(path);
    is_owned(AppKind::Claude, &key)
        || crate::ownership::is_claude_extension_path(&key)
        || crate::ownership::is_claude_credential_path(&key)
        || crate::ownership::preserve_only_path(AppKind::Claude, path, is_table)
        || native.contains(&key)
        || common.iter().any(|claimed| path.starts_with(claimed) || (!is_table && claimed.starts_with(path)))
}

fn prune_object(
    map: &mut Map<String, Json>,
    prefix: &[String],
    native: &BTreeSet<String>,
    common: &BTreeSet<Vec<String>>,
    selected: &BTreeSet<String>,
    candidates: &mut BTreeSet<String>,
) -> bool {
    let mut changed = false;
    let keys: Vec<String> = map.keys().cloned().collect();
    for key in keys {
        let mut path = prefix.to_vec();
        path.push(key.clone());
        let is_table = map.get(&key).is_some_and(Json::is_object);
        if preserved(&path, is_table, native, common) { continue; }
        let remove = if let Some(child) = map.get_mut(&key).and_then(Json::as_object_mut).filter(|child| !child.is_empty()) {
            let removed = prune_object(child, &path, native, common, selected, candidates);
            changed |= removed;
            removed && child.is_empty()
        } else {
            let id = crate::config_path::from_keys(&path);
            let remove = selected.contains(&id);
            candidates.insert(id);
            remove
        };
        if remove {
            map.remove(&key);
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::claude::state;
    fn remove_unmanaged(text: &str) -> Result<String, AdapterError> {
        crate::adapter::remove_unmanaged_entries(AppKind::Claude, text, "")
    }

    #[test]
    fn host_keys_are_removed_and_empty_objects_pruned() {
        let text = r#"{
  "spinnerTipsEnabled": true,
  "statusLine": { "type": "command" },
  "custom": { "enabled": true },
  "env": {
    "ASB_CLAUDE_COMMON_KEYS": "[\"/custom/enabled\"]",
    "MY_TOOL_TOKEN": "secret-value",
    "UNLISTED_VAR": "y"
  }
}"#;
        let rendered = remove_unmanaged(text).expect("prune");
        assert!(rendered.contains("spinnerTipsEnabled"));
        assert!(rendered.contains("custom"));
        assert!(rendered.contains("ASB_CLAUDE_COMMON_KEYS"));
        assert!(rendered.contains("MY_TOOL_TOKEN"));
        assert!(rendered.contains("UNLISTED_VAR"));
        assert!(!rendered.contains("statusLine"));
    }

    #[test]
    fn claimed_extra_paths_survive_as_subtrees() {
        let text = r#"{
  "custom": { "enabled": true, "extra": 1 },
  "env": { "ASB_CLAUDE_COMMON_KEYS": "[\"/custom/enabled\"]" }
}"#;
        let rendered = remove_unmanaged(text).expect("prune");
        assert!(rendered.contains("\"enabled\": true"));
        assert!(!rendered.contains("\"extra\""));
    }

    #[test]
    fn no_removals_return_text_unchanged() {
        let text = "{\n  \"model\": \"claude-x\"\n}\n";
        assert_eq!(remove_unmanaged(text).expect("prune"), text);
    }

    #[test]
    fn empty_object_passes_through() {
        assert_eq!(remove_unmanaged("{}").expect("prune"), "{}");
    }

    #[test]
    fn extension_and_credential_resources_survive_deep_reset() {
        let original = serde_json::json!({
            "enabledPlugins": {"plugin@market": true},
            "extraKnownMarketplaces": {"market": {"source": {"source": "github", "repo": "a/b"}}},
            "enabledMcpjsonServers": ["one"],
            "disabledMcpjsonServers": ["two"],
            "enableAllProjectMcpServers": false,
            "mcpServers": {"one": {"command": "server"}},
            "skillOverrides": {"one": {"enabled": false}},
            "apiKeyHelper": "read-credential",
            "otelHeadersHelper": "read-headers"
        });
        let mut current = original.clone();
        current["unmanaged"] = serde_json::json!(true);
        let pruned: serde_json::Value = serde_json::from_str(
            &remove_unmanaged(&current.to_string()).expect("prune")
        ).expect("document");
        assert_eq!(pruned, original);
    }

    #[test]
    fn full_diff_lists_host_removals_that_owned_diff_hides() {
        let live = r#"{"spinnerTipsEnabled": true, "statusLine": {"type": "command"}}"#;
        let candidate = r#"{"spinnerTipsEnabled": true}"#;
        assert!(state::owned_diff(candidate, live).expect("owned diff").is_empty());
        let changes = state::full_diff(candidate, live).expect("full diff");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].key, "statusLine.type");
    }
}
