use super::matches_provider_identity;
use crate::contracts::{AppKind, ProviderProfile, RouteMode};

fn profile(app: AppKind, mode: RouteMode) -> ProviderProfile {
    ProviderProfile {
        id: "identity-test".into(),
        app,
        route_mode: mode,
        name: "Identity test".into(),
        model: Some("profile-model".into()),
        base_url: (mode == RouteMode::Custom).then(|| "https://example.test/v1".into()),
        api_key: if mode == RouteMode::Custom {
            "fixture-key".into()
        } else {
            String::new()
        },
        model_options: None,
        notes: None,
        website_url: None,
        usage_query: None,
    }
}

const CODEX_CUSTOM: &str = "model_provider = 'openai'\nopenai_base_url = 'https://example.test/v1'\nmodel = 'changed-model'\nhide_agent_reasoning = true\n";
const CODEX_AUTH: &str = r#"{"auth_mode":"apikey","OPENAI_API_KEY":"fixture-key"}"#;
const CLAUDE_CUSTOM: &str = r#"{"model":"changed-model","env":{"ANTHROPIC_BASE_URL":"https://example.test/v1","ANTHROPIC_AUTH_TOKEN":"fixture-key","ANTHROPIC_DEFAULT_HAIKU_MODEL":"another-model"},"permissions":{"allow":["Read"]}}"#;

#[test]
fn identity_ignores_models_and_common_settings_for_both_clients() {
    for (app, text, auth) in [
        (AppKind::Codex, CODEX_CUSTOM, Some(CODEX_AUTH)),
        (AppKind::Claude, CLAUDE_CUSTOM, None),
    ] {
        assert!(matches_provider_identity(text, auth, &profile(app, RouteMode::Custom)).unwrap());
    }
    for (app, text) in [
        (
            AppKind::Codex,
            "model = 'changed-model'\nhide_agent_reasoning = true",
        ),
        (
            AppKind::Claude,
            r#"{"model":"changed-model","env":{"ANTHROPIC_DEFAULT_HAIKU_MODEL":"another-model"}}"#,
        ),
    ] {
        assert!(matches_provider_identity(text, None, &profile(app, RouteMode::Official)).unwrap());
    }
}

#[test]
fn custom_identity_requires_exact_endpoint_and_credential() {
    for (app, text, auth) in [
        (AppKind::Codex, CODEX_CUSTOM, Some(CODEX_AUTH)),
        (AppKind::Claude, CLAUDE_CUSTOM, None),
    ] {
        let mut candidate = profile(app, RouteMode::Custom);
        candidate.api_key = "different-fixture-key".into();
        assert!(!matches_provider_identity(text, auth, &candidate).unwrap());
        candidate.api_key = "fixture-key".into();
        candidate.base_url = Some("https://other.test/v1".into());
        assert!(!matches_provider_identity(text, auth, &candidate).unwrap());
        assert!(
            !matches_provider_identity(text, auth, &profile(app, RouteMode::Official)).unwrap()
        );
    }
}

#[test]
fn codex_identity_rejects_missing_custom_auth_and_named_provider_routes() {
    let custom = profile(AppKind::Codex, RouteMode::Custom);
    for auth in [
        None,
        Some("{}"),
        Some(r#"{"auth_mode":"chatgpt","OPENAI_API_KEY":"fixture-key"}"#),
    ] {
        assert!(!matches_provider_identity(CODEX_CUSTOM, auth, &custom).unwrap());
    }
    let external = "model_provider = 'external'\n[model_providers.external]\nbase_url = 'https://example.test/v1'";
    assert!(!matches_provider_identity(external, Some(CODEX_AUTH), &custom).unwrap());
    assert!(!matches_provider_identity(
        "",
        Some(CODEX_AUTH),
        &profile(AppKind::Codex, RouteMode::Official)
    )
    .unwrap());
    assert!(matches_provider_identity(CODEX_CUSTOM, Some("invalid-json"), &custom).is_err());
}

#[test]
fn claude_official_identity_rejects_explicit_credentials() {
    let official = profile(AppKind::Claude, RouteMode::Official);
    for text in [
        r#"{"env":{"ANTHROPIC_AUTH_TOKEN":"fixture-key"}}"#,
        r#"{"env":{"ANTHROPIC_API_KEY":"fixture-key"}}"#,
    ] {
        assert!(!matches_provider_identity(text, None, &official).unwrap());
    }
    assert!(matches_provider_identity("invalid-json", None, &official).is_err());
    assert!(!matches_provider_identity(
        r#"{"env":{"ANTHROPIC_BASE_URL":"https://example.test/v1"}}"#,
        None,
        &profile(AppKind::Claude, RouteMode::Custom)
    )
    .unwrap());
}
