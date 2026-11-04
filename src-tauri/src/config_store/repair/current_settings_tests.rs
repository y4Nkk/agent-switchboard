use super::*;
use serde_json::json;

#[test]
fn codex_renames_are_explicit_and_current_values_win() {
    let mut values = serde_json::to_value(default_client_settings(AppKind::Codex)).unwrap();
    values["settings"].as_object_mut().unwrap().remove("features.view_image");
    values["settings"]["disable_paste_burst"] = json!({"mode":"explicit","value":true});
    values["settings"]["tools.view_image"] = json!({"mode":"explicit","value":false});
    values["settings"]["features.unified_exec"] = json!({"mode":"explicit","value":false});
    assert!(fill_automatic(&mut values, AppKind::Codex, false).unwrap());
    assert_eq!(values["settings"]["tui.disable_paste_burst"], json!({"mode":"automatic"}));
    assert_eq!(values["settings"]["features.view_image"], json!({"mode":"explicit","value":false}));
    for key in asb_core::ownership::RETIRED_CODEX_CLIENT_KEYS { assert!(values["settings"].get(key).is_none()); }
    let values: SettingsValues = serde_json::from_value(values).unwrap();
    values.validate_client_settings(AppKind::Codex).unwrap();
}

#[test]
fn shared_fields_move_to_the_typed_owner_with_a_complete_backup() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let path = store.client_settings_path(AppKind::Claude);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut original = serde_json::to_value(default_client_settings(AppKind::Claude)).unwrap();
    original["claudeExtra"] = json!({"permissions":{"allow":["Read"]},"attribution":false,"effortLevel":"high","env":{"KEEP":"1"}});
    let original = serde_json::to_string_pretty(&original).unwrap();
    fs::write(&path, &original).unwrap();
    assert!(store.get_client_settings(AppKind::Claude).is_err());
    let report = store.repair().unwrap();
    let saved = store.get_client_settings(AppKind::Claude).unwrap().settings;
    assert_eq!(saved.settings["permissions.allow"], SettingValue::Explicit {
        value: asb_core::ConfigValue::Array(vec![asb_core::ConfigValue::Str("Read".into())]),
    });
    assert_eq!(saved.settings["attribution.commit"], SettingValue::Explicit { value: asb_core::ConfigValue::Str("".into()) });
    assert_eq!(saved.claude_extra, json!({"effortLevel":"high","env":{"KEEP":"1"}}).as_object().unwrap().clone());
    let backup = PathBuf::from(report.backup_path.unwrap()).join("client-settings/claude.json");
    assert_eq!(fs::read_to_string(backup).unwrap(), original);
    assert!(store.repair().unwrap().repaired_files.is_empty());
}

#[test]
fn conflicting_shared_and_typed_values_do_not_change_the_store() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let path = store.client_settings_path(AppKind::Claude);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut original = serde_json::to_value(default_client_settings(AppKind::Claude)).unwrap();
    original["claudeExtra"] = json!({"permissions":{"allow":["Write"]}});
    original["settings"]["permissions.allow"] = json!({"mode":"explicit","value":["Read"]});
    let original = serde_json::to_string_pretty(&original).unwrap();
    fs::write(&path, &original).unwrap();
    assert!(store.repair().unwrap_err().contains("permissions.allow"));
    assert_eq!(fs::read_to_string(path).unwrap(), original);
}

#[test]
fn provider_repair_preserves_native_effort_and_moves_only_provider_fields() {
    let mut provider = json!({"parameters":default_provider_parameters(AppKind::Claude),
        "claudeFragment":{"promptCacheTtl":"1h","maxEffortLevel":"xhigh","modelSettings":{"m":{"autoCompactWindow":"auto"}}}});
    provider["parameters"]["settings"]["effortLevel"] = json!({"mode":"explicit","value":"high"});
    assert!(repair_claude_provider(&mut provider).unwrap());
    assert_eq!(provider["parameters"]["settings"]["promptCacheTtl"], json!({"mode":"explicit","value":"1h"}));
    assert_eq!(provider["parameters"]["settings"]["maxEffortLevel"], json!({"mode":"explicit","value":"xhigh"}));
    assert_eq!(provider["claudeFragment"]["effortLevel"], "high");
    assert_eq!(provider["claudeFragment"]["modelSettings"]["m"]["autoCompactWindow"], "auto");
    assert!(provider["parameters"]["settings"].get("effortLevel").is_none());
    assert!(!repair_claude_provider(&mut provider).unwrap());
    provider["claudeFragment"]["permissions"] = json!({"allow":["Read"]});
    assert!(repair_claude_provider(&mut provider).unwrap_err().contains("客户端级配置"));
}
