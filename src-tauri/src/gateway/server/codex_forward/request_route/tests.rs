use super::*;
use crate::{gateway::GatewayController, local_state::LocalState};
use serde_json::json;

#[test]
fn compact_uses_target_capabilities_and_only_target_endpoints() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let mut source = crate::codex_common::test_draft();
    source.capabilities.compact = false;
    source.catalog[0].compact = false;
    let source = source.into_file(uuid::Uuid::new_v4().to_string(), 1);
    let primary = gateway.route_for_codex_file(&source, None).unwrap();
    let mut target = crate::codex_common::test_draft();
    target.api_key = "target-secret".into();
    target.endpoint.0 = "https://target.example/v1".into();
    target.model_routes = vec![asb_core::contracts::CodexModelRoute {
        client_model: "relay-model".into(), upstream_model: "vendor-model".into(),
    }];
    let target = state.configuration().create_codex_provider(uuid::Uuid::new_v4().to_string(), target).unwrap();
    let wire = CodexSubagentRoute { profile_id: target.profile.id.clone(), model: "relay-model".into() };
    for operation in [CodexOperation::Compact, CodexOperation::Responses] {
        let input = if operation.is_compact() { json!([]) }
            else { json!([{"type":"compaction_trigger"}]) };
        let body = serde_json::to_vec(&json!({"model":wire.wire_id(),"input":input,"stream":true})).unwrap();
        let (route, candidates, body) = resolve_request_route(
            &gateway.inner, primary.clone(), vec![primary.clone()], operation, body,
        ).unwrap();
        assert_eq!(route.profile_id, target.profile.id);
        assert_eq!(route.api_key, "target-secret");
        assert!(candidates.iter().all(|route| route.profile_id == target.profile.id));
        let mapped = crate::gateway::server::codex::resolve_model_and_validate(
            &route, operation, body,
        ).unwrap();
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&mapped).unwrap()["model"], "vendor-model");
    }
    gateway.shutdown();
}

#[test]
fn unresolved_targets_and_other_auxiliary_operations_never_fall_back() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let source = crate::codex_common::test_draft().into_file(uuid::Uuid::new_v4().to_string(), 1);
    let primary = gateway.route_for_codex_file(&source, None).unwrap();
    let body = serde_json::to_vec(&json!({
        "model":format!("asb:{}/relay-model", uuid::Uuid::new_v4()), "input":[],
    })).unwrap();
    for operation in [CodexOperation::Responses, CodexOperation::Compact, CodexOperation::Models] {
        let error = resolve_request_route(
            &gateway.inner, primary.clone(), vec![primary.clone()], operation, body.clone(),
        ).err().unwrap();
        assert_eq!(error.0, 422);
    }
    gateway.shutdown();
}
