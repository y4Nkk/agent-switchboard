use super::contracts::{
    timestamp, ProviderRequestOutcome as Outcome, ProviderRequestResult, MAX_OUTPUT_TOKENS,
    REQUEST_PROMPT,
};
use super::response;
use crate::commands::error::CommandError;
use crate::provider_diagnostics::{
    http_diagnostic, network_diagnostic, ProviderDiagnostic, ProviderFailureKind,
    MAX_DIAGNOSTIC_BODY_BYTES,
};
use asb_core::contracts::{ResponsesRequestMode, UpstreamProtocol};
use asb_core::AuthenticationScheme;
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde_json::{json, Value};

use std::time::{Duration, Instant};

pub(super) const MAX_RESPONSE_BYTES: usize = 64 * 1_024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);

pub(super) fn client() -> Result<reqwest::Client, CommandError> {
    crate::outbound_proxy::cached_client(
        "provider-request",
        |builder| {
            client_builder(builder)
                .build()
                .expect("先前已验证的请求客户端配置不会失效")
        },
    ).map_err(crate::commands::outbound_proxy_settings::settings_error)
}

fn client_builder(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    builder
        .user_agent("Agent Switchboard")
        .connect_timeout(Duration::from_secs(5))
        .timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
}
#[cfg(test)]
#[allow(dead_code)] // request harness
fn fresh_client_builder() -> reqwest::ClientBuilder {
    client_builder(reqwest::Client::builder())
}

pub(super) async fn send(
    client: reqwest::Client,
    profile: super::ProviderRequestConnection,
    endpoint: String,
    model: String,
    started: Instant,
) -> Result<ProviderRequestResult, CommandError> {
    let protocol = profile.upstream_protocol;
    let request = build_request(&client, &endpoint, &profile, protocol, &model)?;
    let response = match client.execute(request).await {
        Ok(response) => response,
        Err(error) => {
            return Ok(ProviderRequestResult::diagnosed(
                network_diagnostic(&endpoint, &error),
                started,
            ))
        }
    };
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let limit = if (200..300).contains(&status) {
        MAX_RESPONSE_BYTES
    } else {
        MAX_DIAGNOSTIC_BODY_BYTES
    };
    let body = read_body(response, limit).await;
    let mut diagnostic = http_diagnostic(
        &endpoint,
        status,
        &headers,
        &body.bytes,
        body.truncated,
        &[&profile.api_key],
    );
    if !(200..300).contains(&status) {
        if body.error.is_some() {
            diagnostic.message.push_str("；读取上游错误响应时连接中断");
            diagnostic.body_truncated = true;
        }
        return Ok(ProviderRequestResult::diagnosed(diagnostic, started));
    }
    if let Some(error) = body.error {
        let failure = network_diagnostic(&endpoint, &error);
        diagnostic.kind = failure.kind;
        diagnostic.message = failure.message;
        return Ok(ProviderRequestResult::diagnosed(diagnostic, started));
    }
    diagnostic.kind = ProviderFailureKind::ResponseParse;
    if body.truncated {
        diagnostic.message =
            "供应商响应超过 64 KiB，已停止读取；请检查服务地址是否为模型 API".into();
        return Ok(ProviderRequestResult::diagnosed(diagnostic, started));
    }
    let bytes = match super::claude::response_body(&profile, body.bytes) {
        Ok(bytes) => bytes,
        Err(message) => {
            diagnostic.message = message;
            return Ok(ProviderRequestResult::diagnosed(diagnostic, started));
        }
    };
    Ok(parse_reply(
        protocol,
        &bytes,
        &profile.api_key,
        status,
        started,
        diagnostic,
    ))
}

fn parse_reply(
    protocol: UpstreamProtocol,
    body: &[u8],
    api_key: &str,
    status: u16,
    started: Instant,
    mut diagnostic: ProviderDiagnostic,
) -> ProviderRequestResult {
    match response::parse(protocol, body) {
        Ok(reply) => ProviderRequestResult {
            outcome: Outcome::Success,
            status: Some(status),
            latency_ms: started.elapsed().as_millis() as u64,
            model: reply.model.map(|model| response::redact(model, api_key)),
            reply: Some(response::redact(reply.text, api_key)),
            error: None,
            diagnostic: None,
            at: timestamp(),
        },
        Err(message) => {
            diagnostic.message = crate::provider_diagnostics::redact_text(&message, &[api_key]);
            ProviderRequestResult::diagnosed(diagnostic, started)
        }
    }
}

fn build_request(
    client: &reqwest::Client,
    endpoint: &str,
    profile: &super::ProviderRequestConnection,
    protocol: UpstreamProtocol,
    model: &str,
) -> Result<reqwest::Request, CommandError> {
    let mut body = payload(
        protocol,
        model,
        profile
            .responses_options
            .map(|options| options.request_mode),
    )
    .to_string()
    .into_bytes();
    crate::upstream_overrides::apply_body_override(&mut body, &profile.connection)
        .map_err(|_| {
            CommandError::keyed(
                "provider-request-invalid",
                "errors.misc.requestBodyOverrideInvalid",
                "请求 body 覆盖无效",
            )
        })?;
    if let Some(account) = &profile.claude_account {
        crate::claude_auth::request::body(account, protocol, &mut body)
            .map_err(|message| CommandError::new("provider-request-invalid", message))?;
    }
    let mut request = client
        .post(endpoint)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json")
        .body(body);
    request = match protocol.resolve_authentication(profile.authentication) {
        AuthenticationScheme::Bearer => {
            // Google OAuth callers identify the CLI client; plain API keys
            // never send this marker.
            let request = if protocol == UpstreamProtocol::GeminiGenerateContent {
                request.header("x-goog-api-client", "GeminiCLI/1.0")
            } else {
                request
            };
            request.bearer_auth(&profile.api_key)
        }
        AuthenticationScheme::XApiKey => request.header("x-api-key", &profile.api_key),
        AuthenticationScheme::XGoogApiKey => request.header("x-goog-api-key", &profile.api_key),
    };
    if protocol == UpstreamProtocol::AnthropicMessages {
        request = request.header("anthropic-version", "2023-06-01");
    }
    let mut request = request.build().map_err(|_| {
        CommandError::keyed(
            "provider-request-invalid",
            "errors.misc.requestBuildFailed",
            "无法构造请求，请检查供应商服务地址与 API 密钥格式",
        )
    })?;
    if let Some(account) = &profile.claude_account {
        let body = request
            .body()
            .and_then(reqwest::Body::as_bytes)
            .unwrap_or_default()
            .to_vec();
        crate::claude_auth::request::request_headers(account, request.headers_mut(), &body);
    }
    crate::upstream_overrides::apply_header_overrides(request.headers_mut(), &profile.connection);
    Ok(request)
}

fn payload(protocol: UpstreamProtocol, model: &str, mode: Option<ResponsesRequestMode>) -> Value {
    let mut body = match protocol {
        UpstreamProtocol::Responses => json!({
            "model": model, "stream": false, "store": false,
            "max_output_tokens": MAX_OUTPUT_TOKENS,
            "input": [{"role": "user", "content": [{"type": "input_text", "text": REQUEST_PROMPT}]}],
        }),
        UpstreamProtocol::ChatCompletions => json!({
            "model": model, "stream": false, "max_completion_tokens": MAX_OUTPUT_TOKENS,
            "messages": [{"role": "user", "content": REQUEST_PROMPT}],
        }),
        UpstreamProtocol::GeminiGenerateContent => {
            super::claude::gemini_test_payload(REQUEST_PROMPT, MAX_OUTPUT_TOKENS)
        }
        UpstreamProtocol::AnthropicMessages => json!({
            "model": model, "stream": false, "max_tokens": MAX_OUTPUT_TOKENS,
            "messages": [{"role": "user", "content": REQUEST_PROMPT}],
        }),
    };
    if mode == Some(ResponsesRequestMode::Minimal) {
        body.as_object_mut()
            .expect("request payload is an object")
            .remove("store");
    }
    body
}

struct BodyRead {
    bytes: Vec<u8>,
    truncated: bool,
    error: Option<reqwest::Error>,
}

async fn read_body(mut response: reqwest::Response, limit: usize) -> BodyRead {
    let mut body = BodyRead {
        bytes: Vec::new(),
        truncated: false,
        error: None,
    };
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                let remaining = limit.saturating_add(1).saturating_sub(body.bytes.len());
                body.bytes
                    .extend_from_slice(&chunk[..remaining.min(chunk.len())]);
                if body.bytes.len() > limit {
                    body.truncated = true;
                    break;
                }
            }
            Ok(None) => break,
            Err(error) => {
                body.error = Some(error);
                body.truncated = true;
                break;
            }
        }
    }
    body
}



