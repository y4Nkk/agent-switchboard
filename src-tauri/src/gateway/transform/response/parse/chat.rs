//! Chat response envelopes, message content, and function tool calls.

use super::*;
use crate::gateway::transform::usage;

pub(super) fn parse(
    value: &Value,
    reasoning_transport: Option<&ReasoningTransport>,
) -> Result<CanonicalResponse, TransformError> {
    let map = parse_envelope(value)?;
    let choice = parse_choice(map)?;
    let message = parse_message(choice)?;
    let content = parse_content(message, reasoning_transport)?;
    Ok(CanonicalResponse {
        id: string(map.get("id"), "id")?,
        model: string(map.get("model"), "model")?,
        content,
        stop: parse_chat_stop(choice.get("finish_reason"))?,
        usage: usage::parse(UpstreamProtocol::ChatCompletions, map.get("usage"))?,
    })
}

fn parse_envelope(value: &Value) -> Result<&Map<String, Value>, TransformError> {
    let map = object(value, "Chat 响应")?;
    allowed(
        map,
        &[
            "id",
            "object",
            "created",
            "model",
            "choices",
            "usage",
            "system_fingerprint",
            "service_tier",
        ],
        "Chat 响应",
    )?;
    if let Some(service_tier) = map.get("service_tier") {
        if !service_tier.is_null() {
            service_tier
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    TransformError("Chat service_tier 必须是非空字符串或 null".to_string())
                })?;
        }
    }
    Ok(map)
}

fn parse_choice(map: &Map<String, Value>) -> Result<&Map<String, Value>, TransformError> {
    let choice = array(
        map.get("choices")
            .ok_or_else(|| TransformError("Chat 响应缺少 choices".to_string()))?,
        "choices",
    )?
    .first()
    .ok_or_else(|| TransformError("Chat 响应 choices 为空".to_string()))?;
    let choice = object(choice, "choice")?;
    allowed(
        choice,
        &["index", "message", "finish_reason", "logprobs"],
        "Chat choice",
    )?;
    if choice.get("logprobs").is_some() && !choice.get("logprobs").is_some_and(Value::is_null) {
        return error("Chat logprobs 无法安全转换");
    }
    Ok(choice)
}

fn parse_message(choice: &Map<String, Value>) -> Result<&Map<String, Value>, TransformError> {
    let message = object(
        choice
            .get("message")
            .ok_or_else(|| TransformError("Chat choice 缺少 message".to_string()))?,
        "Chat message",
    )?;
    allowed(
        message,
        &[
            "role",
            "content",
            "tool_calls",
            "refusal",
            "reasoning_content",
        ],
        "Chat message",
    )?;
    if string(message.get("role"), "message.role")? != "assistant" {
        return error("Chat 响应 message 不是 assistant");
    }
    Ok(message)
}

fn parse_content(
    message: &Map<String, Value>,
    reasoning_transport: Option<&ReasoningTransport>,
) -> Result<Vec<ResponsePart>, TransformError> {
    let mut content = Vec::new();
    if let Some(reasoning_content) = message.get("reasoning_content") {
        match reasoning_content {
            Value::Null => {}
            Value::String(value) => {
                let transport = reasoning_transport
                    .ok_or_else(|| TransformError("当前转换缺少本机推理续接通道".to_string()))?;
                content.push(ResponsePart::Reasoning(
                    transport.from_chat_content(value.clone())?,
                ));
            }
            _ => return error("Chat reasoning_content 必须是字符串或 null"),
        }
    }
    if let Some(value) = message.get("content") {
        if let Some(text) = value.as_str() {
            content.push(ResponsePart::Text(text.to_string()));
        } else if !value.is_null() {
            return error("Chat assistant content 必须是字符串或 null");
        }
    }
    if let Some(refusal) = message.get("refusal") {
        if let Some(text) = refusal.as_str() {
            content.push(ResponsePart::Text(text.to_string()));
        } else if !refusal.is_null() {
            return error("Chat refusal 必须是字符串或 null");
        }
    }
    if let Some(calls) = message.get("tool_calls") {
        for value in array(calls, "tool_calls")? {
            content.push(parse_tool_call(value)?);
        }
    }
    Ok(content)
}

fn parse_tool_call(value: &Value) -> Result<ResponsePart, TransformError> {
    let call = object(value, "tool_call")?;
    allowed(call, &["id", "type", "function"], "tool_call")?;
    if string(call.get("type"), "tool_call.type")? != "function" {
        return error("Chat tool_call 仅支持 function");
    }
    let function = object(
        call.get("function")
            .ok_or_else(|| TransformError("tool_call 缺少 function".to_string()))?,
        "tool_call.function",
    )?;
    allowed(function, &["name", "arguments"], "tool_call.function")?;
    Ok(ResponsePart::ToolCall {
        id: string(call.get("id"), "tool_call.id")?,
        name: string(function.get("name"), "tool_call.function.name")?,
        namespace: None,
        kind: ToolKind::Function,
        input: serde_json::from_str(&string(
            function.get("arguments"),
            "tool_call.function.arguments",
        )?)
        .map_err(|_| TransformError("Chat tool_call arguments 不是 JSON".to_string()))?,
    })
}
