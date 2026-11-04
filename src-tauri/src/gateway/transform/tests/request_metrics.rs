use super::*;
use asb_core::contracts::UpstreamProtocol;
use serde_json::json;
use std::io::{Cursor, Read};
use crate::gateway::transform::usage::frame_has_token;

#[test]
fn first_output_requires_content_and_never_uses_terminal_or_metadata_events() {
    for value in [json!({"type":"response.created"}), json!({"type":"response.in_progress"}),
        json!({"type":"response.completed","response":{"usage":{"output_tokens":100}}}),
        json!({"type":"response.output_text.delta","delta":""})]
    {
        assert!(!frame_has_token(UpstreamProtocol::Responses, &value));
    }
    for kind in ["response.output_text.delta", "response.reasoning_text.delta",
        "response.reasoning_summary_text.delta", "response.function_call_arguments.delta"]
    {
        assert!(frame_has_token(UpstreamProtocol::Responses, &json!({"type":kind,"delta":"content"})));
    }
    assert!(frame_has_token(UpstreamProtocol::AnthropicMessages,
        &json!({"type":"content_block_start","content_block":{"type":"text","text":"hello"}})));
    assert!(!frame_has_token(UpstreamProtocol::GeminiGenerateContent,
        &json!({"candidates":[{"content":{"parts":[{"functionCall":{}}]}}]})));
    assert!(frame_has_token(UpstreamProtocol::ChatCompletions,
        &json!({"choices":[{"delta":{"tool_calls":[{"function":{"name":"read_file"}}]}}]})));
}

#[test]
fn native_failed_frames_keep_their_bytes_but_do_not_report_completed_requests() {
    let source = concat!("data: {\"type\":\"response.created\"}\n\n",
        "data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"message\":\"failed\"}}}\n\n");
    let mut stream = SseTranscoder::new(Cursor::new(source.as_bytes()), UpstreamProtocol::Responses,
        UpstreamProtocol::Responses, 1024 * 1024, None).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    assert!(stream.failed());
    assert!(!stream.has_token());
    assert_eq!(String::from_utf8(bytes).unwrap(), source);
}

#[test]
fn partial_output_is_distinct_from_failure_and_a_complete_generation() {
    let source = "data: {\"type\":\"response.incomplete\",\"response\":{\"status\":\"incomplete\",\"error\":null}}\n\n";
    let mut stream = SseTranscoder::new(Cursor::new(source.as_bytes()), UpstreamProtocol::Responses,
        UpstreamProtocol::Responses, 1024 * 1024, None).unwrap();
    stream.read_to_end(&mut Vec::new()).unwrap();
    assert!(stream.incomplete());
    assert!(!stream.failed());
    assert_eq!(response_accounting_status(&json!({"status":"incomplete"})), Some(206));
    assert_eq!(response_accounting_status(&json!({"status":"failed"})), Some(502));
    assert_eq!(response_accounting_status(&json!({"status":"completed","error":null})), None);
}
