use super::*;
use crate::{gateway::GatewayController, local_state::LocalState};

#[test]
fn target_controls_native_fields_and_continuation_revision() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let target = state.configuration().create_codex_provider(uuid::Uuid::new_v4().to_string(), crate::codex_common::test_draft()).unwrap();
    let source = crate::codex_common::test_draft().into_file(uuid::Uuid::new_v4().to_string(), 1);
    let mut primary = gateway.route_for_codex_file(&source, None).unwrap();
    primary.upstream_protocol = UpstreamProtocol::ChatCompletions;
    let mut context = ConversationContext::for_route(&primary.fingerprint);
    let mut input = json!({"type":"response.create", "model":format!("asb:{}/relay-model",target.profile.id),
        "input":[], "stream":true, "store":false, "service_tier":"priority",
        "reasoning":{"effort":"high"}});
    let resolved = prepare(&gateway.inner, primary.clone(), &mut context, &input.to_string()).unwrap();
    assert_eq!(resolved.route.profile_id, target.profile.id);
    let PreparedResponse::Upstream(pending) = resolved.prepared else { panic!("expected upstream") };
    let body: Value = serde_json::from_slice(&pending.body).unwrap();
    assert_eq!(body["service_tier"], "priority");
    assert_eq!(body["reasoning"]["effort"], "high");
    context.record_completed(&pending, &json!({"id":"response-one","status":"completed","output":[]})).unwrap();
    input["previous_response_id"] = json!("response-one");
    assert!(prepare(&gateway.inner, primary.clone(), &mut context, &input.to_string()).is_ok());
    let mut changed = crate::codex_common::test_draft();
    changed.api_key = "new-target-secret".into();
    state.configuration().update_codex_provider(&target.profile.id, changed, &target.file_hash).unwrap();
    let error = prepare(&gateway.inner, primary, &mut context, &input.to_string()).err().unwrap();
    assert_eq!(error.0, "previous_response_not_found");
    gateway.shutdown();
}

#[test]
fn parent_minimal_mode_does_not_strip_target_reasoning() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let mut target = crate::codex_common::test_draft();
    target.upstream = asb_core::contracts::CodexUpstream::ChatCompletions;
    let target = state.configuration().create_codex_provider(uuid::Uuid::new_v4().to_string(), target).unwrap();
    let mut source = crate::codex_common::test_draft();
    source.request_mode = ResponsesRequestMode::Minimal;
    let source = source.into_file(uuid::Uuid::new_v4().to_string(), 1);
    let primary = gateway.route_for_codex_file(&source, None).unwrap();
    let text = json!({"type":"response.create", "model":format!("asb:{}/relay-model",target.profile.id),
        "input":[],"stream":true,"store":false,"reasoning":{"effort":"high"}}).to_string();
    let mut context = ConversationContext::default();
    let resolved = prepare(&gateway.inner, primary, &mut context, &text).unwrap();
    let PreparedResponse::Upstream(pending) = resolved.prepared else { panic!("expected upstream") };
    let body: Value = serde_json::from_slice(&pending.body).unwrap();
    assert_eq!(body["reasoning"]["effort"], "high");
    gateway.shutdown();
}
