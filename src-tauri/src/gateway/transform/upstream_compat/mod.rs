//! Targeted compatibility rewrites for strict third-party Codex upstreams.
//!
//! Generic cross-protocol conversion must stay provider-agnostic; these
//! modules only fire for upstreams that verifiably reject otherwise-valid
//! Codex traffic. Gates and wiring live here so no other layer knows about a
//! specific vendor.

mod moonshot;
mod namespaces;
mod xai;

use crate::gateway::ActiveRoute;
use asb_core::contracts::UpstreamProtocol;
use serde_json::Value;

/// True when the resolved upstream is xAI's strict native Responses endpoint.
/// ASB admission already guarantees catalog models, so the API-key form is the
/// complete gate until managed xAI accounts exist.
fn is_xai_native_responses(protocol: UpstreamProtocol, base_url: &str) -> bool {
    protocol == UpstreamProtocol::Responses && host_is(base_url, &["api.x.ai"])
}

/// Applies every request-side rewrite the selected upstream requires. Called
/// after model resolution and protocol conversion, on the final body.
pub(crate) fn apply_request_compat(route: &ActiveRoute, body: &mut Vec<u8>) -> Result<(), String> {
    apply_request(route.upstream_protocol, &route.upstream_base_url, body)
}

fn apply_request(
    protocol: UpstreamProtocol,
    base_url: &str,
    body: &mut Vec<u8>,
) -> Result<(), String> {
    if protocol == UpstreamProtocol::ChatCompletions
        && moonshot::upstream_requires_ref_sibling_all_of(base_url)
    {
        moonshot::wrap_ref_siblings_in_chat_tools(body)?;
    }
    if is_xai_native_responses(protocol, base_url) {
        let mut value: serde_json::Value =
            serde_json::from_slice(body).map_err(|_| "Codex 请求体不是有效 JSON".to_string())?;
        namespaces::flatten_request_namespaces(&mut value)?;
        xai::sanitize(&mut value);
        *body = serde_json::to_vec(&value).map_err(|_| "Codex 请求序列化失败".to_string())?;
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
