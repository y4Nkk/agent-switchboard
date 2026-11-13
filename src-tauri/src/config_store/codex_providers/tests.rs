use super::*;
use asb_core::contracts::{
    CodexCapabilities, CodexCatalogEntry, CodexEndpoint, CodexModelRoute, CodexUpstream,
    ProviderEndpoint,
};

fn draft() -> CodexProviderDraft {
    CodexProviderDraft {
        name: "Relay".to_string(),
        endpoint: CodexEndpoint("https://relay.example/v1".to_string()),
        api_key: "secret".to_string(),
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
            images: false,
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
            chat_completions: false,
            alpha_search: false,
            image_generation: false,
            image_edit: false,
            function_tools: true,
            custom_tools: true,
            tool_search: true,
            reasoning: true,
            chat_reasoning: asb_core::contracts::CodexChatReasoning::Unsupported,
        },
        parameters: asb_core::ownership::default_provider_parameters(
            asb_core::contracts::AppKind::Codex,
        ),
        notes: None,
        website_url: None,
        usage_query: None,
    }
}

#[test]
fn codex_store_writes_and_reads_only_the_specialized_file() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let created = store.create_codex_provider(uuid::Uuid::new_v4().to_string(), draft()).unwrap();
    let listed = store.list_codex_providers().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].profile, created.profile);
}

#[test]
fn new_codex_provider_can_reference_its_own_complete_candidate() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let id = uuid::Uuid::new_v4().to_string();
    let mut input = draft();
    input.subagent_route = Some(asb_core::contracts::CodexSubagentRoute {
        profile_id: id.clone(), model: "codex".into(),
    });
    let created = store.create_codex_provider(id.clone(), input).unwrap();
    assert_eq!(created.profile.id, id);
    assert_eq!(created.profile.subagent_route.unwrap().profile_id, id);
    assert_eq!(store.list_codex_providers().unwrap().len(), 1);
}

#[test]
fn creation_rejects_invalid_or_occupied_identity_without_overwriting() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    assert!(store.create_codex_provider("../not-an-id".into(), draft()).is_err());
    let id = uuid::Uuid::new_v4().to_string();
    let created = store.create_codex_provider(id.clone(), draft()).unwrap();
    let mut replacement = draft();
    replacement.api_key = "replacement-fixture-key".into();
    assert!(store.create_codex_provider(id.clone(), replacement).is_err());
    assert_eq!(store.find_codex_provider_with_revision(&id).unwrap().1, created.file_hash);
}

#[test]
fn codex_route_exists_marks_a_stored_route_without_blocking_completion() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let created = store.create_codex_provider(uuid::Uuid::new_v4().to_string(), draft()).unwrap();
    assert!(store.codex_route_exists(&created.profile.endpoint, created.profile.upstream));
    assert!(!store.codex_route_exists(
        &CodexEndpoint("https://other.example/v1".to_string()),
        created.profile.upstream
    ));
    assert!(
        !store.codex_route_exists(&created.profile.endpoint, CodexUpstream::ChatCompletions)
    );
}

#[test]
fn codex_store_rejects_the_old_generic_file_shape() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let path = store
        .providers_dir(asb_core::contracts::AppKind::Codex)
        .join("00000000-0000-0000-0000-000000000001.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{"id":"00000000-0000-0000-0000-000000000001","model":null}"#,
    )
    .unwrap();
    assert!(matches!(store.list_codex_providers(),
        Err(ProfileStoreError::InvalidFile { path: failed, .. }) if failed == path));
}

#[test]
fn codex_store_updates_only_the_expected_revision_and_preserves_position() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let created = store.create_codex_provider(uuid::Uuid::new_v4().to_string(), draft()).unwrap();
    let mut changed = draft();
    changed.name = "Updated relay".to_string();
    let saved = store
        .update_codex_provider(&created.profile.id, changed, &created.file_hash)
        .unwrap();
    assert_eq!(saved.profile.name, "Updated relay");
    assert_ne!(saved.file_hash, created.file_hash);
    assert!(store
        .update_codex_provider(&saved.profile.id, draft(), &created.file_hash)
        .is_err());
}

#[test]
fn codex_endpoint_usage_updates_only_a_configured_fallback() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let fallback = "https://backup.example/v1";
    let mut configured = draft();
    configured.connection.custom_endpoints.insert(
        fallback.to_string(),
        ProviderEndpoint {
            url: fallback.to_string(),
            added_at: 1,
            last_used: None,
        },
    );
    let created = store.create_codex_provider(uuid::Uuid::new_v4().to_string(), configured).unwrap();

    assert!(store
        .mark_codex_endpoint_used(&created.profile.id, "https://backup.example/v1/")
        .unwrap());
    let stored = store.find_codex_provider_file(&created.profile.id).unwrap();
    assert!(stored.profile.connection.custom_endpoints[fallback].last_used.is_some());
    assert!(!store
        .mark_codex_endpoint_used(&created.profile.id, &created.profile.endpoint.0)
        .unwrap());
}

#[test]
fn codex_store_level_failures_stay_typed() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    std::fs::create_dir_all(store.legacy_store_path().parent().unwrap()).unwrap();
    std::fs::write(store.legacy_store_path(), b"{}").unwrap();
    assert_eq!(
        store.create_codex_provider(uuid::Uuid::new_v4().to_string(), draft()),
        Err(StoreOperationError::Store(ProfileStoreError::invalid(
            &store.legacy_store_path(), "检测到旧配置布局，无法按当前格式读取",
        )))
    );
}

#[test]
fn codex_store_deletes_only_the_expected_revision() {
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let created = store.create_codex_provider(uuid::Uuid::new_v4().to_string(), draft()).unwrap();
    assert!(store
        .delete_codex_provider(&created.profile.id, "stale")
        .is_err());
    store
        .delete_codex_provider(&created.profile.id, &created.file_hash)
        .unwrap();
    assert!(store.list_codex_providers().unwrap().is_empty());
}
