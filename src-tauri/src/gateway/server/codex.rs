//! Codex-only request admission and model-catalog projection.

use super::CodexOperation;
use crate::gateway::{metrics::RequestSpan, ActiveRoute};
use asb_core::contracts::{
    codex_model_catalog_document, CodexCatalogEntry,
    CodexRouteSnapshot, UpstreamProtocol,
};
use serde_json::{Map, Value};

pub(super) fn ensure_operation(
    route: &ActiveRoute,
    operation: CodexOperation,
) -> Result<(), String> {
    let snapshot = route
        .codex
        .as_ref()
        .ok_or_else(|| "Codex 路由缺少专用能力快照".to_string())?;
    if route.upstream_protocol == UpstreamProtocol::AnthropicMessages
        && !matches!(
            operation,
            CodexOperation::Responses | CodexOperation::Compact | CodexOperation::Models
        )
    {
        return Err("所选第三方档案不支持此 Codex 操作".to_string());
    }
    // `/v1/models` is served from the activated, validated local catalog. It
    // must remain reachable even when the selected upstream has no native
    // models operation; `capabilities.models` describes the upstream API, not
    // this gateway-owned projection.
    if operation == CodexOperation::Models
        || snapshot
            .capabilities
            .supports_operation(operation)
    {
        Ok(())
    } else {
        Err("所选 Codex 供应商未声明此操作能力".to_string())
    }
}

pub(super) fn respond_models(
    request: crate::gateway::http::Request,
    span: RequestSpan,
    route: &ActiveRoute,
    inner: &super::GatewayInner,
) {
    // An official-takeover route has no ASB catalog: the backend document is
    // the model list, fetched with the request's freshly resolved account.
    if route.codex_account.is_some() {
        match super::codex_account::official_models_document(inner, route) {
            Ok(document) => {
                span.finish(Some(200), document.len() as u64);
                super::respond::respond_bytes(
                    request,
                    200,
                    "application/json; charset=utf-8",
                    document.into_bytes(),
                );
            }
            Err((status, message)) => {
                span.finish(Some(status), 0);
                super::respond_error(request, Some(UpstreamProtocol::Responses), status, &message);
            }
        }
        return;
    }
    let snapshot = route
        .codex
        .as_ref()
        .expect("operation admission requires a Codex route snapshot");
    let bytes = serde_json::to_vec(&codex_model_catalog_document(&snapshot.catalog))
        .expect("Codex model catalog is serializable");
    span.finish(Some(200), bytes.len() as u64);
    super::respond::respond_bytes(request, 200, "application/json; charset=utf-8", bytes);
}

pub(super) fn resolve_model_and_validate(
    route: &ActiveRoute,
    operation: CodexOperation,
    body: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let snapshot = route
        .codex
        .as_ref()
        .ok_or_else(|| "Codex 路由缺少专用模型快照".to_string())?;
    // Official takeover: the ChatGPT backend owns model identity and request
    // capabilities, so admission keeps the client's body verbatim instead of
    // gating it against an ASB catalog that can never list official models.
    if route.codex_account.is_some() {
        return official_passthrough(operation, body);
    }
    let mut value: Value =
        serde_json::from_slice(&body).map_err(|_| "Codex 请求体不是有效 JSON".to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "Codex 请求体必须是 JSON 对象".to_string())?;
    let requested = match object.get("model") {
        Some(Value::String(model)) => model.clone(),
        Some(_) => return Err("Codex 请求模型必须是字符串".to_string()),
        None if requires_model(operation) => snapshot.default_model.clone(),
        None => return serde_json::to_vec(&value).map_err(|_| "Codex 请求序列化失败".to_string()),
    };
    let upstream = snapshot.resolve_model(&requested)?;
    validate_request_capabilities(
        snapshot,
        route.upstream_protocol,
        operation,
        &requested,
        &value,
    )?;
    let object = value
        .as_object_mut()
        .expect("validated Codex request remains an object");
    if route.upstream_protocol != UpstreamProtocol::Responses
        && matches!(
            operation,
            CodexOperation::Responses | CodexOperation::Compact
        )
    {
        // HTTP and WS share this admission path. Resolve the client budget
        // before rewriting its model ID, which may alias another catalog entry.
        // The field is an internal bridge contract and is consumed by both
        // normal Responses conversion and the compaction summarizer.
        apply_anthropic_output_budget(object, catalog_entry(snapshot, &requested)?)?;
    }
    super::codex_reasoning::apply(snapshot, catalog_entry(snapshot, &requested)?, object)?;
    object.insert("model".to_string(), Value::String(upstream));
    serde_json::to_vec(&value).map_err(|_| "Codex 请求序列化失败".to_string())
}

fn apply_anthropic_output_budget(
    request: &mut Map<String, Value>,
    model: &CodexCatalogEntry,
) -> Result<(), String> {
    let Some(value) = request.get("max_output_tokens") else {
        request.insert(
            "max_output_tokens".to_string(),
            Value::from(model.max_output_tokens),
        );
        return Ok(());
    };
    let budget = value
        .as_u64()
        .filter(|budget| *budget > 0)
        .ok_or_else(|| "Codex 请求 max_output_tokens 必须是正整数".to_string())?;
    if budget > model.max_output_tokens {
        return Err(format!(
            "请求的 max_output_tokens 超出 Codex 模型 {} 的输出上限 {}",
            model.id, model.max_output_tokens
        ));
    }
    Ok(())
}

fn requires_model(operation: CodexOperation) -> bool {
    matches!(
        operation,
        CodexOperation::Responses
            | CodexOperation::ChatCompletions
            | CodexOperation::Compact
            | CodexOperation::ImageGeneration
            | CodexOperation::ImageEdit
    )
}

fn validate_request_capabilities(
    snapshot: &CodexRouteSnapshot,
    upstream_protocol: UpstreamProtocol,
    operation: CodexOperation,
    requested_model: &str,
    value: &Value,
) -> Result<(), String> {
    let model = catalog_entry(snapshot, requested_model)?;
    if value.get("reasoning").is_some() && (!snapshot.capabilities.reasoning || !model.reasoning) {
        return Err("所选 Codex 供应商未声明推理能力".to_string());
    }
    if (contains_image(value)
        || matches!(
            operation,
            CodexOperation::ImageGeneration | CodexOperation::ImageEdit
        ))
        && !model.images
    {
        return Err("所选 Codex 模型未声明图像能力".to_string());
    }
    if (operation == CodexOperation::Compact
        || (operation == CodexOperation::Responses && contains_compaction_trigger(value)))
        && (!snapshot.capabilities.compact || !model.compact)
    {
        return Err("所选 Codex 模型未声明原生压缩能力".to_string());
    }
    let Some(tools) = value.get("tools").and_then(Value::as_array) else {
        return Ok(());
    };
    for tool in tools {
        let kind = tool.get("type").and_then(Value::as_str).unwrap_or_default();
        let allowed = match kind {
            // Responses groups ordinary functions under namespace containers.
            // It is a wire representation of function tools, not a separate
            // provider capability.
            "function" | "namespace" => {
                snapshot.capabilities.function_tools && model.function_tools
            }
            "custom" => snapshot.capabilities.custom_tools && model.custom_tools,
            // Server-side Responses tools cannot survive cross-protocol
            // conversion; reject at admission with the same rule the
            // converter enforces, instead of letting it 422 later.
            "web_search_preview" | "web_search" | "file_search"
                if upstream_protocol != UpstreamProtocol::Responses =>
            {
                return Err(format!(
                    "{kind} 是 Responses 服务端工具，无法由 {protocol} 上游无损承载；请使用 Responses 上游",
                    protocol = upstream_protocol.label()
                ));
            }
            "web_search_preview" | "web_search" | "file_search" | "tool_search" => {
                snapshot.capabilities.tool_search && model.tool_search
            }
            _ => false,
        };
        if !allowed {
            return Err(format!("所选 Codex 供应商未声明工具能力：{kind}"));
        }
    }
    Ok(())
}

fn catalog_entry<'a>(
    snapshot: &'a CodexRouteSnapshot,
    requested_model: &str,
) -> Result<&'a CodexCatalogEntry, String> {
    snapshot
        .catalog
        .iter()
        .find(|entry| entry.id == requested_model)
        .ok_or_else(|| format!("请求模型不在已激活 Codex 目录中：{requested_model}"))
}

/// Official-takeover admission: JSON-shape checks only, no catalog rewrite.
fn official_passthrough(operation: CodexOperation, body: Vec<u8>) -> Result<Vec<u8>, String> {
    let value: Value = serde_json::from_slice(&body).map_err(|_| "Codex 请求体不是有效 JSON")?;
    let object = value
        .as_object()
        .ok_or_else(|| "Codex 请求体必须是 JSON 对象".to_string())?;
    match object.get("model") {
        Some(Value::String(model)) if !model.trim().is_empty() => {}
        Some(_) => return Err("Codex 请求模型必须是字符串".to_string()),
        None if requires_model(operation) => {
            return Err("官方 Codex 请求缺少 model 字段".to_string())
        }
        None => {}
    }
    Ok(body)
}

fn contains_image(value: &Value) -> bool {
    match value {
        Value::Object(object) => {
            object.get("type").and_then(Value::as_str) == Some("input_image")
                || object.values().any(contains_image)
        }
        Value::Array(items) => items.iter().any(contains_image),
        _ => false,
    }
}

fn contains_compaction_trigger(value: &Value) -> bool {
    value
        .get("input")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| item.get("type").and_then(Value::as_str) == Some("compaction_trigger"))
        })
}
