//! Native request dialects owned by the selected upstream protocol.
//!
//! Generic cross-protocol conversion must stay provider-agnostic; these
//! modules prepare fields and validate history required by known services.
//! Gates and wiring live here so client entry points do not repeat them.

mod chat;
mod deepseek;
mod moonshot;
mod namespaces;
mod xai;

use crate::gateway::ActiveRoute;
use asb_core::contracts::{AppKind, UpstreamProtocol};
use serde_json::Value;

/// True when the resolved upstream is xAI's strict native Responses endpoint.
/// Credential delivery does not change the provider's native wire spelling.
fn is_xai_native_responses(protocol: UpstreamProtocol, base_url: &str) -> bool {
    protocol == UpstreamProtocol::Responses && host_is(base_url, &["api.x.ai"])
}

/// Applies every request-side rewrite the selected upstream requires. Called
/// after model resolution and protocol conversion, on the final body.
pub(crate) fn prepare_upstream_request(route: &ActiveRoute, body: &mut Vec<u8>) -> Result<(), String> {
    apply_request(
        route.app,
        route.upstream_protocol,
        &route.upstream_base_url,
        body,
    )
}

fn apply_request(
    app: AppKind,
    protocol: UpstreamProtocol,
    base_url: &str,
    body: &mut Vec<u8>,
) -> Result<(), String> {
    let moonshot = protocol == UpstreamProtocol::ChatCompletions
        && moonshot::upstream_requires_ref_sibling_all_of(base_url);
    let xai = app == AppKind::Codex && is_xai_native_responses(protocol, base_url);
    let endpoint = reqwest::Url::parse(base_url).ok();
    let host = endpoint.as_ref().and_then(|url| url.host_str()).unwrap_or("");
    let chat = protocol == UpstreamProtocol::ChatCompletions && chat::applies(host);
    let deepseek = host == "api.deepseek.com"
        && matches!(
            protocol,
            UpstreamProtocol::ChatCompletions | UpstreamProtocol::AnthropicMessages
        );
    if !moonshot && !xai && !chat && !deepseek {
        return Ok(());
    }
    let mut value: Value = serde_json::from_slice(body)
        .map_err(|_| "上游请求体不是有效 JSON".to_string())?;
    let mut changed = false;
    if moonshot {
        changed |= moonshot::rewrite_chat_tools(&mut value)?;
    }
    if chat {
        changed |= chat::prepare(host, &mut value)?;
    }
    if deepseek {
        deepseek::validate(protocol, &value)?;
    }
    // These carriers and their response restoration belong to Codex alone.
    if xai {
        namespaces::flatten_request_namespaces(&mut value)?;
        xai::sanitize(&mut value);
        changed = true;
    }
    if changed {
        *body = serde_json::to_vec(&value).map_err(|_| "上游请求序列化失败".to_string())?;
    }
    Ok(())
}

/// The selected upstream owns the native response spelling for both JSON and
/// complete SSE values; transport framing remains with SseTranscoder.
#[derive(Clone, Copy, Default)]
pub(crate) enum NativeResponsesDialect {
    #[default]
    Standard,
    Xai,
}

impl NativeResponsesDialect {
    pub(crate) fn for_route(route: &ActiveRoute) -> Self {
        if route.app == asb_core::contracts::AppKind::Codex
            && is_xai_native_responses(route.upstream_protocol, &route.upstream_base_url)
        {
            Self::Xai
        } else {
            Self::Standard
        }
    }

    pub(crate) fn restore(self, value: &mut Value) -> bool {
        match self {
            Self::Standard => false,
            Self::Xai => {
                let mut changed = namespaces::restore_response_namespaces(value);
                changed |= xai::normalize_integer_arguments(value);
                changed
            }
        }
    }

    pub(crate) fn restore_json(self, body: &mut Vec<u8>) {
        if matches!(self, Self::Standard) {
            return;
        }
        let Ok(mut value) = serde_json::from_slice(body) else {
            return;
        };
        if self.restore(&mut value) {
            *body = serde_json::to_vec(&value).expect("restored JSON value is serializable");
        }
    }
}

fn host_is(base_url: &str, hosts: &[&str]) -> bool {
    let Ok(url) = reqwest::Url::parse(base_url.trim()) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    hosts.iter().any(|candidate| host == *candidate)
}

#[cfg(test)]
mod tests;
