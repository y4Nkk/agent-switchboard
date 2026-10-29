use super::{chat_frame, convert_request, convert_response, ReasoningTransport, SseTranscoder};
use asb_core::contracts::UpstreamProtocol as Protocol;
use serde_json::{json, Value};
use std::io::{Cursor, Read};

fn replay(protocol: Protocol, parts: Value, transport: &ReasoningTransport) -> Value {
    let body = if protocol == Protocol::Responses {
        json!({"model":"m", "store":false, "input":parts})
    } else {
        json!({"model":"m", "max_tokens":20, "messages":[{"role":"assistant","content":parts}]})
    };
    let converted = convert_request(
        protocol, Protocol::ChatCompletions, body.to_string().as_bytes(), None, Some(transport), None,
    ).unwrap();
    serde_json::from_slice(&converted.body).unwrap()
}

#[test]
fn json_response_round_trip_distinguishes_empty_reasoning_from_absent_reasoning() {
    let transport = ReasoningTransport::from_continuation_key([9; 32]);
    for protocol in [Protocol::Responses, Protocol::AnthropicMessages] {
        for reasoning in [None, Some(""), Some("original trace")] {
            let mut body = json!({"id":"r", "model":"m", "choices":[{
                "index":0, "message":{"role":"assistant","content":"answer"}, "finish_reason":"stop",
            }]});
            if let Some(reasoning) = reasoning {
                body["choices"][0]["message"]["reasoning_content"] = json!(reasoning);
            }
            let response = convert_response(
                Protocol::ChatCompletions, protocol, body.to_string().as_bytes(), Some(&transport),
            ).unwrap();
            let value: Value = serde_json::from_slice(&response).unwrap();
            let parts = if protocol == Protocol::Responses { &value["output"] } else { &value["content"] };
            let restored = replay(protocol, parts.clone(), &transport);
            assert_eq!(restored["messages"][0]["content"], "answer");
            assert_eq!(restored["messages"][0].get("reasoning_content"), reasoning.map(|v| json!(v)).as_ref());
        }
    }
}

#[test]
fn chat_tool_history_preserves_an_original_empty_reasoning_field() {
    let transport = ReasoningTransport::from_continuation_key([9; 32]);
    let body = json!({"model":"m", "messages":[
        {"role":"user","content":"inspect"},
        {"role":"assistant","content":null,"reasoning_content":"","tool_calls":[
            {"id":"c","type":"function","function":{"name":"read","arguments":"{}"}},
        ]},
        {"role":"tool","tool_call_id":"c","content":"ok"},
    ]});
    let translated = convert_request(
        Protocol::ChatCompletions, Protocol::Responses, body.to_string().as_bytes(), None, Some(&transport), None,
    ).unwrap();
    let converted: Value = serde_json::from_slice(&translated.body).unwrap();
    let restored = replay(Protocol::Responses, converted["input"].clone(), &transport);
    assert_eq!(restored["messages"][1]["reasoning_content"], "");
    assert_eq!(restored["messages"][1]["tool_calls"][0]["id"], "c");
    assert_eq!(restored["messages"][2]["tool_call_id"], "c");
}

#[test]
fn repeated_empty_stream_fields_produce_one_authentic_continuation_for_each_client() {
    let transport = ReasoningTransport::from_continuation_key([9; 32]);
    let source = chat_frame(json!({"reasoning_content":""}), Value::Null)
        + &chat_frame(json!({"reasoning_content":"","content":"answer"}), Value::Null)
        + &chat_frame(json!({"reasoning_content":""}), json!("stop")) + "data: [DONE]\n\n";
    for protocol in [Protocol::Responses, Protocol::AnthropicMessages] {
        let mut stream = SseTranscoder::new(
            Cursor::new(source.as_bytes()), Protocol::ChatCompletions, protocol, 100_000, Some(&transport),
        ).unwrap();
        let mut output = String::new();
        stream.read_to_string(&mut output).unwrap();
        assert!(!stream.failed());
        let events: Vec<Value> = output.lines().filter_map(|line| line.strip_prefix("data: "))
            .map(|line| serde_json::from_str(line).unwrap()).collect();
        let parts = if protocol == Protocol::Responses {
            let output = &events.last().unwrap()["response"]["output"];
            assert_eq!(output.as_array().unwrap().iter().filter(|v| v["type"] == "reasoning").count(), 1);
            output.clone()
        } else {
            let blocks: Vec<&Value> = events.iter().filter(|v| v["type"] == "content_block_start"
                && v["content_block"]["type"] == "redacted_thinking").collect();
            assert_eq!(blocks.len(), 1);
            json!([blocks[0]["content_block"], {"type":"text","text":"answer"}])
        };
        let restored = replay(protocol, parts, &transport);
        assert_eq!(restored["messages"][0]["reasoning_content"], "");
        assert_eq!(restored["messages"][0]["content"], "answer");
    }
}
