//! Protocol-specific request parsers producing the canonical request.

use super::*;

mod responses;
pub(super) use responses::parse_responses;

pub(super) fn parse_chat(
    value: &Value,
    reasoning_transport: Option<&ReasoningTransport>,
) -> Result<CanonicalRequest, TransformError> {
    let map = object(value, "Chat Completions 请求")?;
    allowed(map, CHAT_REQUEST_FIELDS, "Chat Completions 请求")?;
    let mut system = Vec::new();
    let mut messages = Vec::new();
    for value in array(
        map.get("messages")
            .ok_or_else(|| TransformError("Chat Completions 请求缺少 messages".to_string()))?,
        "messages",
    )? {
        let message = parse_chat_message(object(value, "message")?, reasoning_transport)?;
        if matches!(message.role, Role::System | Role::Developer) {
            system.extend(message.parts);
        } else {
            messages.push(message);
        }
    }
    Ok(CanonicalRequest {
        model: string(map.get("model"), "model")?,
        system,
        messages,
        tools: parse_chat_tools(map.get("tools"))?,
        tool_choice: parse_chat_tool_choice(map.get("tool_choice"))?,
        parallel_tool_calls: optional_bool_option(map, "parallel_tool_calls")?,
        stream: optional_bool(map, "stream")?,
        max_tokens: optional_u64(map, "max_completion_tokens")?
            .or(optional_u64(map, "max_tokens")?),
        temperature: optional_number(map, "temperature")?,
        top_p: optional_number(map, "top_p")?,
        stop: parse_chat_stop_sequences(map.get("stop"))?,
        user_id: match map.get("user") {
            None => None,
            Some(value) => Some(string(Some(value), "user")?),
        },
        reasoning_effort: parse_chat_reasoning_effort(map.get("reasoning_effort"))?,
    })
}

const CHAT_REQUEST_FIELDS: &[&str] = &[
    "model",
    "messages",
    "tools",
    "tool_choice",
    "parallel_tool_calls",
    "stream",
    "max_tokens",
    "max_completion_tokens",
    "temperature",
    "top_p",
    "stop",
    "user",
    "reasoning_effort",
];

fn parse_chat_message(
    item: &Map<String, Value>,
    transport: Option<&ReasoningTransport>,
) -> Result<Message, TransformError> {
    allowed(
        item,
        &[
            "role",
            "content",
            "tool_calls",
            "tool_call_id",
            "reasoning_content",
        ],
        "Chat message",
    )?;
    let role_name = string(item.get("role"), "message.role")?;
    let role = parse_role(&role_name)?;
    let mut parts = match item.get("content") {
        Some(Value::Null) | None => vec![],
        Some(content) => parse_chat_content(content)?,
    };
    if let Some(reasoning) = item.get("reasoning_content") {
        parse_chat_reasoning_content(reasoning, role, &mut parts, transport)?;
    }
    if let Some(calls) = item.get("tool_calls") {
        if role != Role::Assistant {
            return error("tool_calls 只能出现在 assistant message 中");
        }
        parts.extend(parse_chat_tool_calls(calls)?);
    }
    if role_name == "tool" {
        parts = vec![Part::ToolResult {
            id: string(item.get("tool_call_id"), "tool_call_id")?,
            kind: ToolKind::Function,
            content: parts,
            is_error: false,
        }];
    } else if item.contains_key("tool_call_id") {
        return error("Chat tool 结果必须使用 role=tool");
    }
    Ok(Message { role, parts })
}

fn parse_chat_reasoning_content(
    value: &Value,
    role: Role,
    parts: &mut Vec<Part>,
    transport: Option<&ReasoningTransport>,
) -> Result<(), TransformError> {
    if role != Role::Assistant {
        return error("reasoning_content 只能出现在 assistant message 中");
    }
    match value {
        Value::Null => {}
        Value::String(content) => {
            let transport = transport
                .ok_or_else(|| TransformError("当前转换缺少本机推理续接通道".to_string()))?;
            parts.insert(
                0,
                Part::Reasoning(transport.from_chat_content(content.clone())?),
            );
        }
        _ => return error("reasoning_content 必须是字符串或 null"),
    }
    Ok(())
}

pub(super) fn parse_anthropic(
    value: &Value,
    reasoning_transport: Option<&ReasoningTransport>,
) -> Result<CanonicalRequest, TransformError> {
    let map = object(value, "Anthropic Messages 请求")?;
    allowed(map, ANTHROPIC_REQUEST_FIELDS, "Anthropic Messages 请求")?;
    let tools = parse_anthropic_tools(map.get("tools"))?;
    let system = match map.get("system") {
        None => vec![],
        Some(Value::String(text)) => vec![Part::Text(text.clone())],
        Some(value) => parse_anthropic_content(value, Role::System, reasoning_transport, &tools)?,
    };
    let messages = parse_anthropic_messages(map, reasoning_transport, &tools)?;
    let thinking_effort = parse_anthropic_thinking(map.get("thinking"))?;
    let reasoning_effort =
        parse_anthropic_output_effort(map.get("output_config"))?.or(thinking_effort);
    Ok(CanonicalRequest {
        model: string(map.get("model"), "model")?,
        system,
        messages,
        tools,
        tool_choice: parse_anthropic_tool_choice(map.get("tool_choice"))?,
        parallel_tool_calls: parse_anthropic_parallel_tool_calls(map.get("tool_choice"))?,
        stream: optional_bool(map, "stream")?,
        max_tokens: optional_u64(map, "max_tokens")?,
        temperature: optional_number(map, "temperature")?,
        top_p: optional_number(map, "top_p")?,
        stop: parse_anthropic_stop_sequences(map.get("stop_sequences"))?,
        user_id: parse_user_metadata(map.get("metadata"), "Anthropic metadata")?,
        reasoning_effort,
    })
}

const ANTHROPIC_REQUEST_FIELDS: &[&str] = &[
    "model",
    "messages",
    "system",
    "tools",
    "tool_choice",
    "stream",
    "max_tokens",
    "temperature",
    "top_p",
    "stop_sequences",
    "metadata",
    "thinking",
    "output_config",
];

fn parse_anthropic_messages(
    map: &Map<String, Value>,
    transport: Option<&ReasoningTransport>,
    tools: &[Tool],
) -> Result<Vec<Message>, TransformError> {
    let mut messages = Vec::new();
    for value in array(
        map.get("messages")
            .ok_or_else(|| TransformError("Anthropic Messages 请求缺少 messages".to_string()))?,
        "messages",
    )? {
        let item = object(value, "Anthropic message")?;
        allowed(item, &["role", "content"], "Anthropic message")?;
        let role = match string(item.get("role"), "message.role")?.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            "system" => Role::System,
            other => return error(format!("Anthropic message.role {other} 不支持")),
        };
        let content = item
            .get("content")
            .ok_or_else(|| TransformError("Anthropic message 缺少 content".to_string()))?;
        messages.push(Message {
            role,
            parts: parse_anthropic_content(content, role, transport, tools)?,
        });
    }
    Ok(messages)
}
