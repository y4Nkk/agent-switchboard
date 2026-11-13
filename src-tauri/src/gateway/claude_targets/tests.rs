use super::*;
use asb_core::contracts::{ClaudeModelRoutes, ProviderDraft};
use serde_json::json;
use tiny_http::{Header, Response, Server};

fn draft(name: &str, url: &str) -> ProviderDraft {
    serde_json::from_value(json!({
        "app":"claude", "routeMode":"custom", "name":name, "baseUrl":url,
        "apiKey":format!("fake-{name}"), "model":"vendor-sonnet", "upstreamProtocol":"anthropicMessages", "maxOutputTokens":null,
        "parameters":asb_core::ownership::default_provider_parameters(AppKind::Claude)
    })).unwrap()
}

fn routed_draft(name: &str, url: &str, target: &str) -> ProviderDraft {
    let mut source = draft(name, url);
    source.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
        routes: ClaudeModelRoutes {
            opus: Some(ClaudeModelRoute::Provider { profile_id:target.into(), model:"vendor-sonnet".into(), one_m:false }),
            ..Default::default()
        }, ..Default::default()
    }));
    source
}

#[test]
fn explicit_roles_use_target_credentials_and_models_for_json_and_streaming_without_recursion() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let target_server = Server::http(("127.0.0.1", 0)).unwrap();
    let primary_server = Server::http(("127.0.0.1", 0)).unwrap();
    let mut target = draft("target", &format!("http://{}", target_server.server_addr()));
    target.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
        routes: ClaudeModelRoutes { sonnet: Some(ClaudeModelRoute::Primary { model:"wrong-recursive-model".into(), one_m:false }), ..Default::default() },
        ..Default::default()
    }));
    let target = state.configuration().create_provider(uuid::Uuid::new_v4().to_string(), target).unwrap();
    let source = state.configuration().create_provider(uuid::Uuid::new_v4().to_string(), routed_draft("source", &format!("http://{}", primary_server.server_addr()), &target.profile.id)).unwrap();
    let gateway = GatewayController::start(&state);
    let projection = gateway.project_with_candidates(&state, &SwitchPlan::direct(source.profile.clone(),
        asb_core::ownership::default_client_settings(AppKind::Claude)), None).unwrap();
    assert!(projection.plan.is_gateway());
    gateway.commit(&projection, || Ok(())).unwrap();
    let client_url = format!("{}/v1/messages", projection.plan.client_base_url().unwrap());
    let client_token = projection.plan.client_api_key().to_string();
    let worker = thread::spawn(move || {
        for streaming in [false, true] {
            let mut request = target_server.recv_timeout(Duration::from_secs(10)).unwrap().expect("target request");
            assert_eq!(request.url(), "/v1/messages");
            assert!(request.headers().iter().any(|header| header.value.as_str().contains("fake-target")));
            assert!(!request.headers().iter().any(|header| header.value.as_str().contains("fake-source")));
            let mut body = String::new(); request.as_reader().read_to_string(&mut body).unwrap();
            let value:serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(value["model"], "vendor-sonnet");
            let (content, mime) = if streaming {
                ("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"fake\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"vendor-sonnet\",\"content\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".to_string(), "text/event-stream")
            } else { (json!({"id":"fake", "type":"message", "role":"assistant", "model":"vendor-sonnet", "content":[{"type":"text","text":"ok"}], "stop_reason":"end_turn", "usage":{"input_tokens":1,"output_tokens":1}}).to_string(), "application/json") };
            request.respond(Response::from_string(content).with_header(Header::from_bytes("Content-Type", mime).unwrap())).unwrap();
        }
        assert!(primary_server.recv_timeout(Duration::from_millis(100)).unwrap().is_none());
    });
    let client = reqwest::blocking::Client::builder().no_proxy().timeout(Duration::from_secs(15)).build().unwrap();
    for streaming in [false, true] {
        // Fable inherits the explicit Opus destination when it has no override.
        let response = client.post(&client_url).bearer_auth(&client_token).header("content-type", "application/json").body(json!({
            "model":"asb-claude-fable", "max_tokens":32, "messages":[{"role":"user","content":"test"}], "stream":streaming
        }).to_string()).send().unwrap();
        let status = response.status(); let body = response.text().unwrap();
        assert!(status.is_success(), "{status}: {body}");
    }
    worker.join().unwrap();
    gateway.shutdown();
}

#[test]
fn references_prevent_target_deletion_and_removal_of_the_declared_model() {
    let directory = tempfile::tempdir().unwrap();
    let store = crate::config_store::ConfigStore::new(directory.path().join("state"));
    let target = store.create_provider(uuid::Uuid::new_v4().to_string(), draft("target", "https://target.invalid")).unwrap();
    store.create_provider(uuid::Uuid::new_v4().to_string(), routed_draft("source", "https://source.invalid", &target.profile.id)).unwrap();
    assert!(store.delete_provider(&target.profile.id, &target.file_hash).unwrap_err().to_string().contains("引用"));
    let mut changed = draft("target", "https://target.invalid"); changed.model = Some("removed".into());
    assert!(store.update_provider(&target.profile.id, changed, &target.file_hash).unwrap_err().to_string().contains("已保存模型"));
    assert_eq!(store.find_provider(&target.profile.id).unwrap(), target.profile);
}

#[test]
fn explicit_target_failure_never_falls_back_to_primary_even_with_failover_enabled() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let target_server = Server::http(("127.0.0.1", 0)).unwrap();
    let primary_server = Server::http(("127.0.0.1", 0)).unwrap();
    let target = state.configuration().create_provider(uuid::Uuid::new_v4().to_string(), draft("target", &format!("http://{}", target_server.server_addr()))).unwrap();
    let source = state.configuration().create_provider(uuid::Uuid::new_v4().to_string(), routed_draft("source", &format!("http://{}", primary_server.server_addr()), &target.profile.id)).unwrap();
    let mut policy = failover::ClaudeFailoverPolicy::default();
    policy.enabled = true; policy.max_retries = 1; policy.provider_ids = vec![target.profile.id.clone()];
    failover::save(state.root(), &policy).unwrap();
    let gateway = GatewayController::start(&state);
    let projection = gateway.project_with_candidates(&state, &SwitchPlan::direct(source.profile,
        asb_core::ownership::default_client_settings(AppKind::Claude)), None).unwrap();
    gateway.commit(&projection, || Ok(())).unwrap();
    let worker = thread::spawn(move || {
        for _ in 0..2 {
            let request = target_server.recv_timeout(Duration::from_secs(10)).unwrap().expect("target retry");
            request.respond(Response::from_string("upstream unavailable").with_status_code(503)).unwrap();
        }
        assert!(primary_server.recv_timeout(Duration::from_millis(150)).unwrap().is_none());
    });
    let response = reqwest::blocking::Client::builder().no_proxy().timeout(Duration::from_secs(15)).build().unwrap()
        .post(format!("{}/v1/messages", projection.plan.client_base_url().unwrap()))
        .bearer_auth(projection.plan.client_api_key()).header("content-type", "application/json")
        .body(json!({"model":"asb-claude-opus","max_tokens":32,"messages":[{"role":"user","content":"test"}]}).to_string())
        .send().unwrap();
    assert_eq!(response.status().as_u16(), 503);
    worker.join().unwrap();
    gateway.shutdown();
}

#[test]
fn target_parameter_and_preference_edits_invalidate_prepared_dependency_revision() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let target = state.configuration().create_provider(uuid::Uuid::new_v4().to_string(), draft("target", "https://target.invalid")).unwrap();
    let source = state.configuration().create_provider(uuid::Uuid::new_v4().to_string(), routed_draft("source", "https://source.invalid", &target.profile.id)).unwrap();
    let gateway = GatewayController::start(&state);
    let before = gateway.claude_dependency_revision(&state, &source.profile, None).unwrap();
    let mut changed = draft("target", "https://target.invalid");
    changed.claude_fragment.insert("modelSettings".into(), json!({"vendor-sonnet":{"effortLevel":"high"}}));
    state.configuration().update_provider(&target.profile.id, changed, &target.file_hash).unwrap();
    assert_ne!(before, gateway.claude_dependency_revision(&state, &source.profile, None).unwrap());
    let projection = gateway.project_with_candidates(&state, &SwitchPlan::direct(source.profile,
        asb_core::ownership::default_client_settings(AppKind::Claude)), None).unwrap();
    assert_eq!(projection.plan.profile.claude_fragment["modelSettings"]["asb-claude-fable"]["effortLevel"], "high");
    gateway.shutdown();
}

#[test]
fn request_target_snapshots_wait_for_the_complete_save_transaction_for_both_clients() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    for app in [AppKind::Claude, AppKind::Codex] {
        let (target_id, hash, candidates) = if app == AppKind::Claude {
            let target = state.configuration().create_provider(uuid::Uuid::new_v4().to_string(), draft("target", "https://target.invalid")).unwrap();
            let source = state.configuration().create_provider(uuid::Uuid::new_v4().to_string(), routed_draft("source", "https://source.invalid", &target.profile.id)).unwrap();
            (target.profile.id, target.file_hash, vec![gateway.route_for_profile(&source.profile).unwrap()])
        } else {
            let target = state.configuration().create_codex_provider(uuid::Uuid::new_v4().to_string(), crate::codex_common::test_draft()).unwrap();
            (target.profile.id, target.file_hash, Vec::new())
        };
        let gate = gateway.inner.profile_save_gate.write().unwrap();
        let worker_gateway = gateway.clone(); let id = target_id.clone();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (finished_tx, finished_rx) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            started_tx.send(()).unwrap();
            let key = if app == AppKind::Claude {
                let (routes, _, _) = worker_gateway.resolve_claude_target(candidates,
                    serde_json::to_vec(&json!({"model":"asb-claude-opus"})).unwrap()).unwrap();
                routes[0].api_key.clone()
            } else {
                worker_gateway.inner.subagent_route_candidates(&asb_core::contracts::CodexSubagentRoute {
                    profile_id:id, model:"relay-model".into(),
                }).unwrap().0.api_key
            };
            finished_tx.send(key).unwrap();
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(finished_rx.recv_timeout(Duration::from_millis(50)).is_err());
        if app == AppKind::Claude {
            let mut changed = draft("target", "https://target.invalid"); changed.api_key = "gated-new-key".into();
            state.configuration().update_provider(&target_id, changed, &hash).unwrap();
        } else {
            let mut changed = crate::codex_common::test_draft(); changed.api_key = "gated-new-key".into();
            state.configuration().update_codex_provider(&target_id, changed, &hash).unwrap();
        }
        assert!(finished_rx.recv_timeout(Duration::from_millis(50)).is_err());
        drop(gate);
        assert_eq!(finished_rx.recv_timeout(Duration::from_secs(5)).unwrap(), "gated-new-key");
        worker.join().unwrap();
    }
    gateway.shutdown();
}

#[test]
fn target_model_preferences_override_target_global_defaults_without_changing_source_globals() {
    use asb_core::contracts::{ConfigValue, SettingValue};
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let mut target_draft = draft("target", "https://target.invalid");
    target_draft.claude_fragment.insert("effortLevel".into(), json!("low"));
    target_draft.parameters.settings.insert("maxEffortLevel".into(), SettingValue::Explicit { value:ConfigValue::Str("medium".into()) });
    let target = state.configuration().create_provider(uuid::Uuid::new_v4().to_string(), target_draft.clone()).unwrap();
    let mut source_draft = routed_draft("source", "https://source.invalid", &target.profile.id);
    source_draft.claude_fragment.insert("effortLevel".into(), json!("xhigh"));
    let source = state.configuration().create_provider(uuid::Uuid::new_v4().to_string(), source_draft).unwrap();
    let gateway = GatewayController::start(&state);
    let plan = SwitchPlan::direct(source.profile.clone(), asb_core::ownership::default_client_settings(AppKind::Claude));
    let first = gateway.project_with_candidates(&state, &plan, None).unwrap();
    let role = &first.plan.profile.claude_fragment["modelSettings"]["asb-claude-opus"];
    assert_eq!(role["effortLevel"],"low"); assert_eq!(role["maxEffortLevel"],"medium");
    assert_eq!(first.plan.profile.claude_fragment["effortLevel"],"xhigh");
    target_draft.claude_fragment.insert("modelSettings".into(), json!({"vendor-sonnet":{"effortLevel":"high","maxEffortLevel":"max","autoCompactWindow":"auto"}}));
    state.configuration().update_provider(&target.profile.id,target_draft,&target.file_hash).unwrap();
    let second = gateway.project_with_candidates(&state, &plan, None).unwrap();
    let role = &second.plan.profile.claude_fragment["modelSettings"]["asb-claude-opus"];
    assert_eq!(role["effortLevel"],"high"); assert_eq!(role["maxEffortLevel"],"max");
    assert_eq!(role["autoCompactWindow"],"auto");
    assert_eq!(second.plan.profile.claude_fragment["effortLevel"],"xhigh");
    gateway.shutdown();
}

#[test]
fn target_revision_and_continuation_ignore_endpoint_usage_and_bind_actual_request_facts() {
    use asb_core::contracts::{ProviderEndpoint, ConfigValue, SettingValue};
    let mut target = ProviderProfile::from_draft(Uuid::new_v4().to_string(), draft("target", "https://target.invalid"));
    target.connection.custom_endpoints.insert("https://secondary.invalid".into(), ProviderEndpoint {
        url:"https://secondary.invalid".into(),added_at:1,last_used:None,
    });
    let initial = target_revision(&target).unwrap();
    let initial_key = continuation_key("isolated-identity",&target,&initial);
    target.connection.custom_endpoints.get_mut("https://secondary.invalid").unwrap().last_used = Some(500);
    target.connection.custom_endpoints.get_mut("https://secondary.invalid").unwrap().added_at = 100;
    target.name = "renamed".into(); target.notes = Some("metadata".into()); target.website_url = Some("https://website.invalid".into());
    let after_metadata = target_revision(&target).unwrap();
    assert_eq!(initial,after_metadata);
    assert_eq!(initial_key,continuation_key("isolated-identity",&target,&after_metadata));
    let mut changed = target.clone(); changed.api_key = "changed-key".into();
    assert_ne!(initial,target_revision(&changed).unwrap());
    changed = target.clone(); changed.parameters.settings.insert("maxEffortLevel".into(), SettingValue::Explicit {value:ConfigValue::Str("high".into())});
    assert_ne!(initial,target_revision(&changed).unwrap());
    changed = target; changed.claude_fragment.insert("modelSettings".into(),json!({"vendor-sonnet":{"effortLevel":"high"}}));
    assert_ne!(initial,target_revision(&changed).unwrap());
}
