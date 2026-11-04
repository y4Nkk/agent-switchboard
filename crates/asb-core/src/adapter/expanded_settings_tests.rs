use super::*;
use crate::{claude_common, contracts::*, ownership};
use serde_json::{json, Value};

fn explicit(value: Value) -> SettingValue {
    SettingValue::Explicit { value: serde_json::from_value(value).unwrap() }
}

#[test]
fn granular_approval_round_trips_both_native_table_forms_and_resets() {
    for original in [
        "approval_policy = { granular = { sandbox_approval = true, rules = false, mcp_elicitations = true } }\nkeep = 7\n",
        "keep = 7\n[approval_policy.granular]\nsandbox_approval = true\nrules = false\nmcp_elicitations = true\nrequest_permissions = false\nskill_approval = true\n",
    ] {
        let settings = extract_client_settings(AppKind::Codex, original).unwrap();
        let rendered = render_client_settings_into_file(AppKind::Codex, original, &settings).unwrap();
        assert_eq!(extract_client_settings(AppKind::Codex, &rendered).unwrap(), settings);
        assert!(owned_diff(AppKind::Codex, &rendered, original).unwrap().is_empty());
        let reset = reset_client_settings(AppKind::Codex, original).unwrap();
        assert!(!reset.contains("approval_policy"));
        assert!(reset.contains("keep = 7"));
        assert!(owned_diff(AppKind::Codex, &reset, original).unwrap().iter().any(|change| change.key == "approval_policy"));
    }
}

#[test]
fn malformed_granular_approval_is_not_overwritten_or_silently_accepted() {
    let defaults = ownership::default_client_settings(AppKind::Codex);
    for original in [
        "approval_policy = { granular = { rules = true } }",
        "approval_policy = { granular = { rules = true, sandbox_approval = true, mcp_elicitations = true, unknown = false } }",
        "approval_policy = { granular = { rules = 'true', sandbox_approval = true, mcp_elicitations = true } }",
    ] {
        assert!(extract_client_settings(AppKind::Codex, original).is_err());
        assert!(render_client_settings_into_file(AppKind::Codex, original, &defaults).is_err());
    }
}

#[test]
fn codex_current_keys_replace_retired_keys_without_duplicate_settings() {
    let current = "disable_paste_burst = true\n[tools]\nview_image = false\nother = true\n[features]\nunified_exec = false\n[tui]\ndisable_paste_burst = false\n";
    let mut settings = extract_client_settings(AppKind::Codex, current).unwrap();
    assert_eq!(settings.settings["tui.disable_paste_burst"], explicit(json!(false)));
    settings.settings.insert("features.view_image".into(), explicit(json!(true)));
    let rendered = render_client_settings_into_file(AppKind::Codex, current, &settings).unwrap();
    let document = rendered.parse::<toml_edit::DocumentMut>().unwrap();
    assert!(document.get("disable_paste_burst").is_none());
    assert!(document["tools"].get("view_image").is_none());
    assert_eq!(document["tools"]["other"].as_bool(), Some(true));
    assert!(document["features"].get("unified_exec").is_none());
    assert_eq!(document["features"]["view_image"].as_bool(), Some(true));
    for key in ownership::RETIRED_CODEX_CLIENT_KEYS {
        assert!(owned_diff(AppKind::Codex, &rendered, current).unwrap().iter().any(|change| change.key == *key));
    }
}

#[test]
fn native_permission_profiles_are_visible_preserved_and_cannot_be_mixed() {
    let current = "default_permissions = 'workspace'\n[permissions.workspace]\nfilesystem = { ':workspace' = 'write' }\n";
    let status = codex::permission_profile_status(current).unwrap();
    assert_eq!(status.selected.as_deref(), Some("workspace"));
    assert!(status.has_profiles && status.legacy_settings.is_empty());
    let mut settings = ownership::default_client_settings(AppKind::Codex);
    let rendered = render_client_settings_into_file(AppKind::Codex, current, &settings).unwrap();
    assert!(rendered.contains("[permissions.workspace]"));
    settings.settings.insert("sandbox_mode".into(), explicit(json!("workspace-write")));
    assert!(render_client_settings_into_file(AppKind::Codex, current, &settings).unwrap_err().message.contains("权限档案"));
    let defaults = ownership::default_client_settings(AppKind::Codex);
    let conflicting = format!("{current}\n[sandbox_workspace_write]\nwritable_roots = ['/custom']\n");
    assert!(render_client_settings_into_file(AppKind::Codex, &conflicting, &defaults).is_err());
    let pruned = remove_unmanaged_entries(AppKind::Codex, current, "").unwrap();
    let reset = reset_client_settings(AppKind::Codex, &pruned).unwrap();
    assert!(reset.contains("[permissions.workspace]"));
}

#[test]
fn claude_lists_custom_styles_and_empty_attribution_preserve_native_meaning() {
    let current = json!({"permissions":{"allow":["Read","Bash(npm run *)"],"custom":"keep"},
        "attribution":false,"outputStyle":"My teaching style","effortLevel":"high",
        "sandbox":{"network":{"allowedDomains":[],"httpProxyPort":8123}},"host":true}).to_string();
    let mut settings = extract_client_settings(AppKind::Claude, &current).unwrap();
    let parameters = read_provider_parameters(AppKind::Claude, &current).unwrap();
    assert_eq!(parameters.settings["outputStyle"], explicit(json!("My teaching style")));
    assert_eq!(settings.settings["attribution.commit"], explicit(json!("")));
    assert_eq!(settings.settings["attribution.pr"], explicit(json!("")));
    let rendered = render_client_settings_into_file(AppKind::Claude, &current, &settings).unwrap();
    let root: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(root["attribution"], json!({"commit":"", "pr":""}));
    assert_eq!(root["permissions"]["custom"], "keep");
    assert_eq!(root["sandbox"]["network"]["httpProxyPort"], 8123);
    assert_eq!(root["effortLevel"], "high");
    assert_eq!(extract_client_settings(AppKind::Claude, &rendered).unwrap(), settings);
    settings.settings.insert("attribution.commit".into(), SettingValue::Automatic);
    let changed: Value = serde_json::from_str(&render_client_settings_into_file(AppKind::Claude, &rendered, &settings).unwrap()).unwrap();
    assert!(changed["attribution"].get("commit").is_none());
    assert_eq!(changed["attribution"]["pr"], "");
}

#[test]
fn promoted_manifest_claims_never_remove_typed_fields_and_unrelated_claims_survive() {
    let current = json!({"permissions":{"allow":["Read"]},"promptCacheTtl":"1h","attribution":false,"host":true,
        "modelSettings":{"native":{"effortLevel":"high"}},
        "env":{"HOST":"original", "ASB_CLAUDE_COMMON_KEYS":"[\"/permissions/allow\",\"/attribution\",\"/env/HOST\"]",
            "ASB_CLAUDE_PROFILE_KEYS":"[\"/promptCacheTtl\",\"/modelSettings/native/effortLevel\"]"}}).to_string();
    let mut settings = extract_client_settings(AppKind::Claude, &current).unwrap();
    settings.settings.insert("permissions.allow".into(), explicit(json!(["Read", "Write"])));
    settings.claude_extra = json!({"env":{"HOST":"next"}}).as_object().unwrap().clone();
    let rendered = render_client_settings_into_file(AppKind::Claude, &current, &settings).unwrap();
    let root: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(root["permissions"]["allow"], json!(["Read","Write"]));
    assert_eq!(root["attribution"], json!({"commit":"", "pr":""}));
    assert_eq!(root["promptCacheTtl"], "1h");
    assert_eq!(root["modelSettings"]["native"]["effortLevel"], "high");
    assert_eq!(root["env"][claude_common::MANIFEST], "[\"/env/HOST\"]");
    assert_eq!(root["env"][claude_common::PROFILE_MANIFEST], "[\"/modelSettings/native/effortLevel\"]");
    assert!(claude_common::parse_extra(r#"{"permissions":{"allow":["Read"]}}"#).is_err());
}

#[test]
fn claude_typed_constraints_reject_invalid_lists_paths_and_retention() {
    for current in [json!({"permissions":{"allow":[""]}}), json!({"permissions":{"deny":[true]}}),
        json!({"permissions":false}), json!({"cleanupPeriodDays":0}), json!({"cleanupPeriodDays":1.5}),
        json!({"plansDirectory":"../plans"}), json!({"plansDirectory":"C:/plans"}), json!({"plansDirectory":"/plans"})] {
        assert!(extract_client_settings(AppKind::Claude, &current.to_string()).is_err(), "{current}");
    }
    for path in ["plans", "./plans", "docs/../plans"] {
        assert!(extract_client_settings(AppKind::Claude, &json!({"plansDirectory":path}).to_string()).is_ok());
    }
}

#[test]
fn per_model_caps_and_windows_follow_the_current_native_contract() {
    for preferences in [json!({"maxEffortLevel":"max","autoCompactWindow":"auto"}),
        json!({"effortLevel":"xhigh","maxEffortLevel":"low","autoCompactWindow":100000}),
        json!({"autoCompactWindow":1000000}), json!({"autoCompactWindow":200000.0})] {
        assert!(claude_common::parse_extra(&json!({"modelSettings":{"id/~.1":preferences}}).to_string()).is_ok());
    }
    for preferences in [json!({"autoCompactWindow":99999}), json!({"autoCompactWindow":1000001}),
        json!({"autoCompactWindow":200000.5}), json!({"autoCompactWindow":"200000"}), json!({"maxEffortLevel":"unlimited"})] {
        assert!(claude_common::parse_extra(&json!({"modelSettings":{"m":preferences}}).to_string()).is_err());
    }
}
