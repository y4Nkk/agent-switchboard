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
        apply_request(protocol, endpoint, &mut body).unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value.get("prompt_cache_retention").is_none(), stripped);
    }
    let mut body = br##"{"tools":[{"type":"function","function":{"name":"t","parameters":{"properties":{"p":{"$ref":"#/x","description":"d"}}}}}]}"##.to_vec();
    apply_request(UpstreamProtocol::ChatCompletions, "https://api.kimi.com/coding/v1", &mut body).unwrap();
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert!(value["tools"][0]["function"]["parameters"]["properties"]["p"].get("allOf").is_some());
}
