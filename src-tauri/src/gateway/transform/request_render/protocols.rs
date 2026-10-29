//! Protocol-specific request renderers from the canonical request.

use super::*;

pub(super) fn render_anthropic(request: &CanonicalRequest) -> Result<Value, TransformError> {
    let max_tokens = request.max_tokens.ok_or_else(|| {
        TransformError("转换到 Anthropic Messages 时必须提供最大输出 token 数".to_string())
    })?;
    let mut root = Map::new();
    root.insert("model".to_string(), Value::String(request.model.clone()));
    root.insert(
        "messages".to_string(),
        render_anthropic_messages(&request.messages)?,
    );
    root.insert("max_tokens".to_string(), Value::Number(max_tokens.into()));
    if let Some(effort) = request.reasoning_effort {
        root.insert(
            "output_config".to_string(),
            json!({ "effort": effort_name(effort) }),
        );
    }
    if let Some(user_id) = &request.user_id {
        root.insert("metadata".to_string(), json!({ "user_id": user_id }));
    }
    if !request.system.is_empty() {
        root.insert(
            "system".to_string(),
            render_anthropic_content(&request.system)?,
        );
    }
    if !request.tools.is_empty() {
        root.insert("tools".to_string(), render_anthropic_tools(&request.tools)?);
        root.insert(
            "tool_choice".to_string(),
            render_anthropic_tool_choice(&request.tool_choice, request.parallel_tool_calls)?,
        );
    }
    insert_common_anthropic(&mut root, request);
    Ok(Value::Object(root))
}

fn render_anthropic_messages(messages: &[Message]) -> Result<Value, TransformError> {
    let mut output = Vec::new();
    for message in messages {
        let role = match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::Developer => return error("developer 消息无法无损转换到 Anthropic Messages"),
        };
        if role != "assistant"
            && message
                .parts
                .iter()
                .any(|part| matches!(part, Part::Reasoning(_)))
        {
            return error("reasoning 只能出现在 assistant 消息中");
        }
        output.push(json!({ "role": role, "content": render_anthropic_content(&message.parts)? }));
    }
    Ok(Value::Array(output))
}

fn render_anthropic_tools(tools: &[Tool]) -> Result<Value, TransformError> {
    if tools.iter().any(|tool| tool.strict) {
        return error("当前 Anthropic 桥接未声明 strict 工具转换，不能丢弃严格约束");
    }
    let mut values = Vec::new();
    for tool in tools {
        for keyword in ["anyOf", "oneOf", "allOf"] {
            if tool.input_schema.get(keyword).is_some() {
                return error(format!(
                    "工具 {} 的 input_schema 根部包含 {keyword}，无法无损转换到 Anthropic Messages",
                    tool.name,
                ));
            }
        }
        let mut value = Map::new();
        value.insert(
            "name".to_string(),
            Value::String(render_target_name(
                UpstreamProtocol::AnthropicMessages,
                tool.namespace.as_deref(),
                &tool.name,
                tool.kind,
            )?),
        );
        if let Some(description) = &tool.description {
            value.insert(
                "description".to_string(),
                Value::String(description.clone()),
            );
        }
        value.insert("input_schema".to_string(), tool.input_schema.clone());
        values.push(Value::Object(value));
    }
    Ok(Value::Array(values))
}

fn render_responses_input(request: &CanonicalRequest) -> Result<Vec<Value>, TransformError> {
    let mut input = Vec::new();
    for message in &request.messages {
        let role = responses_message_role(message)?;
        let mut ordinary = Vec::new();
        for part in &message.parts {
            match part {
                Part::ToolResult {
                    id,
                    kind,
                    content,
                    is_error,
                } => {
                    if message.role != Role::User {
                        return error("tool_result 只能出现在 user 消息中");
                    }
                    push_responses_message(&mut input, role, &ordinary)?;
                    ordinary.clear();
                    input.push(render_responses_tool_result(id, *kind, content, *is_error)?);
                }
                Part::ToolCall {
                    id,
                    name,
                    namespace,
                    kind,
                    input: arguments,
                } => {
                    if message.role != Role::Assistant {
                        return error("tool_call 只能出现在 assistant 消息中");
                    }
                    push_responses_message(&mut input, role, &ordinary)?;
                    ordinary.clear();
                    input.push(render_responses_tool_call(
                        id, name, namespace, *kind, arguments,
                    )?);
                }
                Part::Reasoning(reasoning) => {
                    if message.role != Role::Assistant {
                        return error("reasoning 只能出现在 assistant 消息中");
                    }
                    push_responses_message(&mut input, role, &ordinary)?;
                    ordinary.clear();
                    input.push(reasoning.responses_input_item()?);
                }
                Part::Document(_)
                | Part::File { .. }
                | Part::Audio { .. }
                | Part::ToolReference(_)
                    if message.role == Role::User =>
                {
                    ordinary.push(part.clone())
                }
                Part::Document(_)
                | Part::File { .. }
                | Part::Audio { .. }
                | Part::ToolReference(_) => {
                    return error("文档和工具引用只能出现在用户输入或工具结果中")
                }
                Part::Text(_) | Part::Image(_) => ordinary.push(part.clone()),
            }
        }
        push_responses_message(&mut input, role, &ordinary)?;
    }
    Ok(input)
}

fn responses_message_role(message: &Message) -> Result<&'static str, TransformError> {
    if message.parts.is_empty() {
        return error("消息不含可转换内容");
    }
    Ok(match message.role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
        Role::Developer => "developer",
    })
}

pub(super) fn render_responses(request: &CanonicalRequest) -> Result<Value, TransformError> {
    if request.stop.is_some() {
        return error("Responses 上游无法表达 stop_sequences 终止序列");
    }
    let mut root = Map::new();
    root.insert("model".to_string(), Value::String(request.model.clone()));
    root.insert(
        "input".to_string(),
        Value::Array(render_responses_input(request)?),
    );
    insert_openai_reasoning(&mut root, request, UpstreamProtocol::Responses);
    if let Some(user_id) = &request.user_id {
        root.insert("metadata".to_string(), json!({ "user_id": user_id }));
    }
    if !request.system.is_empty() {
        root.insert(
            "instructions".to_string(),
            Value::String(text_only(&request.system, "Responses instructions")?),
        );
    }
    if !request.tools.is_empty() {
        root.insert("tools".to_string(), render_responses_tools(&request.tools));
        root.insert(
            "tool_choice".to_string(),
            render_responses_tool_choice(&request.tool_choice),
        );
    }
    root.insert("stream".to_string(), Value::Bool(request.stream));
    if let Some(value) = request.parallel_tool_calls {
        root.insert("parallel_tool_calls".to_string(), Value::Bool(value));
    }
    if let Some(value) = request.max_tokens {
        root.insert("max_output_tokens".to_string(), Value::Number(value.into()));
    }
    if let Some(value) = &request.temperature {
        root.insert("temperature".to_string(), value.clone());
    }
    if let Some(value) = &request.top_p {
        root.insert("top_p".to_string(), value.clone());
    }
    Ok(Value::Object(root))
}

fn render_responses_tool_call(
    id: &str,
    name: &str,
    namespace: &Option<String>,
    kind: ToolKind,
    arguments: &Value,
) -> Result<Value, TransformError> {
    let mut call = Map::new();
    call.insert(
        "type".to_string(),
        Value::String(
            match kind {
                ToolKind::Function => "function_call",
                ToolKind::Custom => "custom_tool_call",
                ToolKind::ToolSearch => "tool_search_call",
            }
            .to_string(),
        ),
    );
    call.insert("call_id".to_string(), Value::String(id.to_string()));
    call.insert("name".to_string(), Value::String(name.to_string()));
    if let Some(namespace) = namespace {
        call.insert("namespace".to_string(), Value::String(namespace.clone()));
    }
    match kind {
        ToolKind::Function => {
            call.insert(
                "arguments".to_string(),
                Value::String(
                    serde_json::to_string(arguments)
                        .map_err(|_| TransformError("无法编码工具参数".to_string()))?,
                ),
            );
        }
        ToolKind::Custom => {
            let input = arguments
                .as_str()
                .ok_or_else(|| TransformError("custom 工具输入必须是字符串".to_string()))?;
            call.insert("input".to_string(), Value::String(input.to_string()));
        }
        ToolKind::ToolSearch => {
            call.insert("execution".to_string(), Value::String("client".to_string()));
            call.insert("arguments".to_string(), arguments.clone());
        }
    }
    Ok(Value::Object(call))
}

pub(super) fn push_responses_message(
    input: &mut Vec<Value>,
    role: &str,
    ordinary: &[Part],
) -> Result<(), TransformError> {
    if ordinary.is_empty() {
        return Ok(());
    }
    input.push(json!({
        "type": "message",
        "role": role,
        "content": render_responses_content(ordinary)?,
    }));
    Ok(())
}
