#[path = "chat/anthropic.rs"]
mod anthropic;
#[path = "chat/responses.rs"]
mod responses;

pub(in crate::gateway::transform::stream) use anthropic::ChatToAnthropic;
pub(in crate::gateway::transform::stream) use responses::ChatToResponses;

use super::{json_data, Frame};
use crate::gateway::transform::{usage, StopReason, ToolKind, TransformError, Usage};
use asb_core::contracts::UpstreamProtocol;
use serde_json::{Map, Value};

#[derive(Default)]
struct ChatSource {
    id: Option<String>,
    model: Option<String>,
    usage: Usage,
    stop: Option<StopReason>,
    pending_reasoning: Option<String>,
    reasoning_seen: bool,
    done: bool,
}

#[derive(Default)]
struct ChatUpdate {
    id: Option<String>,
    model: Option<String>,
    usage: Option<Usage>,
    text: Vec<String>,
    reasoning: Vec<String>,
    calls: Vec<CallDelta>,
    finish: Option<StopReason>,
    done: bool,
}

struct CallDelta {
    index: u64,
    id: Option<String>,
    name: Option<String>,
    arguments: Option<String>,
}

struct TextOutput {
    index: u64,
    text: String,
}

struct ResponseCall {
    id: Option<String>,
    name: Option<String>,
    rendered_name: Option<String>,
    namespace: Option<String>,
    kind: ToolKind,
    arguments: String,
    sent_arguments: usize,
    output_index: Option<u64>,
}

impl Default for ResponseCall {
    fn default() -> Self {
        Self {
            id: None,
            name: None,
            rendered_name: None,
            namespace: None,
            kind: ToolKind::Function,
            arguments: String::new(),
            sent_arguments: 0,
            output_index: None,
        }
    }
}

#[derive(Default)]
struct AnthropicCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
    sent_arguments: usize,
    content_index: Option<u64>,
}

impl ChatSource {
    fn apply(&mut self, update: &ChatUpdate) -> Result<(), TransformError> {
        if self.done {
            return Err(TransformError(
                "Chat SSE 在 [DONE] 后继续发送数据".to_string(),
            ));
        }
        merge_identity(&mut self.id, update.id.as_deref(), "Chat SSE id")?;
        merge_identity(&mut self.model, update.model.as_deref(), "Chat SSE model")?;
        if let Some(usage) = &update.usage {
            self.usage.merge_from(usage);
        }
        if let Some(stop) = update.finish {
            if self.stop.is_some_and(|current| {
                std::mem::discriminant(&current) != std::mem::discriminant(&stop)
            }) {
                return Err(TransformError(
                    "Chat SSE finish_reason 在流中变化".to_string(),
                ));
            }
            self.stop = Some(stop);
        }
        for reasoning in &update.reasoning {
            if !reasoning.is_empty() || !self.reasoning_seen {
                self.pending_reasoning
                    .get_or_insert_with(String::new)
                    .push_str(reasoning);
                self.reasoning_seen = true;
            }
        }
        if update.done {
            if self.stop.is_none() {
                return Err(TransformError(
                    "Chat SSE 在缺少 finish_reason 时收到 [DONE]".to_string(),
                ));
            }
            self.done = true;
        }
        Ok(())
    }

    fn identity(&self) -> Result<(String, String), TransformError> {
        Ok((
            self.id
                .clone()
                .ok_or_else(|| TransformError("Chat SSE 缺少 id".to_string()))?,
            self.model
                .clone()
                .ok_or_else(|| TransformError("Chat SSE 缺少 model".to_string()))?,
        ))
    }

    fn stop(&self) -> StopReason {
        self.stop.expect("validated before [DONE]")
    }
}

fn parse_chat_frame(frame: &Frame) -> Result<ChatUpdate, TransformError> {
    if frame.data == "[DONE]" {
        if frame.event.is_some() {
            return Err(TransformError(
                "Chat [DONE] 帧不应带 event 名称".to_string(),
            ));
        }
        return Ok(ChatUpdate {
            done: true,
            ..ChatUpdate::default()
        });
    }
    if frame.event.is_some() {
        return Err(TransformError("Chat SSE 不支持命名 event 帧".to_string()));
    }
    let value = json_data(frame, "Chat SSE data")?;
    let map = object(&value, "Chat SSE data")?;
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
        "Chat SSE data",
    )?;
    let usage = match map.get("usage") {
        Some(Value::Null) | None => None,
        Some(value) => Some(parse_usage(value)?),
    };
    let mut update = ChatUpdate {
        id: optional_identity(map, "id")?,
        model: optional_identity(map, "model")?,
        usage,
        ..ChatUpdate::default()
    };
    if let Some(service_tier) = map.get("service_tier") {
        if !service_tier.is_null() {
            service_tier
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    TransformError("Chat SSE service_tier 必须是非空字符串或 null".to_string())
                })?;
        }
    }
    if let Some(choices) = map.get("choices") {
        parse_chat_choices(choices, &mut update)?;
    }
    Ok(update)
}

fn parse_chat_choices(choices: &Value, update: &mut ChatUpdate) -> Result<(), TransformError> {
    let choices = choices
        .as_array()
        .ok_or_else(|| TransformError("Chat SSE choices 必须是数组".to_string()))?;
    if choices.is_empty() {
        return Ok(());
    }
    if choices.len() != 1 {
        return Err(TransformError("Chat SSE 仅支持单个 choice".to_string()));
    }
    let choice = object(&choices[0], "Chat SSE choice")?;
    allowed(
        choice,
        &["index", "delta", "finish_reason", "logprobs"],
        "Chat SSE choice",
    )?;
    if choice.get("index").and_then(Value::as_u64) != Some(0) {
        return Err(TransformError(
            "Chat SSE 仅支持 index=0 的 choice".to_string(),
        ));
    }
    if choice.get("logprobs").is_some_and(|value| !value.is_null()) {
        return Err(TransformError("Chat SSE logprobs 无法安全转换".to_string()));
    }
    if let Some(delta) = choice.get("delta").filter(|value| !value.is_null()) {
        parse_chat_delta(delta, update)?;
    }
    update.finish = choice
        .get("finish_reason")
        .filter(|value| !value.is_null())
        .map(parse_finish_reason)
        .transpose()?;
    Ok(())
}

fn parse_chat_delta(value: &Value, update: &mut ChatUpdate) -> Result<(), TransformError> {
    let delta = object(value, "Chat SSE delta")?;
    allowed(
        delta,
        &[
            "role",
            "content",
            "tool_calls",
            "refusal",
            "reasoning_content",
        ],
        "Chat SSE delta",
    )?;
    if optional_identity(delta, "role")?.is_some_and(|role| role != "assistant") {
        return Err(TransformError(
            "Chat SSE delta.role 不是 assistant".to_string(),
        ));
    }
    for field in ["content", "refusal"] {
        if let Some(text) = optional_string(delta, field)?.filter(|value| !value.is_empty()) {
            update.text.push(text);
        }
    }
    if let Some(reasoning) = optional_string(delta, "reasoning_content")? {
        update.reasoning.push(reasoning);
    }
    if let Some(calls) = delta.get("tool_calls").filter(|value| !value.is_null()) {
        for call in calls
            .as_array()
            .ok_or_else(|| TransformError("Chat SSE tool_calls 必须是数组或 null".to_string()))?
        {
            let call = parse_call_delta(call)?;
            if call.id.is_some()
                || call.name.is_some()
                || call
                    .arguments
                    .as_ref()
                    .is_some_and(|value| !value.is_empty())
            {
                update.calls.push(call);
            }
        }
    }
    Ok(())
}

fn parse_call_delta(value: &Value) -> Result<CallDelta, TransformError> {
    let call = object(value, "Chat SSE tool_call")?;
    allowed(
        call,
        &["index", "id", "type", "function"],
        "Chat SSE tool_call",
    )?;
    if optional_identity(call, "type")?.is_some_and(|kind| kind != "function") {
        return Err(TransformError(
            "Chat SSE 仅支持 function 工具调用".to_string(),
        ));
    }
    let function = call
        .get("function")
        .filter(|value| !value.is_null())
        .map(|value| object(value, "Chat SSE function"))
        .transpose()?;
    if let Some(function) = function {
        allowed(function, &["name", "arguments"], "Chat SSE function")?;
    }
    Ok(CallDelta {
        index: call
            .get("index")
            .and_then(Value::as_u64)
            .ok_or_else(|| TransformError("Chat SSE tool_call 缺少 index".to_string()))?,
        id: optional_identity(call, "id")?,
        name: function
            .map(|function| optional_identity(function, "name"))
            .transpose()?
            .flatten(),
        arguments: function
            .map(|function| optional_string(function, "arguments"))
            .transpose()?
            .flatten(),
    })
}

fn parse_finish_reason(reason: &Value) -> Result<StopReason, TransformError> {
    match reason.as_str() {
        Some("tool_calls") => Ok(StopReason::ToolUse),
        Some("length") => Ok(StopReason::MaxTokens),
        Some("stop") => Ok(StopReason::EndTurn),
        Some(other) => Err(TransformError(format!(
            "Chat SSE finish_reason {other} 不支持转换"
        ))),
        None => Err(TransformError(
            "Chat SSE finish_reason 必须是字符串或 null".to_string(),
        )),
    }
}

fn merge_identity(
    destination: &mut Option<String>,
    incoming: Option<&str>,
    context: &str,
) -> Result<(), TransformError> {
    let Some(incoming) = incoming else {
        return Ok(());
    };
    if incoming.is_empty() {
        return Err(TransformError(format!("{context} 不能为空")));
    }
    match destination {
        Some(current) if current != incoming => {
            Err(TransformError(format!("{context} 在流中变化")))
        }
        Some(_) => Ok(()),
        None => {
            *destination = Some(incoming.to_string());
            Ok(())
        }
    }
}

fn object<'a>(value: &'a Value, context: &str) -> Result<&'a Map<String, Value>, TransformError> {
    value
        .as_object()
        .ok_or_else(|| TransformError(format!("{context} 必须是对象")))
}

fn allowed(map: &Map<String, Value>, fields: &[&str], context: &str) -> Result<(), TransformError> {
    for field in map.keys() {
        if !fields.contains(&field.as_str()) {
            return Err(TransformError(format!(
                "{context} 包含无法安全转换的字段 {field}"
            )));
        }
    }
    Ok(())
}

fn optional_string(
    map: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, TransformError> {
    match map.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(TransformError(format!(
            "Chat SSE {field} 必须是字符串或 null"
        ))),
    }
}

fn optional_identity(
    map: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, TransformError> {
    Ok(optional_string(map, field)?.filter(|value| !value.is_empty()))
}

fn parse_usage(value: &Value) -> Result<Usage, TransformError> {
    usage::parse(UpstreamProtocol::ChatCompletions, Some(value))
}

