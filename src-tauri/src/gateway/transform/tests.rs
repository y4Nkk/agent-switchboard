use super::*;
use asb_core::contracts::UpstreamProtocol;
use serde_json::json;
use std::io::{Cursor, Read};

mod request_boundaries;
mod chat_reasoning_roundtrip;

fn converted_json(from: UpstreamProtocol, body: Value) -> Value {
    let bytes = convert_response(from, UpstreamProtocol::Responses, &body.to_string().into_bytes(), None)
        .expect("supported response");
    serde_json::from_slice(&bytes).expect("response JSON")
}

fn chat_response(stop: &str) -> Value {
    json!({"id":"r","model":"m","choices":[{
        "index":0,"message":{"role":"assistant","content":"partial"},"finish_reason":stop,
    }]})
}

fn anthropic_response(stop: &str, content: Value) -> Value {
    json!({"id":"r","type":"message","role":"assistant","model":"m",
        "content":content,"stop_reason":stop,"stop_sequence":null})
}

#[test]
fn token_limit_json_is_incomplete_for_both_codex_bridges() {
    for (protocol, body) in [
        (UpstreamProtocol::ChatCompletions, chat_response("length")),
        (UpstreamProtocol::AnthropicMessages, anthropic_response("max_tokens", json!([
            {"type":"text","text":"partial"},
        ]))),
    ] {
        let response = converted_json(protocol, body);
        assert_eq!(response["status"], "incomplete");
        assert_eq!(response["incomplete_details"]["reason"], "max_output_tokens");
        assert_eq!(response["output"][0]["status"], "incomplete");
    }
    let complete = converted_json(UpstreamProtocol::ChatCompletions, chat_response("stop"));
    assert_eq!(complete["status"], "completed");
    assert!(complete.get("incomplete_details").is_none());
}

#[test]
fn json_text_blocks_keep_distinct_items_and_ids() {
    let content = json!([
        {"type":"text","text":"first"}, {"type":"text","text":"second"},
        {"type":"tool_use","id":"call","name":"read","input":{}},
        {"type":"text","text":"last"},
    ]);
    let response = converted_json(UpstreamProtocol::AnthropicMessages, anthropic_response("end_turn", content));
    let output = response["output"].as_array().unwrap();
    assert_eq!(output.len(), 4);
    assert_eq!(output[0]["id"], "msg_r_0");
    assert_eq!(output[1]["id"], "msg_r_1");
    assert_eq!(output[3]["id"], "msg_r_3");
}

fn event(kind: &str, mut value: Value) -> String {
    value["type"] = json!(kind);
    format!("event: {kind}\ndata: {value}\n\n")
}

fn message_start() -> String {
    event("message_start", json!({"message":{
        "id":"r","type":"message","role":"assistant","model":"m","content":[],
        "stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":0},
    }}))
}

fn block(index: u64, content: Value, delta: Option<Value>) -> String {
    let mut stream = event("content_block_start", json!({"index":index,"content_block":content}));
    if let Some(delta) = delta {
        stream.push_str(&event("content_block_delta", json!({"index":index,"delta":delta})));
    }
    stream.push_str(&event("content_block_stop", json!({"index":index})));
    stream
}

fn message_stop(stop: &str) -> String {
    event("message_delta", json!({"delta":{"stop_reason":stop,"stop_sequence":null},
        "usage":{"output_tokens":3}})) + &event("message_stop", json!({}))
}

fn converted_stream(from: UpstreamProtocol, source: String) -> (Vec<Value>, bool) {
    let transport = ReasoningTransport::from_continuation_key([7; 32]);
    let mut reader = SseTranscoder::new(
        Cursor::new(source.into_bytes()), from, UpstreamProtocol::Responses, 1_000_000, Some(&transport),
    ).expect("bridge");
    let mut output = String::new();
    reader.read_to_string(&mut output).expect("read conversion");
    let events = output.lines().filter_map(|line| line.strip_prefix("data: "))
        .map(|line| serde_json::from_str(line).expect("event JSON")).collect();
    (events, reader.failed())
}

fn assert_snapshot_matches_items(events: &[Value], terminal: &str) {
    let response = &events.last().expect("terminal")["response"];
    assert_eq!(events.last().unwrap()["type"], terminal);
    let output = response["output"].as_array().expect("snapshot");
    let added = events.iter().filter(|e| e["type"] == "response.output_item.added").collect::<Vec<_>>();
    let done = events.iter().filter(|e| e["type"] == "response.output_item.done").collect::<Vec<_>>();
    assert_eq!(output.len(), added.len());
    assert_eq!(output.len(), done.len());
    for (index, item) in output.iter().enumerate() {
        let started = added.iter().find(|e| e["output_index"] == index).expect("added index");
        let finished = done.iter().find(|e| e["output_index"] == index).expect("done index");
        assert_eq!(started["item"]["id"], item["id"]);
        assert_eq!(finished["item"], *item);
    }
}

#[test]
fn anthropic_stream_snapshot_preserves_blocks_and_skips_empty_thinking_indices() {
    let mut source = message_start();
    source += &block(0, json!({"type":"thinking","thinking":""}), None);
    source += &block(1, json!({"type":"text","text":"one"}), None);
    source += &block(2, json!({"type":"text","text":"two"}), None);
    source += &block(3, json!({"type":"tool_use","id":"c","name":"read","input":{}}),
        Some(json!({"type":"input_json_delta","partial_json":"{}"})));
    source += &block(4, json!({"type":"text","text":"three"}), None);
    source += &message_stop("end_turn");
    let (events, failed) = converted_stream(UpstreamProtocol::AnthropicMessages, source);
    assert!(!failed);
    assert_snapshot_matches_items(&events, "response.completed");
    assert_eq!(events.last().unwrap()["response"]["output"].as_array().unwrap().len(), 4);
}

#[test]
fn anthropic_stream_keeps_reasoning_item_identity() {
    let source = message_start()
        + &block(0, json!({"type":"thinking","thinking":"reason","signature":"sig"}), None)
        + &block(1, json!({"type":"text","text":"answer"}), None)
        + &message_stop("end_turn");
    let (events, failed) = converted_stream(UpstreamProtocol::AnthropicMessages, source);
    assert!(!failed);
    assert_snapshot_matches_items(&events, "response.completed");
}

#[test]
fn anthropic_stream_token_limit_keeps_partial_function_arguments_incomplete() {
    let source = message_start()
        + &block(0, json!({"type":"tool_use","id":"c","name":"read","input":{}}),
            Some(json!({"type":"input_json_delta","partial_json":"{\"path\":"})))
        + &message_stop("max_tokens");
    let (events, failed) = converted_stream(UpstreamProtocol::AnthropicMessages, source);
    assert!(!failed);
    assert_snapshot_matches_items(&events, "response.incomplete");
    let response = &events.last().unwrap()["response"];
    assert_eq!(response["output"][0]["arguments"], "{\"path\":");
    assert_eq!(response["output"][0]["status"], "incomplete");
    assert_eq!(response["incomplete_details"]["reason"], "max_output_tokens");
}

fn chat_frame(delta: Value, finish: Value) -> String {
    format!("data: {}\n\n", json!({"id":"r","model":"m","choices":[{
        "index":0,"delta":delta,"finish_reason":finish,
    }]}))
}

#[test]
fn chat_stream_token_limit_does_not_complete_partial_tool_calls() {
    let delta = json!({"tool_calls":[{"index":0,"id":"c","type":"function",
        "function":{"name":"read","arguments":"{\"path\":"}}]});
    let source = chat_frame(delta, Value::Null) + &chat_frame(json!({}), json!("length")) + "data: [DONE]\n\n";
    let (events, failed) = converted_stream(UpstreamProtocol::ChatCompletions, source);
    assert!(!failed);
    assert_snapshot_matches_items(&events, "response.incomplete");
    assert_eq!(events.last().unwrap()["response"]["output"][0]["arguments"], "{\"path\":");
}

#[test]
fn chat_stream_snapshot_retains_reasoning_text_and_exact_tool_arguments() {
    let arguments = "{ \"path\" : \"file\" }";
    let source = chat_frame(json!({"reasoning_content":"reason"}), Value::Null)
        + &chat_frame(json!({"content":"answer"}), Value::Null)
        + &chat_frame(json!({"tool_calls":[{"index":0,"id":"c","type":"function",
            "function":{"name":"read","arguments":arguments}}]}), Value::Null)
        + &chat_frame(json!({}), json!("tool_calls")) + "data: [DONE]\n\n";
    let (events, failed) = converted_stream(UpstreamProtocol::ChatCompletions, source);
    assert!(!failed);
    assert_snapshot_matches_items(&events, "response.completed");
    assert_eq!(events.last().unwrap()["response"]["output"][2]["arguments"], arguments);
}

#[test]
fn malformed_completed_tool_arguments_are_still_rejected() {
    let delta = json!({"tool_calls":[{"index":0,"id":"c","type":"function",
        "function":{"name":"read","arguments":"{"}}]});
    let source = chat_frame(delta, Value::Null) + &chat_frame(json!({}), json!("tool_calls")) + "data: [DONE]\n\n";
    let (events, failed) = converted_stream(UpstreamProtocol::ChatCompletions, source);
    assert!(failed);
    assert!(!events.iter().any(|event| event["type"] == "response.completed"));
}

#[test]
fn empty_claude_stop_sequences_are_omitted_but_nonempty_sequences_remain_unsupported() {
    for sequences in [json!([]), json!(["stop"]), json!("invalid")] {
        let body = json!({"model":"m","max_tokens":20,"messages":[{"role":"user","content":"hi"}],
            "stop_sequences":sequences});
        let result = convert_request(UpstreamProtocol::AnthropicMessages, UpstreamProtocol::Responses,
            body.to_string().as_bytes(), None, None, None);
        if sequences == json!([]) {
            let converted: Value = serde_json::from_slice(&result.expect("empty list is lossless").body).unwrap();
            assert!(converted.get("stop_sequences").is_none());
        } else {
            assert!(result.is_err());
        }
    }
}
