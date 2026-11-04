use super::*;
use asb_core::{contracts::{ConfigValue, SettingValue}, ownership};
use serde_json::json;

#[test]
fn advanced_reset_matches_manifest_claims_by_decoded_key_segments() {
    let kind = ClientConfigurationResetKind::NativeDefaultsWithUnmanaged;
    let codex = "\"custom.value\" = 'keep'\n[custom]\nvalue = 'remove'\n";
    let reset = kind.settings(AppKind::Codex, ownership::default_client_settings(AppKind::Codex)).unwrap();
    let rendered = render_reset_configuration(AppKind::Codex, codex, kind, &reset, "\"custom.value\" = 'saved'\n").unwrap();
    let changes = asb_core::adapter::full_diff(AppKind::Codex, &rendered, codex).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].key, "custom.value");
    assert_eq!(changes[0].before.as_deref(), Some("remove"));

    let claude = json!({"custom.value":"keep", "custom":{"value":"remove"},
        "env":{"ASB_CLAUDE_COMMON_KEYS":"[\"/custom.value\"]"}});
    let reset = kind.settings(AppKind::Claude, ownership::default_client_settings(AppKind::Claude)).unwrap();
    let rendered = render_reset_configuration(AppKind::Claude, &claude.to_string(), kind, &reset, "").unwrap();
    let changes = asb_core::adapter::full_diff(AppKind::Claude, &rendered, &claude.to_string()).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].key, "custom.value");
    assert_eq!(changes[0].before.as_deref(), Some("remove"));
}

#[test]
fn reset_preview_distinguishes_literal_keys_from_nested_agent_settings() {
    let current = "\"agents.enabled\" = true\n[agents]\nenabled = true\n";
    for kind in [ClientConfigurationResetKind::NativeDefaults, ClientConfigurationResetKind::NativeDefaultsWithUnmanaged] {
        let reset = kind.settings(AppKind::Codex, ownership::default_client_settings(AppKind::Codex)).unwrap();
        let rendered = render_reset_configuration(AppKind::Codex, current, kind, &reset, "").unwrap();
        let changes = asb_core::adapter::full_diff(AppKind::Codex, &rendered, current).unwrap();
        assert!(changes.iter().any(|change| change.key == "agents.enabled" && change.after.is_none()));
        let literal = changes.iter().any(|change| change.key == "\"agents.enabled\"" && change.after.is_none());
        assert_eq!(literal, matches!(kind, ClientConfigurationResetKind::NativeDefaultsWithUnmanaged));
    }
}

#[test]
fn advanced_reset_preserves_directory_owned_structures_while_removing_standard_settings() {
    let current = r#"unknown = true
[agents]
enabled = true
[agents.reviewer]
description = "keep"
config_file = "reviewer.toml"
[permissions.work]
network_access = false
[shell_environment_policy]
include_only = ["PATH"]
[projects."F:/project.with.dots"]
trust_level = "trusted"
[tui.keymap]
quit = "ctrl-q"
"#;
    let kind = ClientConfigurationResetKind::NativeDefaultsWithUnmanaged;
    let reset = kind.settings(AppKind::Codex, ownership::default_client_settings(AppKind::Codex)).unwrap();
    let rendered = render_reset_configuration(AppKind::Codex, current, kind, &reset, "").unwrap();
    let changes = asb_core::adapter::full_diff(AppKind::Codex, &rendered, current).unwrap();
    assert_eq!(changes.iter().map(|change| change.key.as_str()).collect::<Vec<_>>(), vec!["agents.enabled", "unknown"]);

    let current = json!({"permissions":{"deny":["Bash(rm *)"],"custom":"keep"},"env":{"USER_FLAG":"keep"},
        "modelSettings":{"model.with.dots":{"effortLevel":"high"}},"voice":{"enabled":true},
        "spinnerTipsEnabled":true,"unknown":true});
    let reset = kind.settings(AppKind::Claude, ownership::default_client_settings(AppKind::Claude)).unwrap();
    let rendered = render_reset_configuration(AppKind::Claude, &current.to_string(), kind, &reset, "").unwrap();
    let mut expected = current;
    expected["permissions"].as_object_mut().unwrap().remove("deny");
    expected.as_object_mut().unwrap().remove("spinnerTipsEnabled");
    expected.as_object_mut().unwrap().remove("unknown");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&rendered).unwrap(), expected);
}

#[test]
fn advanced_reset_redacts_credentials_in_deleted_urls() {
    let kind = ClientConfigurationResetKind::NativeDefaultsWithUnmanaged;
    for (app, current) in [
        (AppKind::Codex, "proxy_url = 'http://user:password@proxy.example'"),
        (AppKind::Claude, r#"{"proxy_url":"http://user:password@proxy.example"}"#),
    ] {
        let reset = kind.settings(app, ownership::default_client_settings(app)).unwrap();
        let rendered = render_reset_configuration(app, current, kind, &reset, "").unwrap();
        let changes = asb_core::adapter::full_diff(app, &rendered, current).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].before.as_deref(), Some(asb_core::redact::REDACTED));
        assert!(changes[0].after.is_none());
    }
}

#[test]
fn native_reset_does_not_replay_stale_or_unapplied_claude_extra() {
    for kind in [ClientConfigurationResetKind::NativeDefaults, ClientConfigurationResetKind::NativeDefaultsWithUnmanaged] {
        let mut saved = ownership::default_client_settings(AppKind::Claude);
        saved.claude_extra.insert("custom".into(), json!({ "value": "saved" }));
        saved.claude_extra.insert("unapplied".into(), json!(true));
        let current = json!({
            "spinnerTipsEnabled": true,
            "custom": { "value": "external" },
            "env": { "ASB_CLAUDE_COMMON_KEYS": "[\"/custom/value\"]" },
            "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "echo done" }] }] }
        });
        let reset = kind.settings(AppKind::Claude, saved.clone()).unwrap();
        let rendered = render_reset_configuration(AppKind::Claude, &current.to_string(), kind, &reset, "").unwrap();
        let actual: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        let mut expected = current;
        expected.as_object_mut().unwrap().remove("spinnerTipsEnabled");
        assert_eq!(actual, expected);
        assert_eq!(reset.claude_extra, saved.claude_extra);
    }
}

#[test]
fn native_codex_reset_previews_every_runtime_removal_and_preserves_roles() {
    let current = "model = 'gpt-5'\n[agents]\nenabled = true\nmax_concurrent_threads_per_session = 6\ninterrupt_message = true\n[agents.reviewer]\ndescription = 'keep'\n";
    let kind = ClientConfigurationResetKind::NativeDefaults;
    let reset = kind.settings(AppKind::Codex, ownership::default_client_settings(AppKind::Codex)).unwrap();
    let rendered = render_reset_configuration(AppKind::Codex, current, kind, &reset, "").unwrap();
    let changes = asb_core::adapter::owned_diff(AppKind::Codex, &rendered, current).unwrap();
    for key in asb_core::contracts::CodexSubagentKey::ALL {
        assert!(changes.iter().any(|change| change.key == key.path()
            && matches!(change.kind, asb_core::contracts::ChangeKind::Remove)));
    }
    assert_eq!(changes.len(), 3);
    assert!(rendered.contains("gpt-5"));
    assert!(rendered.contains("[agents.reviewer]"));
}

#[test]
fn deep_codex_reset_keeps_extension_families_and_live_common_fragment_values() {
    let current = r#"model = "gpt-5"
plugins = { plugin = { enabled = true } }
unknown = false
[apps]
[hooks]
[[hooks.Stop]]
command = "echo done"
[mcp_servers.one]
command = "server"
[[skills.config]]
path = "/skills/one"
[custom]
value = "external"
unmanaged = true
"#;
    let kind = ClientConfigurationResetKind::NativeDefaultsWithUnmanaged;
    let reset = kind.settings(AppKind::Codex, ownership::default_client_settings(AppKind::Codex)).unwrap();
    let rendered = render_reset_configuration(AppKind::Codex, current, kind, &reset, "[custom]\nvalue = 'saved'\n").unwrap();
    let actual: toml_edit::DocumentMut = rendered.parse().unwrap();
    let mut expected: toml_edit::DocumentMut = current.parse().unwrap();
    expected.remove("unknown");
    expected["custom"].as_table_mut().unwrap().remove("unmanaged");
    assert_eq!(actual.to_string(), expected.to_string());
}

#[test]
fn native_defaults_preserve_claude_extra_configuration() {
    let mut saved = ownership::default_client_settings(AppKind::Claude);
    saved.settings.insert(
        "spinnerTipsEnabled".into(),
        SettingValue::Explicit { value: ConfigValue::Bool(true) },
    );
    saved.claude_extra.insert("permissions".into(), json!({ "allow": ["git status"] }));

    let reset = ClientConfigurationResetKind::NativeDefaults
        .settings(AppKind::Claude, saved.clone())
        .expect("native reset");

    assert!(reset.settings.values().all(|value| matches!(value, SettingValue::Automatic)));
    assert_eq!(reset.claude_extra, saved.claude_extra);
}

#[test]
fn clearing_extra_configuration_preserves_claude_standard_settings() {
    let mut saved = ownership::default_client_settings(AppKind::Claude);
    saved.settings.insert(
        "spinnerTipsEnabled".into(),
        SettingValue::Explicit { value: ConfigValue::Bool(false) },
    );
    saved.claude_extra.insert("permissions".into(), json!({ "allow": ["git status"] }));

    let reset = ClientConfigurationResetKind::ClearExtraConfiguration
        .settings(AppKind::Claude, saved.clone())
        .expect("clear extra configuration");

    assert_eq!(reset.settings, saved.settings);
    assert!(reset.claude_extra.is_empty());
}

#[test]
fn clearing_extra_configuration_does_not_reproject_standard_settings() {
    let mut saved = ownership::default_client_settings(AppKind::Claude);
    saved.settings.insert(
        "spinnerTipsEnabled".into(),
        SettingValue::Explicit { value: ConfigValue::Bool(false) },
    );
    saved.claude_extra.insert("custom".into(), json!({ "value": "remove" }));
    let reset = ClientConfigurationResetKind::ClearExtraConfiguration
        .settings(AppKind::Claude, saved)
        .expect("clear extra configuration");
    let current = r#"{
  "spinnerTipsEnabled": true,
  "custom": { "value": "remove" },
  "env": { "ASB_CLAUDE_COMMON_KEYS": "[\"/custom/value\"]" }
}"#;

    let rendered = render_reset_configuration(
        AppKind::Claude,
        current,
        ClientConfigurationResetKind::ClearExtraConfiguration,
        &reset,
        "",
    )
    .expect("render extra clear");

    assert!(rendered.contains("\"spinnerTipsEnabled\": true"));
    assert!(!rendered.contains("\"custom\""));
    assert!(!rendered.contains("ASB_CLAUDE_COMMON_KEYS"));
}
#[test]
fn clearing_extra_configuration_rejects_codex() {
    let error = ClientConfigurationResetKind::ClearExtraConfiguration
        .settings(AppKind::Codex, ownership::default_client_settings(AppKind::Codex))
        .expect_err("Codex has no Claude extra configuration");

    assert_eq!(error.code, "client-configuration-rejected");
}

#[test]
fn deep_reset_matches_native_settings_intent() {
    let mut saved = ownership::default_client_settings(AppKind::Claude);
    saved.claude_extra.insert("custom".into(), json!({ "enabled": true }));

    let reset = ClientConfigurationResetKind::NativeDefaultsWithUnmanaged
        .settings(AppKind::Claude, saved.clone())
        .expect("deep reset");

    assert!(reset.settings.values().all(|value| matches!(value, SettingValue::Automatic)));
    assert_eq!(reset.claude_extra, saved.claude_extra);
}

#[test]
fn deep_reset_removes_unmanaged_fields_but_keeps_provider_and_extra() {
    let mut saved = ownership::default_client_settings(AppKind::Claude);
    saved.claude_extra.insert("custom".into(), json!({ "enabled": true }));
    let reset = ClientConfigurationResetKind::NativeDefaultsWithUnmanaged
        .settings(AppKind::Claude, saved)
        .expect("deep reset");
    let current = r#"{
  "model": "claude-x",
  "spinnerTipsEnabled": true,
  "statusLine": { "type": "command" },
  "custom": { "enabled": true },
  "env": {
"ANTHROPIC_BASE_URL": "http://127.0.0.1:9",
"ASB_CLAUDE_COMMON_KEYS": "[\"/custom/enabled\"]",
"MY_TOOL_TOKEN": "v"
  }
}"#;

    let rendered = render_reset_configuration(
        AppKind::Claude,
        current,
        ClientConfigurationResetKind::NativeDefaultsWithUnmanaged,
        &reset,
        "",
    )
    .expect("render deep reset");

    assert!(rendered.contains("\"model\": \"claude-x\""));
    assert!(rendered.contains("ANTHROPIC_BASE_URL"));
    assert!(rendered.contains("custom"));
    assert!(rendered.contains("ASB_CLAUDE_COMMON_KEYS"));
    assert!(!rendered.contains("statusLine"));
    assert!(rendered.contains("MY_TOOL_TOKEN"));
    assert!(!rendered.contains("spinnerTipsEnabled"));
}

#[test]
fn deep_reset_keeps_codex_provider_families_and_drops_host_tables() {
    let reset = ClientConfigurationResetKind::NativeDefaultsWithUnmanaged
        .settings(AppKind::Codex, ownership::default_client_settings(AppKind::Codex))
        .expect("deep reset");
    let current = "model = \"gpt-5\"\n\n[agents]\nenabled = true\n\n[tools]\nflag = true\n\n[model_providers.custom]\nbase_url = \"https://x\"\n";

    let rendered = render_reset_configuration(
        AppKind::Codex,
        current,
        ClientConfigurationResetKind::NativeDefaultsWithUnmanaged,
        &reset,
        "",
    )
    .expect("render deep reset");

    assert!(rendered.contains("model = \"gpt-5\""));
    assert!(rendered.contains("model_providers.custom"));
    assert!(!rendered.contains("[agents]"));
    assert!(!rendered.contains("[tools]"));
}
