use super::tool_names::{parse_target_name, render_target_name};
use super::{
    error, CanonicalResponse, Reasoning, ReasoningTransport, ResponsePart, StopReason, ToolKind,
    TransformError, CODEX_TOOL_SEARCH_NAME,
};
use asb_core::contracts::UpstreamProtocol;
use serde_json::{json, Map, Value};

/// Logical response result for accounting, independent of the HTTP envelope.
/// A valid partial result stays 2xx for routing health, but is not a completed generation.
pub(crate) fn response_accounting_status(value: &Value) -> Option<u16> {
    if value.get("error").is_some_and(|error| !error.is_null()) {
        return Some(502);
    }
    match value.get("status").and_then(Value::as_str) {
        Some("failed") => Some(502),
        Some("incomplete") => Some(206),
        _ => None,
    }
}

pub(crate) fn convert_response(
    from: UpstreamProtocol,
    to: UpstreamProtocol,
    body: &[u8],
    reasoning_transport: Option<&ReasoningTransport>,
) -> Result<Vec<u8>, TransformError> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|_| TransformError("上游响应不是有效 JSON".to_string()))?;
    if from == to {
        if !value.is_object() {
            return Err(TransformError("上游响应必须是 JSON 对象".to_string()));
        }
        return Ok(body.to_vec());
    }
    let mut response = parse_response(from, &value, reasoning_transport)?;
    if to == UpstreamProtocol::Responses {
        decode_target_tool_names(&mut response)?;
    }
    let value = render_response(to, &response)?;
    serde_json::to_vec(&value).map_err(|_| TransformError("无法编码转换后的响应".to_string()))
}

fn decode_target_tool_names(response: &mut CanonicalResponse) -> Result<(), TransformError> {
    for part in &mut response.content {
        let ResponsePart::ToolCall {
            name, namespace, ..
        } = part
        else {
            continue;
        };
        let (decoded_namespace, decoded_name, kind) = parse_target_name(name)?;
        *namespace = decoded_namespace;
        *name = decoded_name;
        if let ResponsePart::ToolCall {
            kind: target_kind, ..
        } = part
        {
            *target_kind = kind;
        }
    }
    Ok(())
}

pub(super) fn parse_response(
    protocol: UpstreamProtocol,
    value: &Value,
    reasoning_transport: Option<&ReasoningTransport>,
) -> Result<CanonicalResponse, TransformError> {
    match protocol {
        UpstreamProtocol::Responses => parse_responses(value, reasoning_transport),
        UpstreamProtocol::ChatCompletions => parse_chat(value, reasoning_transport),
        UpstreamProtocol::AnthropicMessages => parse_anthropic(value, reasoning_transport),
        UpstreamProtocol::GeminiGenerateContent => {
            super::claude_gemini::response(value, reasoning_transport)
        }
    }
}

pub(super) fn render_response(
    protocol: UpstreamProtocol,
    response: &CanonicalResponse,
) -> Result<Value, TransformError> {
    match protocol {
        UpstreamProtocol::Responses => render_responses(response),
        UpstreamProtocol::ChatCompletions => render_chat(response),
        UpstreamProtocol::AnthropicMessages => render_anthropic(response),
        UpstreamProtocol::GeminiGenerateContent => error("Gemini Native 不是客户端协议"),
    }
}

/// Generates the selected client protocol's normal error envelope. The error
/// text describes local gateway errors. Provider failures use the shared
/// structured diagnostic envelope at the transport boundary.
pub(crate) fn convert_error(to: UpstreamProtocol, status: u16, message: &str) -> Vec<u8> {
    let value = match to {
        UpstreamProtocol::AnthropicMessages => json!({
            "type": "error",
            "error": { "type": "api_error", "message": message },
        }),
        UpstreamProtocol::Responses => json!({
            "object": "error",
            "status": status,
            "error": { "type": "gateway_error", "message": message },
        }),
        UpstreamProtocol::GeminiGenerateContent => {
            json!({"error":{"code":status,"message":message,"status":"INTERNAL"}})
        }
        UpstreamProtocol::ChatCompletions => json!({
            "error": { "message": message, "type": "gateway_error", "code": status.to_string() },
        }),
    };
    serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec())
}

mod custom;
pub(super) mod lifecycle;
mod parse;
mod render;
mod responses;

pub(super) use responses::{
    responses_envelope, responses_reasoning_item, responses_status, responses_text_item,
    responses_tool_call_item,
};

use custom::*;
use parse::*;
use render::*;
use responses::render_responses;
