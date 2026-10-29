//! Validate original thinking history; never synthesize a missing trace.

use asb_core::contracts::UpstreamProtocol;
use serde_json::Value;

pub(super) fn validate(protocol: UpstreamProtocol, body: &Value) -> Result<(), String> {
    let messages = body
        .get("messages")
        .and_then(Value::as_array)
        .ok_or("DeepSeek messages 必须是数组")?;
    let thinking = body.pointer("/thinking/type").and_then(Value::as_str) != Some("disabled");
    match protocol {
        UpstreamProtocol::ChatCompletions => {
            let tools = body
                .get("tools")
                .and_then(Value::as_array)
                .is_some_and(|v| !v.is_empty());
            if thinking && tools {
                for (index, message) in messages.iter().enumerate() {
                    if message["role"] == "assistant"
                        && !message.get("reasoning_content").is_some_and(Value::is_string)
                    {
                        return Err(format!("DeepSeek 思考工具请求的第 {} 条 assistant 历史缺少原始 reasoning_content；请提供完整推理历史，不能补造推理", index + 1));
                    }
                }
            }
        }
        UpstreamProtocol::AnthropicMessages => validate_anthropic(messages, thinking)?,
        _ => {}
    }
    Ok(())
}

fn validate_anthropic(messages: &[Value], thinking: bool) -> Result<(), String> {
    let mut after_tool = false;
    for (index, message) in messages.iter().enumerate() {
        let blocks = message.get("content").and_then(Value::as_array);
        let has = |kind: &str| {
            blocks.is_some_and(|values| values.iter().any(|value| value["type"] == kind))
        };
        if has("redacted_thinking") {
            return Err(format!("DeepSeek Anthropic 不支持第 {} 条历史中的 redacted_thinking，不能丢弃不透明推理后继续请求", index + 1));
        }
        if message["role"] == "assistant" {
            if thinking && (has("tool_use") || after_tool) && !has("thinking") {
                return Err(format!("DeepSeek 思考工具请求的第 {} 条 assistant 历史缺少原始 thinking；请提供完整推理历史", index + 1));
            }
            after_tool = false;
        } else {
            after_tool = message["role"] == "user"
                && blocks.is_some_and(|values| {
                    !values.is_empty() && values.iter().all(|value| value["type"] == "tool_result")
                });
        }
    }
    Ok(())
}
