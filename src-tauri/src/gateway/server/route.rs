mod response;
use super::super::ActiveRoute;
use crate::gateway::http::Request;
use asb_core::contracts::UpstreamProtocol;
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT, ACCEPT_ENCODING, AUTHORIZATION, CONTENT_TYPE,
};
pub(super) use response::StreamError;
pub(crate) use response::UpstreamResponse;
use std::time::Duration;
use tiny_http::{Header, Method};

const UPSTREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct UpstreamClient {
    pub(super) runtime: tokio::runtime::Handle,
}

impl UpstreamClient {
    /// The outbound proxy policy owns egress; the revision-cached registry
    /// rebuilds this client exactly when the saved proxy settings change.
    pub(crate) fn new(runtime: tokio::runtime::Handle) -> Self {
        Self { runtime }
    }
    pub(super) fn client(&self) -> Result<reqwest::Client, crate::outbound_proxy::OutboundProxyError> {
        crate::outbound_proxy::cached_client("gateway-upstream", |builder| {
            builder
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(UPSTREAM_CONNECT_TIMEOUT)
                .build()
                .expect("先前已验证的网关客户端配置不会失效")
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CodexOperation {
    Responses,
    Compact,
    Models,
    ChatCompletions,
    AlphaSearch,
    ImageGeneration,
    ImageEdit,
}

impl CodexOperation {
    pub(crate) fn method(self) -> Method {
        match self {
            Self::Models => Method::Get,
            Self::Responses
            | Self::Compact
            | Self::ChatCompletions
            | Self::AlphaSearch
            | Self::ImageGeneration
            | Self::ImageEdit => Method::Post,
        }
    }

    pub(crate) fn upstream_path(self) -> &'static str {
        match self {
            Self::Responses => "/responses",
            Self::Compact => "/responses/compact",
            Self::Models => "/models",
            Self::ChatCompletions => "/chat/completions",
            Self::AlphaSearch => "/alpha/search",
            Self::ImageGeneration => "/images/generations",
            Self::ImageEdit => "/images/edits",
        }
    }

    pub(crate) fn is_responses(self) -> bool {
        self == Self::Responses
    }

    pub(crate) fn is_compact(self) -> bool {
        self == Self::Compact
    }

    pub(crate) fn requires_json(self) -> bool {
        self != Self::Models
    }
}

pub(crate) struct CodexRequestPath<'a> {
    pub(crate) capability: &'a str,
    pub(crate) operation: CodexOperation,
}

pub(super) fn upstream_headers(
    route: &ActiveRoute,
    anthropic_version: Option<&str>,
    anthropic_beta: Option<&str>,
    incoming: Option<&[Header]>,
) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/event-stream"),
    );
    // Conversion and diagnostics consume response bytes locally. Asking for
    // identity avoids an avoidable decoder in compliant upstreams; the shared
    // response decoder still handles providers that send compression anyway.
    headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
    match route.authentication {
        asb_core::AuthenticationScheme::Bearer => {
            if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", route.api_key)) {
                headers.insert(AUTHORIZATION, value);
            }
            // Google OAuth callers identify the CLI client; plain API keys
            // never send this marker.
            if route.upstream_protocol == UpstreamProtocol::GeminiGenerateContent {
                headers.insert(
                    HeaderName::from_static("x-goog-api-client"),
                    HeaderValue::from_static("GeminiCLI/1.0"),
                );
            }
        }
        asb_core::AuthenticationScheme::XGoogApiKey => {
            if let Ok(value) = HeaderValue::from_str(&route.api_key) {
                headers.insert(HeaderName::from_static("x-goog-api-key"), value);
            }
        }
        asb_core::AuthenticationScheme::XApiKey => {
            if let Ok(value) = HeaderValue::from_str(&route.api_key) {
                headers.insert(HeaderName::from_static("x-api-key"), value);
            }
        }
    }
    if route.upstream_protocol == UpstreamProtocol::AnthropicMessages {
        let version = anthropic_version.unwrap_or("2023-06-01");
        if let Ok(value) = HeaderValue::from_str(version) {
            headers.insert(HeaderName::from_static("anthropic-version"), value);
        }
        if let Some(beta) = anthropic_beta.and_then(|value| HeaderValue::from_str(value).ok()) {
            headers.insert(HeaderName::from_static("anthropic-beta"), beta);
        }
    }
    if route.app == asb_core::contracts::AppKind::Codex
        && route.upstream_protocol == UpstreamProtocol::Responses
    {
        if let Some(incoming) = incoming {
            append_native_responses_headers(&mut headers, incoming);
        }
    }
    headers
}

/// The only HTTP exit to a third-party Codex upstream. It owns regenerated
/// authentication and filtered protocol headers; callers retain only their
/// request conversion and response presentation responsibilities.
/// Native Responses vendors may require Codex beta, client-version, or
/// session headers. The gateway preserves them only after replacing every
/// credential, connection, routing, and tracing header with local state.
fn append_native_responses_headers(target: &mut HeaderMap, incoming: &[Header]) {
    for header in incoming {
        let name = header.field.as_str().as_str();
        if !is_native_responses_header(name) {
            continue;
        }
        let Ok(name) = HeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        let Ok(value) = HeaderValue::from_bytes(header.value.as_bytes()) else {
            continue;
        };
        target.append(name, value);
    }
}

fn is_native_responses_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "openai-beta"
            | "originator"
            | "session_id"
            | "thread_id"
            | "turn_id"
            | "user-agent"
            | "x-client-request-id"
            | "x-codex-beta-features"
            | "x-codex-installation-id"
            | "x-codex-parent-thread-id"
            | "x-codex-routing-hint"
            | "x-codex-turn-metadata"
            | "x-codex-turn-state"
            | "x-codex-window-id"
            | "x-openai-internal-codex-responses-lite"
            | "x-openai-subagent"
            | "x-responsesapi-include-timing-metrics"
            | "x-stainless-arch"
            | "x-stainless-lang"
            | "x-stainless-os"
            | "x-stainless-package-version"
            | "x-stainless-retry-count"
            | "x-stainless-runtime"
            | "x-stainless-runtime-version"
            | "x-stainless-timeout"
    )
}

pub(crate) fn upstream_url(
    route: &ActiveRoute,
    gateway_base: &str,
    request_url: &str,
) -> Result<String, String> {
    let url = asb_core::endpoint::upstream_endpoint_with_options(
        &route.upstream_base_url,
        route.upstream_protocol,
        route.connection.is_full_url,
    )?;
    with_request_query(url, gateway_base, request_url)
}

pub(crate) fn upstream_compact_url(
    route: &ActiveRoute,
    gateway_base: &str,
    request_url: &str,
) -> Result<String, String> {
    if route.upstream_protocol != UpstreamProtocol::Responses {
        return Err("当前第三方上游不支持原生 Responses compact".to_string());
    }
    let url = codex_operation_endpoint(route, CodexOperation::Compact)?;
    with_request_query(url, gateway_base, request_url)
}

pub(crate) fn upstream_codex_operation_url(
    route: &ActiveRoute,
    gateway_base: &str,
    operation: CodexOperation,
    request_url: &str,
) -> Result<String, String> {
    if route.upstream_protocol == UpstreamProtocol::AnthropicMessages {
        return Err("所选第三方档案不支持此 Codex 操作".to_string());
    }
    let url = codex_operation_endpoint(route, operation)?;
    with_request_query(url, gateway_base, request_url)
}

fn codex_operation_endpoint(
    route: &ActiveRoute,
    operation: CodexOperation,
) -> Result<String, String> {
    if route.connection.is_full_url {
        return rewrite_codex_full_url(&route.upstream_base_url, operation.upstream_path());
    }
    asb_core::endpoint::codex_endpoint(&route.upstream_base_url, operation.upstream_path())
}

/// Derives a typed Codex sibling endpoint from a full URL configuration.
///
/// Full-URL mode is exact for the primary request, but standalone operations
/// still need their own paths. Only known API shapes are rewritten; opaque
/// full URLs fail closed instead of sending a payload to an unrelated route.
fn rewrite_codex_full_url(base_url: &str, target_path: &str) -> Result<String, String> {
    asb_core::endpoint::validate_full_url(base_url)
        .map_err(|_| "供应商上游请求地址无效".to_string())?;
    let parsed = reqwest::Url::parse(base_url).map_err(|_| "供应商上游请求地址无效".to_string())?;
    let source_path = parsed.path().trim_end_matches('/').to_ascii_lowercase();
    let suffix = codex_full_url_suffixes(target_path)
        .iter()
        .find(|suffix| source_path.ends_with(**suffix))
        .copied()
        .ok_or_else(|| {
            format!("无法从完整 Codex 地址推导 {target_path}，请填写 API 根地址或已知操作地址")
        })?;
    let without_query = base_url.split_once('?').map_or(base_url, |(url, _)| url);
    let without_query = without_query.trim_end_matches('/');
    let prefix_len = without_query
        .len()
        .checked_sub(suffix.len())
        .ok_or_else(|| "供应商上游请求地址无效".to_string())?;
    let query = base_url
        .split_once('?')
        .map(|(_, query)| query)
        .filter(|query| !query.is_empty())
        .map(|query| format!("?{query}"))
        .unwrap_or_default();
    Ok(format!(
        "{}{target_path}{query}",
        &without_query[..prefix_len]
    ))
}

fn codex_full_url_suffixes(target_path: &str) -> &'static [&'static str] {
    match target_path {
        "/responses/compact" => &["/responses/compact", "/responses"],
        "/alpha/search" => &["/responses/compact", "/responses"],
        "/images/generations" | "/images/edits" => &[
            "/images/generations",
            "/images/edits",
            "/chat/completions",
            "/responses/compact",
            "/responses",
        ],
        "/chat/completions" => &["/chat/completions", "/responses/compact", "/responses"],
        _ => &[],
    }
}

fn validate_upstream_url(url: &str, gateway_base: &str) -> Result<String, String> {
    let parsed = reqwest::Url::parse(&url).map_err(|_| "供应商上游请求地址无效".to_string())?;
    let gateway =
        reqwest::Url::parse(gateway_base).map_err(|_| "本机协议网关地址无效".to_string())?;
    if parsed.origin() == gateway.origin() {
        return Err("供应商服务地址不能指向本机协议网关自身".to_string());
    }
    Ok(url.to_string())
}

/// Copies only the request target's query onto a fixed, already-resolved
/// upstream endpoint. The path and authority always come from the route's
/// validated endpoint; an empty component, fragment, or control character is
/// rejected instead of being interpreted as a second forwarding target.
pub(super) fn with_request_query(
    url: String,
    gateway_base: &str,
    request_url: &str,
) -> Result<String, String> {
    let mut parsed = reqwest::Url::parse(&url).map_err(|_| "供应商上游请求地址无效".to_string())?;
    let Some((_, query)) = request_url.split_once('?') else {
        return validate_upstream_url(parsed.as_str(), gateway_base);
    };
    if query.is_empty()
        || query.contains('#')
        || query.chars().any(char::is_control)
        || query
            .split('&')
            .any(|part| part.split('=').next().is_none_or(str::is_empty))
    {
        return Err("Codex 操作查询参数无效".to_string());
    }
    let merged = parsed
        .query()
        .map(|existing| format!("{existing}&{query}"))
        .unwrap_or_else(|| query.to_string());
    parsed.set_query(Some(&merged));
    validate_upstream_url(parsed.as_str(), gateway_base)
}

pub(crate) fn client_protocol(path: &str) -> Option<UpstreamProtocol> {
    let path = path.split('?').next().unwrap_or(path);
    if codex_request(path).is_some() {
        return Some(UpstreamProtocol::Responses);
    }
    match path {
        "/v1/messages" | "/messages" => Some(UpstreamProtocol::AnthropicMessages),
        _ => None,
    }
}

pub(crate) fn codex_request(path: &str) -> Option<CodexRequestPath<'_>> {
    let path = path.split('?').next()?;
    let (token, operation) = path.strip_prefix("/codex/")?.split_once('/')?;
    let secret = token.strip_prefix("asb_codex_")?;
    if secret.len() != 64 || !secret.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let operation = match operation {
        "v1/responses" => CodexOperation::Responses,
        "v1/responses/compact" => CodexOperation::Compact,
        "v1/models" => CodexOperation::Models,
        "v1/chat/completions" => CodexOperation::ChatCompletions,
        "v1/alpha/search" => CodexOperation::AlphaSearch,
        "v1/images/generations" => CodexOperation::ImageGeneration,
        "v1/images/edits" => CodexOperation::ImageEdit,
        _ => return None,
    };
    Some(CodexRequestPath {
        capability: token,
        operation,
    })
}

pub(crate) fn request_capability(request: &Request, protocol: UpstreamProtocol) -> Option<String> {
    if protocol == UpstreamProtocol::Responses {
        return codex_request(request.url()).map(|path| path.capability.to_string());
    }
    let mut token = None;
    for header in request.headers() {
        let candidate = if header.field.equiv("Authorization") {
            header
                .value
                .as_str()
                .strip_prefix("Bearer ")
                .or_else(|| header.value.as_str().strip_prefix("bearer "))
        } else {
            None
        };
        if let Some(candidate) = candidate {
            if candidate.is_empty() || token.replace(candidate.to_string()).is_some() {
                return None;
            }
        }
    }
    token
}

pub(super) fn request_header(request: &Request, name: &'static str) -> Option<String> {
    let mut value = None;
    for header in request.headers() {
        if header.field.equiv(name) {
            if value.replace(header.value.as_str().to_string()).is_some() {
                return None;
            }
        }
    }
    value
}

pub(super) fn json_content_type(request: &Request) -> bool {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Content-Type"))
        .is_none_or(|header| {
            header
                .value
                .as_str()
                .to_ascii_lowercase()
                .starts_with("application/json")
        })
}

