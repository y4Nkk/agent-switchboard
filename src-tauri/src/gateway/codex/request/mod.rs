//! Codex-specific request compatibility, applied after conversion and before transport.
mod anthropic;
use crate::gateway::ActiveRoute;
use asb_core::contracts::{
    AppKind, CodexPromptCacheRouting, CodexRequestOptions, UpstreamProtocol,
};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use serde_json::{json, Value};
use tiny_http::Header;

pub(crate) fn prepare(
    route: &ActiveRoute,
    source: &[u8],
    wire: &mut Vec<u8>,
    incoming: Option<&[Header]>,
) -> Result<Option<String>, String> {
    crate::upstream_overrides::apply_body_override(wire, &route.connection)?;
    if route.app != AppKind::Codex || route.upstream_protocol == UpstreamProtocol::Responses {
        crate::gateway::transform::prepare_upstream_request(route, wire)?;
        return Ok(None);
    }
    let options = route.connection.codex.clone().unwrap_or_default();
    let source: Value = serde_json::from_slice(source).map_err(|_| "Codex 源请求不是 JSON")?;
    let mut body: Value =
        serde_json::from_slice(wire).map_err(|_| "Codex 转换后的请求不是 JSON")?;
    let session = source
        .pointer("/client_metadata/session_id")
        .and_then(Value::as_str)
        .or_else(|| {
            incoming
                .into_iter()
                .flatten()
                .find(|header| header.field.equiv("session_id") || header.field.equiv("thread_id"))
                .map(|header| header.value.as_str())
        });
    let (changed, beta) = apply(
        route.upstream_protocol,
        &route.upstream_base_url,
        &options,
        &source,
        &mut body,
        session,
    )?;
    if changed {
        *wire = serde_json::to_vec(&body).map_err(|_| "Codex 请求无法序列化")?;
    }
    crate::gateway::transform::prepare_upstream_request(route, wire)?;
    Ok(beta)
}
fn apply(
    protocol: UpstreamProtocol,
    endpoint: &str,
    options: &CodexRequestOptions,
    source: &Value,
    body: &mut Value,
    session: Option<&str>,
) -> Result<(bool, Option<String>), String> {
    if !body.is_object() {
        return Err("Codex 转换后的请求必须是对象".into());
    }
    match protocol {
        UpstreamProtocol::GeminiGenerateContent => Err("Gemini Native 是 Claude 专用上游".into()),
        UpstreamProtocol::ChatCompletions => {
            let changed = cache_key(
                endpoint,
                options.prompt_cache_routing,
                source,
                body,
                session,
            );
            Ok((changed, None))
        }
        UpstreamProtocol::AnthropicMessages => Ok(anthropic::rewrite(body, options)),
        UpstreamProtocol::Responses => Ok((false, None)),
    }
}
fn cache_key(
    endpoint: &str,
    mode: CodexPromptCacheRouting,
    source: &Value,
    body: &mut Value,
    session: Option<&str>,
) -> bool {
    if mode == CodexPromptCacheRouting::Disabled {
        return body
            .as_object_mut()
            .unwrap()
            .remove("prompt_cache_key")
            .is_some();
    }
    let enabled = mode == CodexPromptCacheRouting::Enabled
        || reqwest::Url::parse(endpoint).ok().is_some_and(|url| {
            url.host_str() == Some("api.openai.com")
                || (url.host_str() == Some("api.kimi.com")
                    && (url.path().trim_end_matches('/') == "/coding"
                        || url.path().starts_with("/coding/")))
        });
    if !enabled {
        return false;
    }
    let key = body
        .get("prompt_cache_key")
        .and_then(Value::as_str)
        .or_else(|| source.get("prompt_cache_key").and_then(Value::as_str))
        .or(session)
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(str::to_string);
    let Some(key) = key else {
        return false;
    };
    if body.get("prompt_cache_key") == Some(&json!(key)) {
        return false;
    }
    body["prompt_cache_key"] = json!(key);
    true
}
/// Defaults are opt-in; explicit provider headers still win at the HTTP boundary.
pub(crate) fn headers(route: &ActiveRoute, headers: &mut HeaderMap) {
    if route.app == AppKind::Codex
        && route.upstream_protocol == UpstreamProtocol::AnthropicMessages
        && route
            .connection
            .codex
            .as_ref()
            .is_some_and(|options| options.emulate_claude_code)
    {
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("claude-cli/2.1.259 (external, cli)"),
        );
        headers.insert("x-app", HeaderValue::from_static("cli"));
    }
    // The official ChatGPT backend scopes every request to the bound
    // account; the resolved value replaces whatever the client sent.
    if let Some(account_id) = route
        .codex_account
        .as_ref()
        .map(|auth| auth.account_id.as_str())
        .filter(|value| HeaderValue::from_str(value).is_ok())
    {
        headers.insert(
            HeaderName::from_static("chatgpt-account-id"),
            HeaderValue::from_str(account_id).expect("checked above"),
        );
    }
}
pub(crate) fn combine_beta(first: Option<String>, second: Option<String>) -> Option<String> {
    let mut values = Vec::new();
    for input in [first, second].into_iter().flatten() {
        for value in input
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if !values.iter().any(|existing| existing == value) {
                values.push(value.to_string());
            }
        }
    }
    (!values.is_empty()).then(|| values.join(","))
}
