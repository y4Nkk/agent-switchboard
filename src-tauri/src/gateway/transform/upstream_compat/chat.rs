//! Wire fields required by the selected native Chat service.

use serde_json::{Map, Value};

pub(super) fn applies(host: &str) -> bool {
    completion_limit(host) || separate_reasoning(host)
}

fn completion_limit(host: &str) -> bool {
    ["openai.azure.com", "openai.azure.us", "openai.azure.cn"]
        .iter()
        .any(|suffix| {
            host.strip_suffix(suffix)
                .is_some_and(|prefix| prefix.ends_with('.'))
        })
        || (host.starts_with("bedrock-runtime.")
            && (host.ends_with(".amazonaws.com") || host.ends_with(".amazonaws.com.cn")))
}

fn separate_reasoning(host: &str) -> bool {
    matches!(host, "api.minimax.io" | "api.minimaxi.com")
}

pub(super) fn prepare(host: &str, body: &mut Value) -> Result<bool, String> {
    let root = body.as_object_mut().ok_or("Chat 请求必须是 JSON 对象")?;
    let mut changed = false;
    if completion_limit(host) {
        changed |= rename_limit(root)?;
    }
    if separate_reasoning(host) {
        match root.get("reasoning_split") {
            Some(Value::Bool(true)) => {}
            None => {
                root.insert("reasoning_split".into(), Value::Bool(true));
                changed = true;
            }
            _ => return Err("MiniMax Chat 路由要求 reasoning_split=true，以保留独立推理字段；不能使用标签式推理输出".into()),
        }
    }
    Ok(changed)
}

fn rename_limit(root: &mut Map<String, Value>) -> Result<bool, String> {
    let Some(limit) = root.remove("max_tokens") else {
        return Ok(false);
    };
    if !limit.is_u64() {
        return Err("Chat 最大输出 token 数必须是非负整数".into());
    }
    if let Some(current) = root.get("max_completion_tokens") {
        if current != &limit {
            return Err("max_tokens 与 max_completion_tokens 冲突，不能猜测输出上限".into());
        }
    } else {
        root.insert("max_completion_tokens".into(), limit);
    }
    Ok(true)
}
