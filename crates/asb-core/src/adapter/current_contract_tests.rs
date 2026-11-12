use super::*;
use crate::{claude_common, contracts::*, ownership};
use serde_json::{json, Value};

fn profile() -> ProviderProfile {
    ProviderProfile {
        id: "model-preferences".into(), app: AppKind::Claude, route_mode: RouteMode::Custom,
        name: "Model preferences".into(), model: Some("vendor.model/~v1".into()),
        base_url: Some("https://provider.example".into()), connection: Default::default(),
        api_key: "isolated-test-key".into(), authentication: Some(AuthenticationScheme::Bearer),
        upstream_protocol: Some(UpstreamProtocol::AnthropicMessages), responses_options: None,
        max_output_tokens: ExplicitMaxOutputTokens::none(), model_options: None,
        parameters: ownership::default_provider_parameters(AppKind::Claude),
        claude_fragment: json!({"modelSettings":{"vendor.model/~v1":{"effortLevel":"xhigh","maxEffortLevel":"max","autoCompactWindow":200000}}})
            .as_object().unwrap().clone(),
        notes: None, website_url: None, display: None, usage_query: None,
        official_quota_refresh_interval_minutes: None,
    }
}

#[test]
fn codex_notifications_round_trip_without_false_diffs_or_list_collisions() {
    let original = "[tui]\nnotifications = ['a, b']\nalternate_screen = 'always'\nresume_cwd = 'session'\n";
    let mut settings = extract_client_settings(AppKind::Codex, original).unwrap();
    let rendered = render_client_settings_into_file(AppKind::Codex, original, &settings).unwrap();
    assert!(owned_diff(AppKind::Codex, &rendered, original).unwrap().is_empty());
    settings.settings.insert("tui.notifications".into(), SettingValue::Explicit {
        value: ConfigValue::Array(vec![ConfigValue::Str("a".into()), ConfigValue::Str("b".into())]),
    });
    let changed = render_client_settings_into_file(AppKind::Codex, original, &settings).unwrap();
    let diff = owned_diff(AppKind::Codex, &changed, original).unwrap();
    assert_eq!(diff.len(), 1);
    assert_eq!(diff[0].before.as_deref(), Some(r#"["a, b"]"#));
    assert_eq!(diff[0].after.as_deref(), Some(r#"["a","b"]"#));
    assert_eq!(extract_client_settings(AppKind::Codex, &changed).unwrap(), settings);
    assert!(extract_client_settings(AppKind::Codex, "[tui]\nnotifications = []").is_ok());
}

#[test]
fn retired_codex_types_and_approval_values_are_rejected() {
    for invalid in [
        "[tui]\nalternate_screen = true", "[tui]\nresume_cwd = false",
        "[tui]\nnotifications = ['']", "[tui]\nnotifications = [1]",
        "approval_policy = 'untrusted'",
    ] {
        assert!(extract_client_settings(AppKind::Codex, invalid).is_err(), "{invalid}");
    }
    assert!(extract_client_settings(AppKind::Codex, "[windows]\nsandbox = 'mxc'").is_ok());
}

#[test]
fn provider_readers_preserve_numbers_and_numeric_environment_strings() {
    let codex = read_provider_parameters(AppKind::Codex,
        "service_tier = 'priority'\nmodel_auto_compact_token_limit = 200000\nmodel_auto_compact_token_limit_scope = 'body_after_prefix'").unwrap();
    assert_eq!(codex.settings["model_auto_compact_token_limit"], SettingValue::Explicit { value: ConfigValue::Number(200000.0) });
    assert!(read_provider_parameters(AppKind::Codex, "model_auto_compact_token_limit = 200000.0").is_err());
    let claude = read_provider_parameters(AppKind::Claude,
        r#"{"autoCompactWindow":200000,"env":{"API_TIMEOUT_MS":"60000","CLAUDE_CODE_MAX_OUTPUT_TOKENS":"64000"}}"#).unwrap();
    assert_eq!(claude.settings["env.API_TIMEOUT_MS"], SettingValue::Explicit { value: ConfigValue::Str("60000".into()) });
    for invalid in [r#"{"autoCompactWindow":99999}"#, r#"{"autoCompactWindow":1000000.5}"#,
        r#"{"env":{"API_TIMEOUT_MS":60000}}"#, r#"{"env":{"API_TIMEOUT_MS":"-1"}}"#,
        r#"{"env":{"API_TIMEOUT_MS":"2147483648"}}"#] {
        assert!(read_provider_parameters(AppKind::Claude, invalid).is_err());
    }
}

#[test]
fn claude_effort_preserves_the_native_top_level_and_rejects_invalid_defaults() {
    for invalid in [r#"{"effortLevel":"max"}"#, r#"{"modelSettings":{"m":{"effortLevel":"max"}}}"#,
        r#"{"modelSettings":{"m":false}}"#] {
        assert!(claude_common::parse_extra(invalid).is_err());
    }
    let current = r#"{"effortLevel":"high","unmanaged":true}"#;
    let settings = ownership::default_client_settings(AppKind::Claude);
    let rendered = render_client_settings_into_file(AppKind::Claude, current, &settings).unwrap();
    let root: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(root["effortLevel"], "high");
    assert_eq!(root["unmanaged"], true);
    assert!(owned_diff(AppKind::Claude, &rendered, current).unwrap().iter().all(|change| change.key != "effortLevel"));
    let (kept, rejected) = claude_common::import_filter(json!({"effortLevel":"high"}).as_object().unwrap());
    assert!(rejected.is_empty());
    assert_eq!(kept["effortLevel"], "high");
    let reset: Value = serde_json::from_str(&reset_client_settings(AppKind::Claude, current).unwrap()).unwrap();
    assert_eq!(reset["effortLevel"], "high");
    let mut retired = ownership::default_provider_parameters(AppKind::Claude);
    retired.settings.insert("effortLevel".into(), SettingValue::Automatic);
    assert!(retired.validate_provider_parameters(AppKind::Claude).is_err());
}

#[test]
fn model_preferences_follow_gateway_roles_and_restore_the_previous_route_only() {
    let profile = profile();
    let shared = ownership::default_client_settings(AppKind::Claude);
    let native = SwitchPlan::direct(profile.clone(), shared.clone());
    let before = render(r#"{"modelSettings":{"host.model":{"effortLevel":"low"}}}"#, &native).unwrap();
    let plan = SwitchPlan::through_gateway(profile, shared, "http://127.0.0.1:47821".into(), "isolated-gateway-key".into());
    let rendered = render(&before, &plan).unwrap();
    let root: Value = serde_json::from_str(&rendered).unwrap();
    for role in ["primary", "haiku", "sonnet", "opus", "fable"] {
        assert_eq!(root["modelSettings"][format!("asb-claude-{role}")]["effortLevel"], "xhigh");
        assert_eq!(root["modelSettings"][format!("asb-claude-{role}")]["maxEffortLevel"], "max");
    }
    assert_eq!(root["modelSettings"]["asb-claude-primary"]["autoCompactWindow"], 200000);
    assert_eq!(root["modelSettings"]["host.model"]["effortLevel"], "low");
    let restored = claude::restore_gateway_overlay(&rendered, &before).unwrap();
    let restored: Value = serde_json::from_str(&restored).unwrap();
    assert_eq!(restored["modelSettings"]["vendor.model/~v1"]["effortLevel"], "xhigh");
    assert_eq!(restored["modelSettings"]["vendor.model/~v1"]["maxEffortLevel"], "max");
    assert!(restored["modelSettings"].get("asb-claude-primary").is_none());
    assert_eq!(restored["modelSettings"]["host.model"]["effortLevel"], "low");
    let automatic = SwitchPlan::direct(ProviderProfile { claude_fragment: Default::default(), ..native.profile }, native.client_settings);
    let cleared: Value = serde_json::from_str(&render(&before, &automatic).unwrap()).unwrap();
    assert!(cleared["modelSettings"].get("vendor.model/~v1").is_none());
    assert_eq!(cleared["modelSettings"]["host.model"]["effortLevel"], "low");
}

#[test]
fn gateway_projection_rejects_overlapping_literal_client_preferences() {
    let mut settings = ownership::default_client_settings(AppKind::Claude);
    settings.claude_extra = json!({"modelSettings":{"asb-claude-primary":{"effortLevel":"low"}}}).as_object().unwrap().clone();
    let plan = SwitchPlan::through_gateway(profile(), settings, "http://127.0.0.1:47821".into(), "test".into());
    assert!(render("{}", &plan).is_err());
    assert!(preview("{}", &plan, "isolated-backup").is_err());
}

#[test]
fn memory_directory_accepts_native_absolute_paths_without_guessing_relative_paths() {
    for directory in ["~/memory", "/home/user/memory", "C:\\Users\\test\\memory", "\\\\server\\share\\memory"] {
        let source = json!({"autoMemoryDirectory":directory});
        assert!(extract_client_settings(AppKind::Claude, &source.to_string()).is_ok());
    }
    for directory in ["", "memory", "./memory", "C:memory"] {
        let source = json!({"autoMemoryDirectory":directory});
        assert!(extract_client_settings(AppKind::Claude, &source.to_string()).is_err());
    }
}

#[test]
fn ownership_handoff_removes_the_old_profile_before_applying_shared_preferences() {
    let profile = profile();
    let shared = ownership::default_client_settings(AppKind::Claude);
    let before = render("{}", &SwitchPlan::direct(profile.clone(), shared.clone())).unwrap();
    let mut shared = shared;
    shared.claude_extra = profile.claude_fragment.clone();
    assert!(render_client_settings_into_file(AppKind::Claude, &before, &shared).is_err());
    let profile = ProviderProfile { claude_fragment: Default::default(), ..profile };
    let rendered = render(&before, &SwitchPlan::direct(profile, shared)).unwrap();
    let root: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(root["modelSettings"]["vendor.model/~v1"]["effortLevel"], "xhigh");
    assert!(root["env"].get(claude_common::PROFILE_MANIFEST).is_none());
}

fn assert_fixed_target_preferences(inherited: bool) {
    let mut profile = profile();
    let mut settings = ClaudeModelSettings::default();
    let destination = Some(ClaudeModelRoute::Provider {
        profile_id: "01234567-89ab-cdef-0123-456789abcdef".into(),
        model: "vendor.model/~v1".into(), one_m: true,
    });
    if inherited { settings.routes.opus = destination; } else { settings.routes.fable = destination; }
    profile.model_options = Some(ModelOptions::Claude(settings));
    profile.claude_fragment.get_mut("modelSettings").unwrap().as_object_mut().unwrap()
        .insert("asb-claude-fable".into(),json!({"effortLevel":"low"}));
    let plan = SwitchPlan::through_gateway(profile,ownership::default_client_settings(AppKind::Claude),
        "http://127.0.0.1:47821".into(),"local-token".into());
    let rendered: Value = serde_json::from_str(&render("{}",&plan).unwrap()).unwrap();
    assert_eq!(rendered["modelSettings"]["asb-claude-fable"]["effortLevel"],"low");
    assert_eq!(rendered["modelSettings"]["asb-claude-primary"]["effortLevel"],"xhigh");
    assert_eq!(rendered["env"]["ANTHROPIC_DEFAULT_FABLE_MODEL"],if inherited { "asb-claude-fable" } else { "asb-claude-fable[1m]" });
}

#[test]
fn fixed_target_preferences_do_not_inherit_same_named_source_model_preferences() {
    assert_fixed_target_preferences(false);
    assert_fixed_target_preferences(true);
}

#[test]
fn explicit_claude_target_cannot_be_rendered_as_a_direct_connection() {
    let mut profile = profile();
    let shared = ownership::default_client_settings(AppKind::Claude);
    assert!(render("{}",&SwitchPlan::direct(profile.clone(),shared.clone())).is_ok());
    let mut settings = ClaudeModelSettings::default();
    settings.routes.sonnet = Some(ClaudeModelRoute::Provider {
        profile_id: "01234567-89ab-cdef-0123-456789abcdef".into(),
        model:"target-only-model".into(),one_m:true,
    });
    profile.model_options = Some(ModelOptions::Claude(settings));
    let direct = SwitchPlan::direct(profile.clone(),shared.clone());
    assert!(render("{}",&direct).is_err());
    assert!(preview("{}",&direct,"isolated-backup").is_err());
    let gateway = SwitchPlan::through_gateway(profile,shared,
        "http://127.0.0.1:47821".into(),"local-token".into());
    let content = render("{}",&gateway).unwrap();
    let rendered:Value = serde_json::from_str(&content).unwrap();
    assert_eq!(rendered["env"]["ANTHROPIC_DEFAULT_SONNET_MODEL"],"asb-claude-sonnet[1m]");
    assert_eq!(rendered["env"]["ANTHROPIC_BASE_URL"],"http://127.0.0.1:47821");
    assert!(!content.contains("target-only-model"));
}
