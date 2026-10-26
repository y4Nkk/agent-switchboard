//! Codex `namespace` tool flattening for the native Responses passthrough.
//!
//! Codex declares plugin/MCP tools as `{"type":"namespace","name":…,"tools":[…]}`
//! which the OpenAI backend understands but strict third-party gateways reject
//! with `422 unknown variant "namespace"`. The Chat/Anthropic conversions
//! already unwrap these; the native passthrough forwards them verbatim, so this
//! module flattens them on the way out and restores them on the way back.
//!
//! Flat names reuse [`super::super::tool_names`] reversible envelopes, so the
//! response-side restore parses names instead of threading a request-derived
//! map through the connection.

use super::super::tool_names::{parse_target_name, render_target_name};
use super::super::{ToolKind, TransformError};
use asb_core::contracts::UpstreamProtocol;
use serde_json::{json, Value};

fn flat_name(namespace: &str, name: &str) -> Result<String, TransformError> {
    render_target_name(
        UpstreamProtocol::Responses,
        Some(namespace),
        name,
        ToolKind::Function,
    )
}

/// Lifts namespace children to top-level function tools, rewrites
/// namespace-qualified `function_call` history items and degrades a namespace
/// `tool_choice`. Returns an error on flat-name collisions, which the upstream
/// could never disambiguate.
pub(super) fn flatten_request_namespaces(body: &mut Value) -> Result<(), String> {
    let Some(tools) = body.get("tools").and_then(Value::as_array) else {
        return Ok(());
    };
    if !tools
        .iter()
        .any(|tool| tool.get("type").and_then(Value::as_str) == Some("namespace"))
    {
        return Ok(());
    }
    let mut top_level = std::collections::HashSet::new();
    for tool in tools {
        let kind = tool.get("type").and_then(Value::as_str).unwrap_or("");
        if matches!(kind, "function" | "custom") {
            if let Some(name) = tool.get("name").and_then(Value::as_str).map(str::trim) {
                if !name.is_empty() {
                    top_level.insert(name.to_string());
                }
            }
        }
    }
    let tools = tools.clone();
    let mut flattened = Vec::with_capacity(tools.len());
    let mut seen = std::collections::HashSet::new();
    for tool in &tools {
        if tool.get("type").and_then(Value::as_str) != Some("namespace") {
            flattened.push(tool.clone());
            continue;
        }
        let Some(namespace) = tool
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|namespace| !namespace.is_empty())
        else {
            continue;
        };
        for child in namespace_children(tool) {
            if child.get("type").and_then(Value::as_str) != Some("function") {
                continue;
            }
            let Some(name) = child
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
            else {
                continue;
            };
            let flat = flat_name(namespace, name).map_err(|error| error.to_string())?;
            if top_level.contains(&flat) || !seen.insert(flat.clone()) {
                return Err(format!(
                    "namespace 工具 {namespace}/{name} 扁平化为 {flat} 后与顶层工具同名，无法区分"
                ));
            }
            let mut lifted = child.clone();
            if let Some(object) = lifted.as_object_mut() {
                object.insert("name".to_string(), json!(flat));
            }
            flattened.push(lifted);
        }
    }
    if let Some(object) = body.as_object_mut() {
        object.insert("tools".to_string(), Value::Array(flattened));
    }
    if let Some(input) = body.get_mut("input") {
        rewrite_namespaced_calls(input);
    }
    if let Some(choice) = body.get_mut("tool_choice") {
        if choice.get("type").and_then(Value::as_str) == Some("namespace") {
            *choice = json!("auto");
        } else {
            rewrite_namespaced_call(choice);
        }
    }
    Ok(())
}

/// Restores flattened `function_call` names to `{name, namespace}` in a full
/// Responses payload. Only names emitted by our envelope are touched.
pub(super) fn restore_response_namespaces(value: &mut Value) -> bool {
    restore_value(value)
}

fn restore_value(value: &mut Value) -> bool {
    let mut changed = false;
    match value {
        Value::Array(items) => {
            for item in items {
                changed |= restore_value(item);
            }
        }
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("function_call") {
                if let Some(name) = object.get("name").and_then(Value::as_str) {
                    if name.starts_with("asbns_") {
                        if let Ok((namespace, bare, ToolKind::Function)) = parse_target_name(name) {
                            object.insert("name".to_string(), json!(bare));
                            if let Some(namespace) = namespace {
                                object.insert("namespace".to_string(), json!(namespace));
                            }
                            changed = true;
                        }
                    }
                }
            }
            for child in object.values_mut() {
                changed |= restore_value(child);
            }
        }
        _ => {}
    }
    changed
}

fn namespace_children(tool: &Value) -> Vec<Value> {
    tool.get("tools")
        .or_else(|| tool.get("children"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn rewrite_namespaced_calls(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                rewrite_namespaced_calls(item);
            }
        }
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("function_call") {
                rewrite_namespaced_call(value);
                return;
            }
            for child in object.values_mut() {
                rewrite_namespaced_calls(child);
            }
        }
        _ => {}
    }
}

fn rewrite_namespaced_call(item: &mut Value) -> bool {
    let Some(object) = item.as_object_mut() else {
        return false;
    };
    let namespace = object
        .get("namespace")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    let name = object
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    if namespace.is_empty() || name.is_empty() {
        return false;
    }
    match flat_name(&namespace, &name) {
        Ok(flat) => {
            object.insert("name".to_string(), json!(flat));
            object.remove("namespace");
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request() -> Value {
        json!({
            "model": "grok-code-fast-1",
            "tools": [
                { "type": "function", "name": "plain_tool", "parameters": {} },
                { "type": "namespace", "name": "mcp__files__", "tools": [
                    { "type": "function", "name": "read", "description": "read a file", "parameters": {} },
                    { "type": "function", "name": "write", "parameters": {} }
                ]}
            ],
            "input": [
                { "type": "function_call", "name": "read", "namespace": "mcp__files__",
                  "call_id": "c1", "arguments": "{}" }
            ],
            "tool_choice": { "type": "namespace", "name": "mcp__files__" }
        })
    }

    #[test]
    fn flatten_lifts_children_rewrites_history_and_degrades_choice() {
        let mut body = request();
        flatten_request_namespaces(&mut body).unwrap();
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 3);
        assert!(tools.iter().all(|tool| tool["type"] == "function"));
        let flat_read = flat_name("mcp__files__", "read").unwrap();
        let lifted = tools.iter().find(|t| t["name"] == flat_read).unwrap();
        assert_eq!(lifted["description"], "read a file");
        assert_eq!(body["input"][0]["name"], json!(flat_read));
        assert!(body["input"][0].get("namespace").is_none());
        assert_eq!(body["tool_choice"], json!("auto"));
    }

    #[test]
    fn flatten_rejects_collisions_with_top_level_tools() {
        let flat = flat_name("ns", "collide").unwrap();
        let mut body = json!({
            "tools": [
                { "type": "function", "name": flat, "parameters": {} },
                { "type": "namespace", "name": "ns", "tools": [
                    { "type": "function", "name": "collide", "parameters": {} }
                ]}
            ]
        });
        assert!(flatten_request_namespaces(&mut body).is_err());
    }

    #[test]
    fn flatten_is_a_noop_without_namespaces() {
        let mut body =
            json!({ "tools": [{ "type": "function", "name": "plain", "parameters": {} }] });
        flatten_request_namespaces(&mut body).unwrap();
        assert_eq!(body["tools"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn restore_inverts_flatten_for_function_calls() {
        let mut body = request();
        flatten_request_namespaces(&mut body).unwrap();
        let flat = body["input"][0]["name"].as_str().unwrap().to_string();
        let mut response = json!({
            "output": [
                { "type": "function_call", "name": flat, "call_id": "c1", "arguments": "{}" },
                { "type": "function_call", "name": "unrelated", "call_id": "c2", "arguments": "{}" }
            ]
        });
        assert!(restore_response_namespaces(&mut response));
        assert_eq!(response["output"][0]["name"], json!("read"));
        assert_eq!(response["output"][0]["namespace"], json!("mcp__files__"));
        assert_eq!(response["output"][1]["name"], json!("unrelated"));
    }

}
