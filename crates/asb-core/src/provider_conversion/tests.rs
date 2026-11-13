use super::*;
use crate::contracts::*;

fn codex(upstream: CodexUpstream) -> CodexProviderRecord {
    let draft = CodexProviderDraft {
        name: "independent-provider".into(), endpoint: CodexEndpoint("https://fixture.example/api".into()),
        api_key: "isolated-fixture-key".into(), authentication: None, connection: Default::default(),
        upstream, request_mode: ResponsesRequestMode::Standard, default_model: "alias".into(),
        catalog: vec![CodexCatalogEntry::default_entry("alias"), CodexCatalogEntry::default_entry("other")],
        model_routes: vec![CodexModelRoute { client_model: "alias".into(), upstream_model: "vendor-a".into() },
            CodexModelRoute { client_model: "other".into(), upstream_model: "vendor-b".into() }],
        subagent_route: None, capabilities: DEFAULT_CODEX_CAPABILITIES,
        parameters: crate::ownership::default_provider_parameters(AppKind::Codex),
        notes: Some("local note".into()), website_url: Some("https://fixture.example".into()), usage_query: None,
    };
    let file = draft.into_file("11111111-1111-4111-8111-111111111111".into(), 100);
    file.validate().unwrap();
    file.record("codex-revision".into())
}

fn claude(upstream: CodexUpstream) -> ProviderRecord {
    let converted = from_codex(&codex(upstream)).unwrap();
    let Some(ProviderConversionSeed::Claude { draft }) = converted.seed else { panic!("Claude seed") };
    ProviderRecord { profile: ProviderProfile::from_draft("22222222-2222-4222-8222-222222222222".into(), draft),
        position: 100, file_hash: "claude-revision".into() }
}

#[test]
fn supported_protocols_keep_connection_authentication_and_real_model_ids() {
    for upstream in [CodexUpstream::Responses, CodexUpstream::ChatCompletions, CodexUpstream::AnthropicMessages] {
        let source = codex(upstream);
        let converted = from_codex(&source).unwrap();
        assert_eq!(converted.status, ConversionStatus::Ready);
        let Some(ProviderConversionSeed::Claude { draft }) = converted.seed else { panic!("Claude seed") };
        assert_eq!(draft.model.as_deref(), Some("vendor-a"));
        assert_eq!(draft.authentication, Some(upstream.protocol().authentication_scheme()));
        assert_eq!(draft.base_url.as_deref(), Some(source.profile.endpoint.0.as_str()));
        assert_eq!(draft.upstream_protocol, Some(upstream.protocol()));
        assert!(draft.api_key == source.profile.api_key);
        draft.validate().unwrap();
        let reverse = from_provider(&claude(upstream)).unwrap();
        assert_eq!(reverse.status, ConversionStatus::NeedsInput);
        let Some(ProviderConversionSeed::Codex { mut draft }) = reverse.seed else { panic!("Codex seed") };
        assert!(draft.clone().into_draft().is_err(), "unknown limits cannot become defaults");
        assert!(!draft.capabilities.reasoning && !draft.capabilities.compact && !draft.capabilities.function_tools);
        for model in &mut draft.catalog { model.context_window = Some(32768); model.max_output_tokens = Some(2048); }
        draft.into_draft().unwrap().into_file("new-target".into(), 200).validate().unwrap();
    }
}

#[test]
fn aliases_are_resolved_once_and_deduplicated() {
    let mut source = codex(CodexUpstream::Responses);
    source.profile.model_routes[1].upstream_model = "vendor-a".into();
    let converted = from_codex(&source).unwrap();
    let Some(ProviderConversionSeed::Claude { draft }) = converted.seed else { panic!("Claude seed") };
    let Some(ModelOptions::Claude(options)) = draft.model_options else { panic!("Claude models") };
    assert_eq!(options.available_models, Some(vec!["vendor-a".to_string()]));
    assert!(ClaudeModelRole::ALL.into_iter().all(|role| options.route(role).is_none()));
}

#[test]
fn foreign_provider_roles_are_not_sent_through_the_primary_credential() {
    let mut source = claude(CodexUpstream::Responses);
    let Some(ModelOptions::Claude(settings)) = &mut source.profile.model_options else { panic!("Claude models") };
    settings.routes.opus = Some(ClaudeModelRoute::Provider {
        profile_id: "33333333-3333-4333-8333-333333333333".into(), model: "foreign-only".into(), one_m: false,
    });
    settings.routes.haiku = Some(ClaudeModelRoute::Primary { model: "local-only".into(), one_m: false });
    let converted = from_provider(&source).unwrap();
    assert!(converted.notices.iter().any(|notice| notice.key.ends_with("dependencies")));
    let Some(ProviderConversionSeed::Codex { draft }) = converted.seed else { panic!("Codex seed") };
    assert!(draft.catalog.iter().any(|model| model.id == "local-only"));
    assert!(!draft.catalog.iter().any(|model| model.id == "foreign-only"));
    assert!(draft.subagent_route.is_none());
}

#[test]
fn usage_query_is_optional_and_manual_only_without_modifying_the_source() {
    let mut source = codex(CodexUpstream::Responses);
    source.usage_query = Some(UsageQuery::Script { source: "fixture".into(), refresh_interval_minutes: 15 });
    let converted = from_codex(&source).unwrap();
    assert_eq!(converted.usage_query.as_ref().unwrap().refresh_interval_minutes(), 0);
    assert_eq!(source.usage_query.as_ref().unwrap().refresh_interval_minutes(), 15);
    let Some(ProviderConversionSeed::Claude { draft }) = converted.seed else { panic!("Claude seed") };
    assert!(draft.usage_query.is_none());
}

#[test]
fn full_urls_and_overrides_keep_their_semantics_and_drop_usage_timestamps() {
    let mut source = codex(CodexUpstream::Responses);
    source.profile.connection.is_full_url = true;
    source.profile.endpoint.0 = "https://fixture.example/custom/responses".into();
    source.profile.connection.models_url = Some("https://fixture.example/custom/models".into());
    source.profile.connection.custom_endpoints.insert("https://mirror.example/responses".into(), ProviderEndpoint {
        url: "https://mirror.example/responses".into(), added_at: 5, last_used: Some(7),
    });
    source.profile.route_mode = CodexRouteMode::Gateway;
    let converted = from_codex(&source).unwrap();
    let Some(ProviderConversionSeed::Claude { draft }) = converted.seed else { panic!("Claude seed") };
    assert!(draft.connection.is_full_url);
    assert_eq!(draft.connection.models_url, source.profile.connection.models_url);
    assert!(draft.connection.custom_endpoints.values().all(|endpoint| endpoint.last_used.is_none()));
    let actual = crate::endpoint::upstream_endpoint_with_options(draft.base_url.as_ref().unwrap(),
        draft.upstream_protocol.unwrap(), draft.connection.is_full_url).unwrap();
    assert_eq!(actual, source.profile.endpoint.0);
}

#[test]
fn official_and_managed_sources_never_expose_a_key_seed() {
    let source = codex(CodexUpstream::Responses);
    let mut official = claude(CodexUpstream::Responses);
    official.profile = ProviderProfile::from_draft("official".into(),
        codex_official_draft(crate::ownership::default_provider_parameters(AppKind::Codex)));
    assert_eq!(from_provider(&official).unwrap().status, ConversionStatus::Blocked);
    let mut managed = source;
    managed.profile.connection.provider_type = Some("xai_oauth".into());
    managed.profile.api_key = XAI_OAUTH_PLACEHOLDER.into();
    managed.profile.route_mode = CodexRouteMode::Gateway;
    let result = from_codex(&managed).unwrap();
    assert_eq!(result.status, ConversionStatus::Blocked);
    assert!(result.seed.is_none() && result.usage_query.is_none());
}

#[test]
fn backend_messages_do_not_embed_credentials() {
    let source = codex(CodexUpstream::Responses);
    let converted = from_codex(&source).unwrap();
    let rendered = serde_json::to_string(&converted.notices).unwrap();
    assert!(!rendered.contains(&source.profile.api_key));
}

#[test]
fn anthropic_codex_wire_marker_becomes_claude_typed_context_flag() {
    let mut source = codex(CodexUpstream::AnthropicMessages);
    source.profile.model_routes[0].upstream_model = "vendor-a[1m]".into();
    let converted = from_codex(&source).unwrap();
    let Some(ProviderConversionSeed::Claude { draft }) = converted.seed else { panic!("Claude seed") };
    assert_eq!(draft.model.as_deref(), Some("vendor-a"));
    let Some(ModelOptions::Claude(settings)) = draft.model_options.as_ref() else { panic!("Claude models") };
    assert!(settings.primary_one_m);
    assert!(settings.available_models.as_ref().unwrap().iter().all(|model| !model.contains("[1m]")));
    draft.validate().unwrap();
}

#[test]
fn claude_aliases_resolve_only_with_proven_local_model_mappings() {
    let mut source = claude(CodexUpstream::AnthropicMessages);
    source.profile.model = Some("sonnet".into());
    let Some(ModelOptions::Claude(settings)) = &mut source.profile.model_options else { panic!("Claude models") };
    settings.available_models = Some(vec!["sonnet".into(), "haiku".into(), "asb-claude-opus".into(), "claude-sonnet-actual".into()]);
    settings.routes.sonnet = Some(ClaudeModelRoute::Primary { model: "vendor-sonnet".into(), one_m: false });
    settings.routes.opus = Some(ClaudeModelRoute::Provider {
        profile_id: "33333333-3333-4333-8333-333333333333".into(), model: "foreign-only".into(), one_m: false,
    });
    let converted = from_provider(&source).unwrap();
    let Some(ProviderConversionSeed::Codex { draft }) = converted.seed else { panic!("Codex seed") };
    assert_eq!(draft.default_model, "vendor-sonnet");
    assert_eq!(draft.catalog.iter().map(|model| model.id.as_str()).collect::<Vec<_>>(),
        vec!["vendor-sonnet", "claude-sonnet-actual"]);
}

#[test]
fn unresolved_claude_main_alias_stays_an_explicit_selection() {
    let mut source = claude(CodexUpstream::Responses);
    source.profile.model = Some("sonnet".into());
    let converted = from_provider(&source).unwrap();
    let Some(ProviderConversionSeed::Codex { draft }) = converted.seed else { panic!("Codex seed") };
    assert!(draft.default_model.is_empty());
    assert!(!draft.catalog.iter().any(|entry| entry.id == "sonnet"));
}

#[test]
fn claude_cloud_sdk_managed_and_gemini_connections_are_explicitly_blocked() {
    let mut native = claude(CodexUpstream::AnthropicMessages);
    native.profile.api_key.clear();
    native.profile.authentication = None;
    native.profile.connection.claude_native = Some(crate::claude_native::ClaudeNative {
        kind: crate::claude_native::ClaudeNativeKind::Bedrock, environment: Default::default(),
    });
    let mut managed = claude(CodexUpstream::Responses);
    managed.profile.api_key.clear();
    managed.profile.connection.provider_type = Some("codex_oauth".into());
    let mut gemini = claude(CodexUpstream::AnthropicMessages);
    gemini.profile.upstream_protocol = Some(UpstreamProtocol::GeminiGenerateContent);
    gemini.profile.authentication = Some(AuthenticationScheme::XGoogApiKey);
    gemini.profile.base_url = Some("https://generativelanguage.googleapis.com/v1beta".into());
    for (source, key) in [(native, "blocked.native"), (managed, "blocked.managed"), (gemini, "blocked.protocol")] {
        let result = from_provider(&source).unwrap();
        assert_eq!(result.status, ConversionStatus::Blocked);
        assert!(result.seed.is_none() && result.usage_query.is_none());
        assert!(result.blocked_reason.as_ref().unwrap().key.ends_with(key));
    }
}
