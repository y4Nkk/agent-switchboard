use super::*;
use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use std::io::{self, Cursor};
use tokio::sync::{mpsc, watch};

#[derive(Default)]
struct Wire(Vec<u8>);

impl Read for Wire {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { Ok(0) }
}

impl Write for Wire {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

fn route() -> ActiveRoute {
    ActiveRoute {
        app: AppKind::Codex, profile_id: "fixture".into(), fingerprint: "revision".into(),
        client_token: "client-token".into(), continuation_key: [0; 32],
        upstream_base_url: "https://api.x.ai/v1".into(), connection: Default::default(),
        upstream_protocol: UpstreamProtocol::Responses, responses_options: None,
        max_output_tokens: None, api_key: "provider-key".into(),
        authentication: asb_core::AuthenticationScheme::Bearer,
        claude_primary_model: None, claude_model_options: None, claude_account: None,
        codex_account: None, claude_fragment: Default::default(), codex: None,
    }
}

fn response(route: &ActiveRoute) -> Value {
    let mut request = json!({"tools":[{"type":"namespace","name":"mcp__files__","tools":[{
        "type":"function","name":"read","parameters":{},
    }]}]}).to_string().into_bytes();
    crate::gateway::transform::prepare_upstream_request(route, &mut request).unwrap();
    let request: Value = serde_json::from_slice(&request).unwrap();
    json!({"id":"response-one","object":"response","status":"completed","model":"m",
        "error":null,"vendor_metadata":{"kept":true},"output":[{
            "type":"function_call","id":"item-one","call_id":"call-one","status":"completed",
            "name":request["tools"][0]["name"],"arguments":"{\"limit\":5.0,\"text\":\"中文\"}",
        }],
    })
}

fn upstream(body: &[u8], stream: bool) -> UpstreamResponse {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(
        if stream { "text/event-stream" } else { "application/json" },
    ));
    let (sender, receiver) = mpsc::channel(body.len().max(1));
    for byte in body {
        sender.try_send(Ok(Bytes::copy_from_slice(&[*byte]))).unwrap();
    }
    drop(sender);
    let (cancellation, _) = watch::channel(false);
    UpstreamResponse::new(
        reqwest::StatusCode::OK, headers,
        reqwest::Url::parse("https://api.x.ai/v1/responses").unwrap(),
        true, receiver, cancellation,
    )
}

fn prepare(context: &mut ConversationContext, previous: Option<&str>) -> PendingRequest {
    let mut value = json!({"type":"response.create","model":"m","input":[]});
    if let Some(previous) = previous {
        value["previous_response_id"] = json!(previous);
        value["input"] = json!([{"type":"function_call_output","call_id":"call-one","output":"ok"}]);
    }
    match context.prepare_native("revision", &value.to_string()).unwrap() {
        PreparedResponse::Upstream(request) => request,
        PreparedResponse::Prewarm(_) => panic!("request must reach the upstream"),
    }
}

fn events(socket: &WebSocket<Wire>) -> Vec<Value> {
    let mut client = WebSocket::from_raw_socket(Cursor::new(socket.get_ref().0.clone()), Role::Client, None);
    let mut events = Vec::new();
    loop {
        let Message::Text(text) = client.read().unwrap() else { panic!("expected text frame") };
        let event: Value = serde_json::from_str(&text).unwrap();
        let terminal = matches!(event["type"].as_str(), Some("response.completed" | "response.failed"));
        events.push(event);
        if terminal { return events; }
    }
}

fn assert_restored(call: &Value) {
    assert_eq!(call["name"], "read");
    assert_eq!(call["namespace"], "mcp__files__");
    let arguments: Value = serde_json::from_str(call["arguments"].as_str().unwrap()).unwrap();
    assert_eq!(arguments["limit"].as_u64(), Some(5));
    assert_eq!(arguments["text"], "中文");
}

fn assert_replay(context: &mut ConversationContext) {
    let next = prepare(context, Some("response-one"));
    let body: Value = serde_json::from_slice(&next.body).unwrap();
    assert_restored(&body["input"][0]);
    assert_eq!(body["input"][1]["type"], "function_call_output");
}

fn span() -> RequestSpan {
    RequestSpan::start(
        Arc::new(crate::gateway::metrics::GatewayMetrics::new()),
        AppKind::Codex, UpstreamProtocol::Responses,
    )
}

#[test]
fn native_json_restores_websocket_frames_and_the_following_tool_replay() {
    let route = route();
    let response = response(&route);
    let mut context = ConversationContext::for_route("revision");
    let request = prepare(&mut context, None);
    let mut socket = WebSocket::from_raw_socket(Wire::default(), Role::Server, None);
    let outcome = relay_response(
        &mut socket, upstream(response.to_string().as_bytes(), false),
        &route, &mut context, &request, &mut span(),
    );
    assert!(matches!(outcome, ExchangeOutcome::Served));
    let events = events(&socket);
    assert_eq!(events[0]["type"], "response.created");
    assert_restored(&events[1]["response"]["output"][0]);
    assert_eq!(events[1]["response"]["vendor_metadata"], response["vendor_metadata"]);
    assert_replay(&mut context);
}

#[test]
fn byte_fragmented_sse_restores_websocket_frames_and_the_following_tool_replay() {
    let route = route();
    let response = response(&route);
    let item = &response["output"][0];
    let body = format!(
        "event: response.output_item.done\r\ndata: {{\"type\":\"response.output_item.done\",\"item\":{item}}}\r\n\r\nevent: response.completed\r\ndata: {{\"type\":\"response.completed\",\"response\":{response}}}\r\n\r\n",
    );
    let mut context = ConversationContext::for_route("revision");
    let request = prepare(&mut context, None);
    let mut socket = WebSocket::from_raw_socket(Wire::default(), Role::Server, None);
    let outcome = relay_stream(
        &mut socket, upstream(body.as_bytes(), true), &route, &mut context, &request, &mut span(),
    );
    assert!(matches!(outcome, ExchangeOutcome::Served));
    let events = events(&socket);
    assert_restored(&events[0]["item"]);
    assert_restored(&events[1]["response"]["output"][0]);
    assert_replay(&mut context);
}

#[test]
fn truncated_sse_emits_a_failed_frame_without_committing_a_continuation() {
    let mut context = ConversationContext::for_route("revision");
    let request = prepare(&mut context, None);
    let mut socket = WebSocket::from_raw_socket(Wire::default(), Role::Server, None);
    let outcome = relay_stream(
        &mut socket, upstream(b"event: response.completed\ndata: {\"type\":", true),
        &route(), &mut context, &request, &mut span(),
    );
    assert!(matches!(outcome, ExchangeOutcome::Rejected));
    assert_eq!(events(&socket)[0]["type"], "response.failed");
    let next = json!({"type":"response.create","model":"m","input":[],"previous_response_id":"response-one"});
    assert!(context.prepare_native("revision", &next.to_string()).is_err());
}
