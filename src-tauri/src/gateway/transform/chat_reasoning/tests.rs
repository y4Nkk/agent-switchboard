use super::*;
use asb_core::contracts::{
    CodexChatEffortMode as Mode, CodexChatEffortParameter as Effort,
    CodexChatThinkingParameter as Thinking,
};

fn converted(
    thinking: Thinking,
    separate_output: bool,
    parameter: Effort,
    mode: Mode,
    effort: Option<&str>,
) -> Result<Value, TransformError> {
    let configuration = CodexChatReasoning::Configured {
        thinking_parameter: thinking,
        separate_output,
        effort_parameter: parameter,
        effort_mode: mode,
    };
    let mut body = json!({"model":"m", "input":"hello", "store":false});
    if let Some(effort) = effort {
        body["reasoning"] = json!({"effort":effort});
    }
    let request = crate::gateway::transform::convert_request(
        UpstreamProtocol::Responses,
        UpstreamProtocol::ChatCompletions,
        body.to_string().as_bytes(),
        None,
        None,
        Some(&configuration),
    )?;
    Ok(serde_json::from_slice(&request.body).unwrap())
}

#[test]
fn separate_reasoning_output_does_not_require_a_per_request_effort() {
    for effort in [None, Some("high")] {
        let wire = converted(Thinking::None, true, Effort::None, Mode::Passthrough, effort).unwrap();
        assert_eq!(wire["reasoning_split"], true);
        assert!(wire.get("reasoning_effort").is_none());
        assert!(wire.get("thinking").is_none());
    }
    let wire = converted(Thinking::None, false, Effort::None, Mode::Passthrough, None).unwrap();
    assert!(wire.get("reasoning_split").is_none());
}

#[test]
fn output_format_cannot_claim_to_disable_model_thinking() {
    for effort in ["none", "off", "disabled"] {
        let failure = converted(Thinking::None, true, Effort::None, Mode::Passthrough, Some(effort))
            .err().expect("format-only configuration cannot disable thinking");
        assert!(failure.0.contains("固定推理模式不能关闭"));
    }
    let failure = converted(Thinking::None, true, Effort::ReasoningEffort, Mode::DeepSeek, Some("none"))
        .err().expect("restricted effort needs a real off mechanism");
    assert!(failure.0.contains("无法表达关闭推理"));
}

#[test]
fn explicit_effort_off_keeps_the_separate_output_format() {
    for (parameter, field) in [
        (Effort::ReasoningEffort, "reasoning_effort"),
        (Effort::ReasoningObject, "reasoning"),
    ] {
        let wire = converted(Thinking::None, true, parameter, Mode::Passthrough, Some("none")).unwrap();
        assert_eq!(wire["reasoning_split"], true);
        if parameter == Effort::ReasoningObject {
            assert_eq!(wire[field]["effort"], "none");
        } else {
            assert_eq!(wire[field], "none");
        }
    }
}

#[test]
fn real_thinking_switches_still_disable_reasoning() {
    for parameter in [Thinking::Thinking, Thinking::EnableThinking] {
        let wire = converted(parameter, true, Effort::None, Mode::Passthrough, Some("none")).unwrap();
        assert_eq!(wire["reasoning_split"], true);
        match parameter {
            Thinking::Thinking => assert_eq!(wire["thinking"]["type"], "disabled"),
            Thinking::EnableThinking => assert_eq!(wire["enable_thinking"], false),
            _ => unreachable!(),
        }
    }
}

#[test]
fn effort_mapping_and_summary_validation_remain_independent_of_output_format() {
    let wire = converted(Thinking::None, true, Effort::ReasoningEffort, Mode::DeepSeek, Some("xhigh")).unwrap();
    assert_eq!(wire["reasoning_split"], true);
    assert_eq!(wire["reasoning_effort"], "max");
    let configuration = CodexChatReasoning::Configured {
        thinking_parameter: Thinking::None,
        separate_output: true,
        effort_parameter: Effort::None,
        effort_mode: Mode::Passthrough,
    };
    for summary in ["auto", "detailed"] {
        let body = json!({"model":"m", "input":"hi", "store":false, "reasoning":{"summary":summary}});
        let result = crate::gateway::transform::convert_request(
            UpstreamProtocol::Responses, UpstreamProtocol::ChatCompletions,
            body.to_string().as_bytes(), None, None, Some(&configuration),
        );
        assert_eq!(result.is_ok(), summary == "auto");
    }
}
