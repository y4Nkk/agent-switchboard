use super::*;
use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::{json, Value};
use tokio::sync::{mpsc, watch};

fn codex_route(upstream_protocol: UpstreamProtocol) -> ActiveRoute {
    ActiveRoute {
        app: asb_core::contracts::AppKind::Codex,
        profile_id: "test".into(), fingerprint: "test".into(), client_token: "client-token".into(),
        continuation_key: [0; 32], upstream_base_url: "https://vendor.example/v1".into(),
        connection: Default::default(), upstream_protocol, responses_options: None,
        max_output_tokens: None, api_key: "provider-key".into(),
        authentication: asb_core::AuthenticationScheme::Bearer,
        claude_primary_model: None, claude_model_options: None, claude_account: None,
        codex_account: None, claude_fragment: Default::default(), codex: None,
    }
}

fn json_response(body: &'static [u8]) -> UpstreamResponse {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let (sender, receiver) = mpsc::channel(1);
    sender.try_send(Ok(Bytes::from_static(body))).unwrap();
    drop(sender);
    let (cancellation, _) = watch::channel(false);
    UpstreamResponse::new(
        reqwest::StatusCode::OK, headers,
        reqwest::Url::parse("https://vendor.example/v1/chat/completions").unwrap(),
        true, receiver, cancellation,
    )
}

#[test]
fn codex_cross_protocol_response_is_preflighted() {
    assert!(requires_non_stream_preflight(asb_core::contracts::AppKind::Codex, UpstreamProtocol::ChatCompletions));
    assert!(requires_non_stream_preflight(asb_core::contracts::AppKind::Codex, UpstreamProtocol::AnthropicMessages));
}

#[test]
fn malformed_codex_chat_response_fails_before_candidate_delivery() {
    let result = prepare_non_stream_response(
        json_response(br#"{"id":"chat-1","model":"relay"}"#),
        &codex_route(UpstreamProtocol::ChatCompletions),
    );
    assert!(matches!(result, Err(ProviderDiagnostic { kind: ProviderFailureKind::ResponseParse, .. })));
}

#[test]
fn codex_native_response_waits_for_final_restoration() {
    assert!(!requires_non_stream_preflight(asb_core::contracts::AppKind::Codex, UpstreamProtocol::Responses));
    assert!(requires_non_stream_preflight(asb_core::contracts::AppKind::Claude, UpstreamProtocol::AnthropicMessages));
}

#[test]
fn shared_http_and_websocket_json_rendering_restores_upstream_tool_spelling() {
    let mut route = codex_route(UpstreamProtocol::Responses);
    route.upstream_base_url = "https://api.x.ai/v1".into();
    let mut request = json!({"tools":[{"type":"namespace","name":"mcp__files__","tools":[{
        "type":"function","name":"read","parameters":{},
    }]}]}).to_string().into_bytes();
    crate::gateway::transform::apply_request_compat(&route, &mut request).unwrap();
    let request: Value = serde_json::from_slice(&request).unwrap();
    let response = json!({"id":"r","object":"response","status":"completed","model":"m",
        "vendor_metadata":{"kept":true},"output":[{"type":"function_call",
        "name":request["tools"][0]["name"],"arguments":"{\"limit\":5.0}"}],
    });
    let restored = convert_safe_json(response.to_string().into_bytes(), true, UpstreamProtocol::Responses, &route).unwrap();
    let restored: Value = serde_json::from_slice(&restored).unwrap();
    assert_eq!(restored["output"][0]["name"], "read");
    assert_eq!(restored["output"][0]["namespace"], "mcp__files__");
    assert_eq!(restored["output"][0]["arguments"], "{\"limit\":5}");
    assert_eq!(restored["vendor_metadata"], response["vendor_metadata"]);
}

#[test]
fn native_response_restoration_respects_the_client_protocol_and_endpoint() {
    let body = br#"{"output":[{"type":"function_call","name":"asbns_n_2_bnM_4_cmVhZA","arguments":"{\"n\":1.0}"}]}"#.to_vec();
    for (app, protocol, endpoint) in [
        (asb_core::AppKind::Codex, UpstreamProtocol::Responses, "https://other.example/v1"),
        (asb_core::AppKind::Claude, UpstreamProtocol::Responses, "https://api.x.ai/v1"),
        (asb_core::AppKind::Codex, UpstreamProtocol::ChatCompletions, "https://api.x.ai/v1"),
    ] {
        let mut route = codex_route(protocol);
        route.app = app;
        route.upstream_base_url = endpoint.into();
        let mut rendered = body.clone();
        crate::gateway::transform::NativeResponsesDialect::for_route(&route).restore_json(&mut rendered);
        assert_eq!(rendered, body);
    }
}
