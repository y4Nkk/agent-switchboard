use super::*;
use crate::{claude_common, contracts::*, ownership};
use serde_json::{json, Value};

fn profile(app: AppKind) -> ProviderProfile {
    // Codex keeps the OpenAI-style /v1 prefix; Claude's Anthropic contract
    // takes the service root (endpoint.rs).
    let base_url = match app {
        AppKind::Codex => "https://provider.example/v1",
        AppKind::Claude => "https://provider.example",
    };
    ProviderProfile {
        id: "repair-profile".into(), app, route_mode: RouteMode::Custom,
        name: "Repair profile".into(), model: Some("saved-model".into()),
        base_url: Some(base_url.into()), connection: Default::default(),
        api_key: "isolated-test-key".into(), authentication: None,
        upstream_protocol: Some(UpstreamProtocol::native_for(app)),
        responses_options: (app == AppKind::Codex).then_some(ResponsesOptions { request_mode: ResponsesRequestMode::Standard }),
        max_output_tokens: ExplicitMaxOutputTokens::none(), model_options: None,
        parameters: ownership::default_provider_parameters(app), claude_fragment: Default::default(),
        notes: None, website_url: None, display: None, usage_query: None,
        official_quota_refresh_interval_minutes: None,
    }
}

fn explicit(value: &str) -> SettingValue {
    SettingValue::Explicit { value: ConfigValue::Str(value.into()) }
}

#[test]
fn codex_repair_preserves_live_client_settings_and_shared_fragment() {
    let current = "# live comment\nmodel = 'old-model'\nsandbox_mode = 'workspace-write'\n\
        [agents]\nmax_threads = 7\n[mcp_servers.tools]\ncommand = 'live-tool'\n";
    let mut settings = ownership::default_client_settings(AppKind::Codex);
    settings.settings.insert("sandbox_mode".into(), explicit("read-only"));
    let full = SwitchPlan::direct(profile(AppKind::Codex), settings)
        .with_codex_common_fragment(CodexCommonFragment {
            text: "[mcp_servers.tools]\ncommand = 'stored-tool'\n".into(), enabled: true,
        });
    let repair = full.clone().for_provider_repair();
    let rendered = render(current, &repair).unwrap();
    let document: toml_edit::DocumentMut = rendered.parse().unwrap();
    assert_eq!(document["model"].as_str(), Some("saved-model"));
    assert_eq!(document["sandbox_mode"].as_str(), Some("workspace-write"));
    assert_eq!(document["agents"]["max_threads"].as_integer(), Some(7));
    assert_eq!(document["mcp_servers"]["tools"]["command"].as_str(), Some("live-tool"));
    assert!(rendered.contains("# live comment"));
    let changes = preview(current, &repair, "backup").unwrap().changes;
    assert!(changes.iter().any(|change| change.key == "model"));
    assert!(changes.iter().all(|change| change.key != "sandbox_mode"
        && !change.key.starts_with("mcp_servers.") && change.key != "agents.max_threads"));
    assert!(preview(&rendered, &repair, "backup").unwrap().changes.is_empty());
    let full_rendered = render(current, &full).unwrap();
    let full_document: toml_edit::DocumentMut = full_rendered.parse().unwrap();
    assert_eq!(full_document["sandbox_mode"].as_str(), Some("read-only"));
    assert_eq!(full_document["agents"]["max_threads"].as_integer(), Some(7));
    assert_eq!(full_document["mcp_servers"]["tools"]["command"].as_str(), Some("stored-tool"));
    let full_changes = preview(current, &full, "backup").unwrap().changes;
    assert!(full_changes.iter().any(|change| change.key == "sandbox_mode"));
    assert!(full_changes.iter().all(|change| change.key != "agents.max_threads"));
}

#[test]
fn codex_gateway_repair_keeps_its_route_and_catalog_inside_provider_scope() {
    let mut saved = profile(AppKind::Codex);
    saved.upstream_protocol = Some(UpstreamProtocol::ChatCompletions);
    saved.responses_options = None;
    let endpoint = format!("http://127.0.0.1:47821/codex/asb_codex_{}/v1", "a".repeat(64));
    let plan = SwitchPlan::through_gateway(saved, ownership::default_client_settings(AppKind::Codex),
        endpoint.clone(), "isolated-gateway-key".into())
        .with_codex_model_catalog("agent-switchboard-codex-repair.json".into())
        .for_provider_repair();
    let current = "model = 'old-model'\nsandbox_mode = 'workspace-write'\n";
    let rendered = render(current, &plan).unwrap();
    let document: toml_edit::DocumentMut = rendered.parse().unwrap();
    assert_eq!(route_state(AppKind::Codex, &rendered).base_url.as_deref(), Some(endpoint.as_str()));
    assert_eq!(document["model_catalog_json"].as_str(), Some("agent-switchboard-codex-repair.json"));
    assert_eq!(document["sandbox_mode"].as_str(), Some("workspace-write"));
    assert!(preview(&rendered, &plan, "").unwrap().changes.is_empty());
}

#[test]
fn claude_repair_replaces_only_profile_fragment_and_preserves_live_public_values() {
    let shared = json!({"env":{"PUBLIC_FLAG":"live"},"modelSettings":{"public.model":{"effortLevel":"low"}}});
    let old_profile = json!({"modelSettings":{"old.model":{"effortLevel":"high"}}});
    let current = claude_common::apply(r#"{"model":"old-model","language":"Japanese","unmanaged":true}"#,
        shared.as_object().unwrap()).unwrap();
    let current = claude_common::apply_profile(&current, old_profile.as_object().unwrap()).unwrap();
    let mut saved = profile(AppKind::Claude);
    saved.claude_fragment = json!({"modelSettings":{"saved-model":{"effortLevel":"xhigh"}}})
        .as_object().unwrap().clone();
    let mut settings = ownership::default_client_settings(AppKind::Claude);
    settings.settings.insert("language".into(), explicit("English"));
    settings.claude_extra = json!({"env":{"PUBLIC_FLAG":"stored"},"modelSettings":{"public.model":{"effortLevel":"high"}}})
        .as_object().unwrap().clone();
    let plan = SwitchPlan::direct(saved, settings).for_provider_repair();
    let rendered = render(&current, &plan).unwrap();
    let root: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(root["model"], "saved-model");
    assert_eq!(root["language"], "Japanese");
    assert_eq!(root["env"]["PUBLIC_FLAG"], "live");
    assert_eq!(root["modelSettings"]["public.model"]["effortLevel"], "low");
    assert_eq!(root["modelSettings"]["saved-model"]["effortLevel"], "xhigh");
    assert!(root["modelSettings"].get("old.model").is_none());
    assert_eq!(root["unmanaged"], true);
    let before: Value = serde_json::from_str(&current).unwrap();
    assert_eq!(root["env"][claude_common::MANIFEST], before["env"][claude_common::MANIFEST]);
    assert!(preview(&current, &plan, "").unwrap().changes.iter()
        .all(|change| change.key != "language" && !change.key.contains("public.model") && !change.key.contains("PUBLIC_FLAG")));
    assert!(preview(&rendered, &plan, "").unwrap().changes.is_empty());
}

#[test]
fn claude_repair_rejects_live_shared_ownership_even_when_the_leaf_is_missing() {
    let shared = json!({"modelSettings":{"saved-model":{"effortLevel":"low"}}});
    let current = claude_common::apply("{}", shared.as_object().unwrap()).unwrap();
    let mut saved = profile(AppKind::Claude);
    saved.claude_fragment = json!({"modelSettings":{"saved-model":{"effortLevel":"high"}}})
        .as_object().unwrap().clone();
    let plan = SwitchPlan::direct(saved, ownership::default_client_settings(AppKind::Claude)).for_provider_repair();
    let mut missing: Value = serde_json::from_str(&current).unwrap();
    missing.as_object_mut().unwrap().remove("modelSettings");
    for text in [current, missing.to_string()] {
        assert!(render(&text, &plan).is_err());
        assert!(preview(&text, &plan, "").is_err());
    }
}

#[test]
fn overlapping_live_manifests_cannot_delete_a_shared_leaf_during_profile_cleanup() {
    let fragment = json!({"env":{"PUBLIC_FLAG":"live"}});
    let current = claude_common::apply("{}", fragment.as_object().unwrap()).unwrap();
    let mut root: Value = serde_json::from_str(&current).unwrap();
    root["env"][claude_common::PROFILE_MANIFEST] = root["env"][claude_common::MANIFEST].clone();
    assert!(claude_common::apply_profile(&root.to_string(), &Default::default()).is_err());
    assert!(claude_common::validate_shared_write(&root.to_string(), &Default::default()).is_err());
}
