use crate::gateway::transform::{convert_request, ReasoningTransport, TransformError};
use asb_core::contracts::UpstreamProtocol as Protocol;
use serde_json::{json, Value};

fn converted(from: Protocol, to: Protocol, body: Value) -> Result<Value, TransformError> {
    let transport = ReasoningTransport::from_continuation_key([3; 32]);
    let result = convert_request(from, to, body.to_string().as_bytes(), None, Some(&transport), None)?;
    Ok(serde_json::from_slice(&result.body).unwrap())
}

#[test]
fn empty_assistant_turns_are_removed_between_user_messages_for_both_clients() {
    for empty in [json!(""), json!([]), json!([{"type":"text","text":"  \n"}])] {
        let body = json!({"model":"m","max_tokens":20,"messages":[
            {"role":"user","content":"first"},
            {"role":"assistant","content":empty},
            {"role":"user","content":"second"},
        ]});
        let wire = converted(Protocol::AnthropicMessages, Protocol::ChatCompletions, body).unwrap();
        assert_eq!(wire["messages"], json!([{"role":"user","content":"first\n\nsecond"}]));
    }
    let body = json!({"model":"m","store":false,"input":[
        {"role":"user","content":"first"},
        {"role":"assistant","content":[]},
        {"role":"user","content":"second"},
    ]});
    let wire = converted(Protocol::Responses, Protocol::ChatCompletions, body).unwrap();
    assert_eq!(wire["messages"], json!([{"role":"user","content":"first\n\nsecond"}]));
}

#[test]
fn valid_adjacent_user_turns_and_native_history_keep_their_existing_shape() {
    let body = json!({"model":"m","max_tokens":20,"messages":[
        {"role":"user","content":"first"}, {"role":"user","content":"second"},
    ]});
    let wire = converted(Protocol::AnthropicMessages, Protocol::ChatCompletions, body).unwrap();
    assert_eq!(wire["messages"].as_array().unwrap().len(), 2);
    let body = br#"{"model":"m","max_tokens":20,"messages":[{"role":"assistant","content":[]},{"role":"user","content":"hi"}]}"#;
    let native = convert_request(Protocol::AnthropicMessages, Protocol::AnthropicMessages, body, None, None, None).unwrap();
    assert_eq!(native.body.as_slice(), body);
}

#[test]
fn reasoning_only_history_is_rejected_without_discarding_or_faking_it() {
    let body = json!({"model":"m","max_tokens":20,"messages":[
        {"role":"user","content":"first"},
        {"role":"assistant","content":[{"type":"thinking","thinking":"original reasoning","signature":"sig"}]},
        {"role":"user","content":"continue"},
    ]});
    let failure = converted(Protocol::AnthropicMessages, Protocol::ChatCompletions, body).unwrap_err();
    assert!(failure.0.contains("仅含推理"));
    let body = json!({"model":"m","max_tokens":20,"messages":[{"role":"assistant","content":[]}]});
    let failure = converted(Protocol::AnthropicMessages, Protocol::ChatCompletions, body).unwrap_err();
    assert!(failure.0.contains("不包含可转换的消息"));
}

#[test]
fn removing_empty_turns_keeps_parallel_tool_results_and_moved_media_adjacent() {
    let body = json!({"model":"m","max_tokens":20,"messages":[
        {"role":"user","content":"inspect"},
        {"role":"assistant","content":[
            {"type":"thinking","thinking":"original reasoning","signature":"sig"},
            {"type":"tool_use","id":"a","name":"read","input":{"n":1}},
            {"type":"tool_use","id":"b","name":"read","input":{"n":2}},
        ]},
        {"role":"user","content":[{"type":"tool_result","tool_use_id":"a","content":[
            {"type":"image","source":{"type":"base64","media_type":"image/png","data":"AA=="}},
        ]}]},
        {"role":"assistant","content":[]},
        {"role":"user","content":[{"type":"tool_result","tool_use_id":"b","content":"done"}]},
    ]});
    let wire = converted(Protocol::AnthropicMessages, Protocol::ChatCompletions, body).unwrap();
    let messages = wire["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 5);
    assert_eq!(messages[1]["reasoning_content"], "original reasoning");
    assert_eq!(messages[1]["tool_calls"].as_array().unwrap().len(), 2);
    assert_eq!(messages[2]["role"], "tool");
    assert_eq!(messages[2]["tool_call_id"], "a");
    assert_eq!(messages[3]["role"], "tool");
    assert_eq!(messages[3]["tool_call_id"], "b");
    assert_eq!(messages[4]["role"], "user");
    assert!(messages[4]["content"].as_array().unwrap().iter().any(|part| part["type"] == "image_url"));
}

#[test]
fn anthropic_root_unions_report_the_exact_tool_and_keyword() {
    for keyword in ["anyOf", "oneOf", "allOf"] {
        let mut schema = json!({"type":"object"});
        schema[keyword] = json!([{"type":"object","properties":{"value":{"type":"string"}}}]);
        let body = json!({"model":"m","max_output_tokens":20,"input":"hi","store":false,
            "tools":[{"type":"function","name":"update_record","parameters":schema}],
        });
        let failure = converted(Protocol::Responses, Protocol::AnthropicMessages, body).unwrap_err();
        assert!(failure.0.contains("update_record"));
        assert!(failure.0.contains(keyword));
    }
}

#[test]
fn nested_unions_and_original_tool_constraints_remain_intact() {
    let schema = json!({"type":"object","properties":{"value":{"anyOf":[{"type":"string"},{"type":"null"}]}},
        "required":["value"],"additionalProperties":false});
    let body = json!({"model":"m","max_output_tokens":20,"input":"hi","store":false,
        "tools":[{"type":"function","name":"update_record","parameters":schema}],
    });
    let wire = converted(Protocol::Responses, Protocol::AnthropicMessages, body).unwrap();
    assert_eq!(wire["tools"][0]["input_schema"], schema);
}
