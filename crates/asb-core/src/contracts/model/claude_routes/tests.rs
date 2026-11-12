use super::*;
use serde_json::json;

#[test]
fn upgrade_preserves_each_model_flag_and_follow_primary_semantics() {
    let mut old = json!({"kind":"claude","primaryOneM":true,"haikuModel":"h","haikuOneM":false,
        "sonnetModel":"s","sonnetOneM":true,"opusModel":null,"opusOneM":false,
        "fableModel":"f","fableOneM":true,"subagentModel":"child","subagentOneM":false,
        "availableModels":["f"],"displayNames":{"haiku":"Small"}});
    assert!(upgrade_claude_model_options(&mut old).unwrap());
    let settings: crate::contracts::ModelOptions = serde_json::from_value(old.clone()).unwrap();
    let crate::contracts::ModelOptions::Claude(settings) = settings else {
        panic!("Claude")
    };
    assert_eq!(settings.model(ClaudeModelRole::Fable), Some("f"));
    assert!(settings.one_m(ClaudeModelRole::Fable));
    assert_eq!(settings.model(ClaudeModelRole::Subagent), Some("child"));
    assert!(!settings.has_provider_routes());
    assert!(settings.route(ClaudeModelRole::Opus).is_none());
    assert_eq!(
        old["routes"]["sonnet"],
        json!({"mode":"primary","model":"s","oneM":true})
    );
    assert!(!upgrade_claude_model_options(&mut old).unwrap());
}

#[test]
fn runtime_rejects_legacy_and_mixed_fields() {
    let old = json!({"kind":"claude","primaryOneM":false,"haikuModel":"h"});
    assert!(serde_json::from_value::<crate::contracts::ModelOptions>(old).is_err());
    let mut mixed = json!({"kind":"claude","primaryOneM":false,"routes":{},"haikuModel":"h"});
    let before = mixed.clone();
    assert!(upgrade_claude_model_options(&mut mixed).is_err());
    assert_eq!(mixed, before);
    assert!(serde_json::from_value::<crate::contracts::ModelOptions>(mixed).is_err());
}

#[test]
fn invalid_legacy_flag_does_not_mutate_original() {
    let mut old = json!({"kind":"claude","haikuModel":"h","haikuOneM":false,"sonnetModel":null,"sonnetOneM":true});
    let before = old.clone();
    assert!(upgrade_claude_model_options(&mut old).is_err());
    assert_eq!(old, before);
}

#[test]
fn provider_route_is_explicit_and_rejects_unknown_properties() {
    let value = json!({"mode":"provider","profileId":"01234567-89ab-cdef-0123-456789abcdef","model":"shared-name","oneM":true});
    let route: ClaudeModelRoute = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        route.profile_id(),
        Some("01234567-89ab-cdef-0123-456789abcdef")
    );
    assert_eq!(route.model(), "shared-name");
    assert!(route.one_m());
    let mut invalid = value;
    invalid["endpoint"] = json!("https://wrong.example");
    assert!(serde_json::from_value::<ClaudeModelRoute>(invalid).is_err());
}

#[test]
fn fable_inheritance_preserves_the_target_and_is_not_a_copied_override() {
    let mut settings = ClaudeModelSettings::default();
    settings.routes.opus = Some(ClaudeModelRoute::Provider {
        profile_id: "01234567-89ab-cdef-0123-456789abcdef".into(),
        model: "opus-target".into(),
        one_m: true,
    });
    assert!(settings.route(ClaudeModelRole::Fable).is_none());
    assert_eq!(
        settings.effective_route(ClaudeModelRole::Fable),
        settings.route(ClaudeModelRole::Opus)
    );
    assert_eq!(
        crate::adapter::claude::gateway_model_source(
            "asb-claude-fable",
            Some("main"),
            Some(&settings)
        )
        .unwrap(),
        Some("opus-target")
    );
}
