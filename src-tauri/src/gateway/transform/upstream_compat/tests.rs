use super::*;
use crate::gateway::transform::SseTranscoder;
use serde_json::json;
use std::io::{self, Cursor, Read};

struct Fragmented<'a> {
    bytes: &'a [u8],
    size: usize,
}

impl Read for Fragmented<'_> {
    fn read(&mut self, target: &mut [u8]) -> io::Result<usize> {
        let size = self.bytes.len().min(target.len()).min(self.size);
        target[..size].copy_from_slice(&self.bytes[..size]);
        self.bytes = &self.bytes[size..];
        Ok(size)
    }
}

fn call() -> Value {
    let mut request = json!({"tools":[{"type":"namespace","name":"mcp__files__","tools":[{
        "type":"function","name":"read","parameters":{},
    }]}]});
    namespaces::flatten_request_namespaces(&mut request).unwrap();
    json!({"type":"function_call","name":request["tools"][0]["name"],"call_id":"call-1",
        "arguments":json!({"limit":5.0,"text":"中文"}).to_string()})
}

fn source() -> String {
    let item = call();
    format!(
        "id: event-1\r\nretry: 100\r\nevent: response.output_item.done\r\ndata: {{\r\ndata: \"type\":\"response.output_item.done\",\"item\":{item}}}\r\n\r\nevent: response.completed\r\ndata: {{\"type\":\"response.completed\",\"response\":{{\"id\":\"r\",\"status\":\"completed\",\"output\":[{item}]}}}}\r\n\r\n"
    )
}

fn restore_stream(source: &str, size: usize, dialect: NativeResponsesDialect) -> String {
    let reader = Fragmented { bytes: source.as_bytes(), size };
    let mut stream = SseTranscoder::new(
        reader, UpstreamProtocol::Responses, UpstreamProtocol::Responses, 100_000, None,
    ).unwrap().with_native_responses_dialect(dialect);
    let mut output = String::new();
    stream.read_to_string(&mut output).unwrap();
    output
}

#[test]
fn native_events_restore_after_complete_frames_across_all_split_sizes() {
    let source = source();
    for size in 1..=source.len() {
        let output = restore_stream(&source, size, NativeResponsesDialect::Xai);
        assert!(!output.contains("asbns_"), "split {size}");
        assert!(output.contains("中文"));
        assert!(output.contains("id: event-1\nretry: 100\n"));
        let values = output.split("\n\n").filter_map(|event| {
            event.lines().find_map(|line| line.strip_prefix("data: "))
                .map(|data| serde_json::from_str::<Value>(data).unwrap())
        }).collect::<Vec<_>>();
        assert_eq!(values.len(), 2, "split {size}");
        for item in [&values[0]["item"], &values[1]["response"]["output"][0]] {
            assert_eq!(item["name"], "read");
            assert_eq!(item["namespace"], "mcp__files__");
            let arguments: Value = serde_json::from_str(item["arguments"].as_str().unwrap()).unwrap();
            assert_eq!(arguments["limit"].as_u64(), Some(5));
        }
        assert_eq!(values[1]["type"], "response.completed");
    }
}

#[test]
fn small_consumer_buffers_do_not_expose_or_drop_partial_events() {
    let source = source();
    let mut reader = SseTranscoder::new(
        Cursor::new(source), UpstreamProtocol::Responses, UpstreamProtocol::Responses, 100_000, None,
    ).unwrap().with_native_responses_dialect(NativeResponsesDialect::Xai);
    let mut output = Vec::new();
    let mut byte = [0];
    while reader.read(&mut byte).unwrap() != 0 {
        output.extend_from_slice(&byte);
    }
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("response.completed"));
    assert!(!output.contains("asbns_"));
}

#[test]
fn incomplete_native_event_reports_failure_instead_of_successful_eof() {
    let output = restore_stream("event: response.completed\ndata: {\"type\":", 1, NativeResponsesDialect::Xai);
    assert!(output.contains("response.failed"));
    assert!(!output.contains("event: response.completed"));
}

#[test]
fn native_json_and_sse_restore_the_same_tool_identity_and_arguments() {
    let mut bytes = json!({"output":[call()]}).to_string().into_bytes();
    NativeResponsesDialect::Xai.restore_json(&mut bytes);
    let restored: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(restored["output"][0]["name"], "read");
    assert_eq!(restored["output"][0]["namespace"], "mcp__files__");
    let arguments: Value = serde_json::from_str(restored["output"][0]["arguments"].as_str().unwrap()).unwrap();
    assert_eq!(arguments["limit"].as_u64(), Some(5));
    let mut untouched = json!({"output":[call()]}).to_string().into_bytes();
    let original = untouched.clone();
    NativeResponsesDialect::Standard.restore_json(&mut untouched);
    assert_eq!(untouched, original);
}

#[test]
fn standard_responses_keep_native_tool_names() {
    let item = call();
    let source = format!("data: {{\"type\":\"response.completed\",\"response\":{{\"output\":[{item}]}}}}\n\n");
    assert_eq!(restore_stream(&source, 1, NativeResponsesDialect::Standard), source);
}

#[test]
fn request_dialects_only_rewrite_their_selected_protocol_and_host() {
    for (protocol, endpoint, stripped) in [
        (UpstreamProtocol::Responses, "https://api.x.ai/v1", true),
        (UpstreamProtocol::ChatCompletions, "https://api.x.ai/v1", false),
        (UpstreamProtocol::Responses, "https://other.example/v1", false),
    ] {
        let mut body = br#"{"model":"grok-4.6","prompt_cache_retention":"24h"}"#.to_vec();
        apply_request(AppKind::Codex, protocol, endpoint, &mut body).unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value.get("prompt_cache_retention").is_none(), stripped);
    }
    let mut body = br##"{"tools":[{"type":"function","function":{"name":"t","parameters":{"properties":{"p":{"$ref":"#/x","description":"d"}}}}}]}"##.to_vec();
    apply_request(AppKind::Codex, UpstreamProtocol::ChatCompletions, "https://api.kimi.com/coding/v1", &mut body).unwrap();
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert!(value["tools"][0]["function"]["parameters"]["properties"]["p"].get("allOf").is_some());
}

#[test]
fn moonshot_schema_rewrites_cover_both_clients_without_touching_other_upstreams() {
    let source = br##"{"tools":[{"type":"function","function":{"name":"read","parameters":{"type":"object","properties":{"path":{"$ref":"#/$defs/path","description":"file"}},"$defs":{"path":{"type":"string"}}}}}]}"##;
    for app in [AppKind::Codex, AppKind::Claude] {
        let mut body = source.to_vec();
        apply_request(app, UpstreamProtocol::ChatCompletions, "https://api.kimi.com/coding/v1", &mut body).unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        let path = &value["tools"][0]["function"]["parameters"]["properties"]["path"];
        assert_eq!(path["allOf"][0]["$ref"], "#/$defs/path");
        assert_eq!(path["description"], "file");
        let normalized = body.clone();
        apply_request(app, UpstreamProtocol::ChatCompletions, "https://api.kimi.com/coding/v1", &mut body).unwrap();
        assert_eq!(body, normalized);
        for (protocol, endpoint) in [
            (UpstreamProtocol::Responses, "https://api.kimi.com/coding/v1"),
            (UpstreamProtocol::ChatCompletions, "https://api.kimi.com.unrelated.example/v1"),
        ] {
            let mut untouched = source.to_vec();
            apply_request(app, protocol, endpoint, &mut untouched).unwrap();
            assert_eq!(untouched.as_slice(), source);
        }
    }
}

#[test]
fn claude_does_not_inherit_codex_private_xai_rewrites() {
    let source = br#"{"model":"grok-4.6","prompt_cache_retention":"24h"}"#;
    let mut claude = source.to_vec();
    apply_request(AppKind::Claude, UpstreamProtocol::Responses, "https://api.x.ai/v1", &mut claude).unwrap();
    assert_eq!(claude.as_slice(), source);
    let mut codex = source.to_vec();
    apply_request(AppKind::Codex, UpstreamProtocol::Responses, "https://api.x.ai/v1", &mut codex).unwrap();
    let value: Value = serde_json::from_slice(&codex).unwrap();
    assert!(value.get("prompt_cache_retention").is_none());
}

#[test]
fn native_chat_output_limits_keep_the_declared_budget_for_both_clients() {
    for app in [AppKind::Codex, AppKind::Claude] {
        for endpoint in [
            "https://resource.openai.azure.com/openai/v1",
            "https://resource.openai.azure.us/openai/v1",
            "https://resource.openai.azure.cn/openai/v1",
            "https://bedrock-runtime.us-east-1.amazonaws.com/openai/v1",
            "https://bedrock-runtime.cn-north-1.amazonaws.com.cn/openai/v1",
        ] {
            let mut body = json!({"model":"m", "max_tokens":123, "temperature":0.4}).to_string().into_bytes();
            apply_request(app, UpstreamProtocol::ChatCompletions, endpoint, &mut body).unwrap();
            let value: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(value, json!({"model":"m", "max_completion_tokens":123, "temperature":0.4}));
            let prepared = body.clone();
            apply_request(app, UpstreamProtocol::ChatCompletions, endpoint, &mut body).unwrap();
            assert_eq!(body, prepared);
        }
    }
    for (protocol, endpoint) in [
        (UpstreamProtocol::ChatCompletions, "https://openai.azure.com.example/v1"),
        (UpstreamProtocol::ChatCompletions, "https://bedrock-runtime.us-east-1.amazonaws.com.example/v1"),
        (UpstreamProtocol::Responses, "https://resource.openai.azure.com/openai/v1"),
    ] {
        let mut body = br#"{"max_tokens":123}"#.to_vec();
        let original = body.clone();
        apply_request(AppKind::Codex, protocol, endpoint, &mut body).unwrap();
        assert_eq!(body, original);
    }
}

#[test]
fn invalid_limits_and_schema_constraints_fail_without_mutating_wire_bytes() {
    for (endpoint, body) in [
        ("https://resource.openai.azure.com/openai/v1", json!({"max_tokens":12,"max_completion_tokens":13})),
        ("https://resource.openai.azure.com/openai/v1", json!({"max_tokens":1.5})),
        ("https://api.kimi.com/v1", json!({"tools":[{"function":{"parameters":{"$ref":"#/x","allOf":{}}}}]})),
    ] {
        let mut wire = body.to_string().into_bytes();
        let original = wire.clone();
        assert!(apply_request(AppKind::Codex, UpstreamProtocol::ChatCompletions, endpoint, &mut wire).is_err());
        assert_eq!(wire, original);
    }
}

#[test]
fn minimax_requests_structured_reasoning_and_rejects_tag_output() {
    for app in [AppKind::Codex, AppKind::Claude] {
        for endpoint in ["https://api.minimax.io/v1", "https://api.minimaxi.com/v1"] {
            let mut body = br#"{"model":"m","thinking":{"type":"disabled"}}"#.to_vec();
            apply_request(app, UpstreamProtocol::ChatCompletions, endpoint, &mut body).unwrap();
            let value: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(value["reasoning_split"], true);
            assert_eq!(value["thinking"]["type"], "disabled");
            let prepared = body.clone();
            apply_request(app, UpstreamProtocol::ChatCompletions, endpoint, &mut body).unwrap();
            assert_eq!(body, prepared);
            let mut tags = br#"{"reasoning_split":false}"#.to_vec();
            assert!(apply_request(app, UpstreamProtocol::ChatCompletions, endpoint, &mut tags).is_err());
        }
    }
}

#[test]
fn deepseek_requires_original_reasoning_without_fabricating_empty_history() {
    for app in [AppKind::Codex, AppKind::Claude] {
        let mut value = json!({"tools":[{"type":"function","function":{"name":"t","parameters":{}}}],
            "messages":[{"role":"user","content":"hi"},{"role":"assistant","content":"hello"}]});
        let mut wire = value.to_string().into_bytes();
        let original = wire.clone();
        assert!(apply_request(app, UpstreamProtocol::ChatCompletions, "https://api.deepseek.com/v1", &mut wire).is_err());
        assert_eq!(wire, original);
        for reasoning in [json!(""), json!("original trace")] {
            value["messages"][1]["reasoning_content"] = reasoning;
            let mut wire = value.to_string().into_bytes();
            let original = wire.clone();
            apply_request(app, UpstreamProtocol::ChatCompletions, "https://api.deepseek.com/v1", &mut wire).unwrap();
            assert_eq!(wire, original);
        }
        value["messages"][1]["reasoning_content"] = Value::Null;
        let mut wire = value.to_string().into_bytes();
        assert!(apply_request(app, UpstreamProtocol::ChatCompletions, "https://api.deepseek.com/v1", &mut wire).is_err());
        value["thinking"] = json!({"type":"disabled"});
        let mut wire = value.to_string().into_bytes();
        apply_request(app, UpstreamProtocol::ChatCompletions, "https://api.deepseek.com/v1", &mut wire).unwrap();
        value.as_object_mut().unwrap().remove("thinking");
        value.as_object_mut().unwrap().remove("tools");
        let mut wire = value.to_string().into_bytes();
        apply_request(app, UpstreamProtocol::ChatCompletions, "https://api.deepseek.com/v1", &mut wire).unwrap();
    }
}

#[test]
fn deepseek_anthropic_preserves_thinking_and_rejects_unreadable_history() {
    let mut value = json!({"messages":[
        {"role":"assistant","content":[{"type":"thinking","thinking":"plan"},{"type":"tool_use","id":"c","name":"t","input":{}}]},
        {"role":"user","content":[{"type":"tool_result","tool_use_id":"c","content":"ok"}]},
        {"role":"assistant","content":[{"type":"thinking","thinking":"result"},{"type":"text","text":"done"}]},
    ]});
    let mut wire = value.to_string().into_bytes();
    apply_request(AppKind::Claude, UpstreamProtocol::AnthropicMessages, "https://api.deepseek.com/anthropic", &mut wire).unwrap();
    value["messages"][2]["content"] = json!([{"type":"text","text":"done"}]);
    let mut wire = value.to_string().into_bytes();
    assert!(apply_request(AppKind::Claude, UpstreamProtocol::AnthropicMessages, "https://api.deepseek.com/anthropic", &mut wire).is_err());
    value["thinking"] = json!({"type":"disabled"});
    value["messages"][0]["content"] = json!([{"type":"redacted_thinking","data":"opaque"}]);
    let mut wire = value.to_string().into_bytes();
    assert!(apply_request(AppKind::Claude, UpstreamProtocol::AnthropicMessages, "https://api.deepseek.com/anthropic", &mut wire).is_err());
}
