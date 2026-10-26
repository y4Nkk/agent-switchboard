use super::*;
use asb_core::contracts::{
    CodexCapabilities, CodexCatalogEntry, CodexEndpoint, CodexModelRoute, CodexUpstream,
};

fn draft() -> CodexProviderDraft {
    CodexProviderDraft {
        name: "relay".to_string(),
        endpoint: CodexEndpoint("https://relay.example/v1".to_string()),
        api_key: "fixture-key".to_string(),
        authentication: None,
        connection: Default::default(),
        upstream: CodexUpstream::Responses,
        request_mode: asb_core::contracts::ResponsesRequestMode::Standard,
        default_model: "codex".to_string(),
        catalog: vec![CodexCatalogEntry {
            id: "codex".to_string(),
            context_window: 128_000,
            max_output_tokens: 16_384,
            function_tools: true,
            custom_tools: true,
            tool_search: true,
            reasoning: true,
            default_reasoning_level: asb_core::contracts::CodexReasoningLevel::High,
            supported_reasoning_levels: vec![
                asb_core::contracts::CodexReasoningLevel::None,
                asb_core::contracts::CodexReasoningLevel::High,
            ],
            images: true,
            compact: true,
            display_name: None,
            description: None,
            base_instructions: None,
            supports_parallel_tool_calls: None,
        }],
        model_routes: vec![CodexModelRoute {
            client_model: "codex".to_string(),
            upstream_model: "vendor-codex".to_string(),
        }],
        subagent_route: None,
        capabilities: CodexCapabilities {
            responses: true,
            compact: true,
            models: true,
            chat_completions: true,
            alpha_search: false,
            image_generation: false,
            image_edit: false,
            function_tools: true,
            custom_tools: true,
            tool_search: true,
            reasoning: true,
            chat_reasoning: asb_core::contracts::CodexChatReasoning::Unsupported,
        },
        parameters: asb_core::ownership::default_provider_parameters(asb_core::AppKind::Codex),
        notes: None,
        website_url: None,
        usage_query: None,
    }
}

#[test]
fn active_catalog_change_requires_a_projection_preview() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = crate::local_state::LocalState::from_root(directory.path().join("state"));
    let gateway = crate::gateway::GatewayController::start(&state);
    let record = state
        .configuration()
        .create_codex_provider(draft())
        .unwrap();
    let target = state.target(asb_core::AppKind::Codex).unwrap();
    std::fs::write(&target, "model_provider = 'openai'\n").unwrap();
    std::fs::write(
        target.with_file_name("auth.json"),
        r#"{"auth_mode":"chatgpt","tokens":{"access_token":"at","refresh_token":"rt","id_token":"id"}}"#,
    )
    .unwrap();
    let projection = super::build_codex_plan(
        &state,
        &gateway,
        state
            .configuration()
            .find_codex_provider_file(&record.profile.id)
            .unwrap(),
    )
    .unwrap();
    let preview = super::preview_projection(&state, &projection).unwrap();
    super::execute_projection(
        &state,
        &gateway,
        &projection,
        &preview.content_hash,
        &preview.rendered_hash,
    )
    .unwrap();

    let mut changed = draft();
    changed.catalog[0].context_window = 256_000;
    let (kind, preview, _, _) = prepare_data(
        &state,
        &gateway,
        &record.profile.id,
        &changed,
        &record.file_hash,
        ProfileSaveIntent::Save,
    )
    .unwrap();
    assert_eq!(kind, ProfileSaveKind::SaveAndApply);
    assert!(preview.is_some());
    gateway.shutdown();
}

#[test]
fn a_subagent_route_reference_must_resolve_at_save_time() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = crate::local_state::LocalState::from_root(directory.path().join("state"));
    let record = state
        .configuration()
        .create_codex_provider(draft())
        .unwrap();

    let resolved = asb_core::contracts::CodexSubagentRoute {
        profile_id: record.profile.id.clone(),
        model: "codex".to_string(),
    };
    assert!(super::validate_subagent_route_reference(&state, &resolved).is_ok());

    let missing = asb_core::contracts::CodexSubagentRoute {
        profile_id: "01234567-89ab-cdef-0123-456789abcdef".to_string(),
        model: "codex".to_string(),
    };
    let error = super::validate_subagent_route_reference(&state, &missing).unwrap_err();
    assert_eq!(error.code, "codex-subagent-route-unresolved");
    assert!(error.message.contains("不存在"), "{error:?}");

    let unknown_model = asb_core::contracts::CodexSubagentRoute {
        profile_id: record.profile.id.clone(),
        model: "no-such-model".to_string(),
    };
    let error = super::validate_subagent_route_reference(&state, &unknown_model).unwrap_err();
    assert_eq!(error.code, "codex-subagent-route-unresolved");
    assert!(error.message.contains("目录"), "{error:?}");
}
