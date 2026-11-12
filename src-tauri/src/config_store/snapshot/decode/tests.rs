use super::*;
use asb_core::contracts::{AppKind, ClaudeModelRole, ModelOptions, ProviderFile, ProviderProfile};
use asb_core::ownership::{default_client_settings, default_provider_parameters};
use serde_json::json;
use std::collections::BTreeMap;

fn snapshot() -> Value {
    let mut client_settings = BTreeMap::new();
    let mut history = BTreeMap::new();
    for app in [AppKind::Codex, AppKind::Claude] {
        client_settings.insert(app, default_client_settings(app));
        history.insert(app, Vec::new());
    }
    serde_json::to_value(ConfigurationSnapshot {
        schema_version: CLOUD_BACKUP_SNAPSHOT_SCHEMA_VERSION,
        codex_providers: vec![],
        codex_official: vec![],
        claude_providers: vec![],
        client_settings,
        history,
    })
    .unwrap()
}

fn old_claude() -> Value {
    let profile: ProviderProfile = serde_json::from_value(json!({
        "id":"01234567-89ab-cdef-0123-456789abcdef", "app":"claude", "routeMode":"custom",
        "name":"Saved", "model":"saved-model", "baseUrl":"https://relay.example", "apiKey":"isolated-key",
        "upstreamProtocol":"anthropicMessages", "maxOutputTokens":null,
        "parameters":default_provider_parameters(AppKind::Claude), "websiteUrl":null,
    })).unwrap();
    let mut file = serde_json::to_value(ProviderFile::from_profile(&profile, 1)).unwrap();
    file["modelOptions"] = json!({"kind":"claude","primaryOneM":true,
        "haikuModel":"saved-haiku","haikuOneM":false,"sonnetModel":"saved-sonnet","sonnetOneM":true,
        "opusModel":null,"opusOneM":false,"fableModel":"saved-fable","fableOneM":true,
        "subagentModel":"saved-child","subagentOneM":true,"availableModels":["saved-model"],
        "displayNames":{"fable":"Saved Fable"}});
    file
}

fn decode(value: &Value) -> Result<ConfigurationSnapshot, String> {
    decode_cloud_backup_snapshot(&serde_json::to_vec(value).unwrap())
}

#[test]
fn previous_snapshot_preserves_models_flags_and_names() {
    let mut value = snapshot();
    value["schemaVersion"] = json!(7);
    value["claudeProviders"] = json!([old_claude()]);
    let snapshot = decode(&value).unwrap();
    assert_eq!(snapshot.schema_version, 8);
    let Some(ModelOptions::Claude(settings)) = snapshot.claude_providers[0].model_options.as_ref()
    else {
        panic!("Claude settings")
    };
    assert_eq!(settings.model(ClaudeModelRole::Fable), Some("saved-fable"));
    assert!(settings.one_m(ClaudeModelRole::Sonnet));
    assert!(settings.one_m(ClaudeModelRole::Subagent));
    assert!(settings.primary_one_m);
    assert!(!settings.has_provider_routes());
    assert_eq!(
        settings.display_names.as_ref().unwrap().fable.as_deref(),
        Some("Saved Fable")
    );
    let encoded = serde_json::to_value(snapshot).unwrap();
    assert!(encoded["claudeProviders"][0]["modelOptions"]
        .get("fableModel")
        .is_none());
    assert!(decode(&encoded).is_ok());
}

#[test]
fn current_snapshot_does_not_accept_retired_model_fields() {
    let mut value = snapshot();
    value["claudeProviders"] = json!([old_claude()]);
    assert!(decode(&value).is_err());
}

#[test]
fn conflicting_and_invalid_previous_values_are_rejected() {
    let mut value = snapshot();
    value["schemaVersion"] = json!(7);
    value["claudeProviders"] = json!([old_claude()]);
    value["claudeProviders"][0]["modelOptions"]["routes"] = json!({});
    assert!(decode(&value).is_err());
    value["claudeProviders"] = json!([old_claude()]);
    value["claudeProviders"][0]["modelOptions"]["sonnetModel"] = Value::Null;
    assert!(decode(&value).is_err());
}

#[test]
fn previous_codex_model_mapping_enters_gateway_but_current_direct_is_rejected() {
    let mut draft = crate::codex_common::test_draft();
    draft.model_routes = vec![asb_core::contracts::CodexModelRoute {
        client_model: draft.default_model.clone(),
        upstream_model: "vendor-mapped".into(),
    }];
    let file = draft.into_file("01234567-89ab-cdef-0123-456789abcdef".into(), 1);
    let mut value = snapshot();
    value["codexProviders"] = json!([file]);
    value["codexProviders"][0]["profile"]["routeMode"] = json!("direct");
    assert!(decode(&value).is_err());
    value["schemaVersion"] = json!(7);
    let upgraded = decode(&value).unwrap();
    assert_eq!(
        upgraded.codex_providers[0].profile.route_mode,
        CodexRouteMode::Gateway
    );
    assert_eq!(
        upgraded.codex_providers[0].profile.model_routes[0].upstream_model,
        "vendor-mapped"
    );
}

#[test]
fn unknown_snapshot_versions_are_rejected() {
    for version in [json!(6), json!(9), json!("7"), Value::Null] {
        let mut value = snapshot();
        value["schemaVersion"] = version;
        assert!(decode(&value).is_err());
    }
}
