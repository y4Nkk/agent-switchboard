//! One Claude upstream attempt, before any bytes are published to the client.

use super::*;
use crate::gateway::claude_settings::ClaudeTrafficSettings;
use crate::gateway::server::transport::{send_with_timeouts, Timeouts};
use crate::gateway::transform;
use std::time::Duration;

pub(super) struct Failure {
    pub(super) diagnostic: ProviderDiagnostic,
    pub(super) status: u16,
    pub(super) retryable: bool,
    pub(super) penalize: bool,
}

impl Failure {
    fn preparation(url: &str, message: &str, status: u16) -> Self {
        Self {
            diagnostic: ProviderDiagnostic::new(
                ProviderFailureKind::RequestParameters,
                url,
                message,
            ),
            status,
            retryable: status != 413,
            penalize: false,
        }
    }
    fn upstream(diagnostic: ProviderDiagnostic) -> Self {
        let status = diagnostic.status.filter(|s| *s >= 400).unwrap_or(502);
        let retryable = crate::gateway::is_retryable_http_status(status);
        Self {
            diagnostic,
            status,
            retryable,
            penalize: retryable,
        }
    }
}

pub(super) fn send(
    request: &Request,
    span: &mut RequestSpan,
    candidate: &ActiveRoute,
    inner: &GatewayInner,
    client: &UpstreamClient,
    body: &[u8],
    policy: &ClaudeTrafficSettings,
) -> Result<(super::response::Prepared, ActiveRoute), Failure> {
    if let Some(message) = inner
        .claude_request_ledger
        .budget_exceeded(
            &candidate.profile_id,
            candidate.connection.claude_billing.as_ref(),
        )
        .map_err(|error| Failure::preparation(&candidate.upstream_base_url, &error, 503))?
    {
        let mut failure = Failure::preparation(&candidate.upstream_base_url, &message, 429);
        failure.retryable = true;
        return Err(failure);
    }
    let effective = super::accounts::resolve(candidate, inner).map_err(|message| Failure {
        diagnostic: ProviderDiagnostic::new(
            ProviderFailureKind::Authentication,
            &candidate.upstream_base_url,
            &message,
        ),
        status: 401,
        retryable: false,
        penalize: false,
    })?;
    if request.is_cancelled() {
        return Err(Failure::preparation(&candidate.upstream_base_url, "客户端已取消请求", 499));
    }
    let candidate = &effective;
    span.protect_secrets(&[&candidate.api_key]);
    let (converted, model) = prepare(candidate, span, body, &candidate.upstream_base_url)?;
    let url = super::query::request_url(
        candidate,
        &inner.configured_base_url(),
        request.url(),
        model.as_deref(),
        converted.stream,
    )
    .map_err(|message| Failure::preparation(&candidate.upstream_base_url, &message, 502))?;
    let stream = converted.stream;
    let timeouts = timeouts(policy, stream);
    let cancelled = || request.is_cancelled();
    let upstream = send_with_timeouts(
        client,
        candidate,
        &url,
        reqwest::Method::POST,
        converted.body,
        Some(request.headers()),
        request_header(request, "anthropic-version").as_deref(),
        request_header(request, "anthropic-beta").as_deref(),
        timeouts,
        Some(&cancelled),
    )
    .map_err(Failure::upstream)?;
    if upstream.initial_body_received() {
        span.note_first_byte();
    }
    if !upstream.status().is_success() {
        return Err(Failure::upstream(read_upstream_diagnostic(
            upstream,
            &[&candidate.api_key, &candidate.client_token],
        )));
    }
    let upstream = if stream {
        upstream
    } else {
        super::accounts::non_streaming(upstream, candidate).map_err(Failure::upstream)?
    };
    let reply =
        super::response::Prepared::new(upstream, candidate, stream).map_err(Failure::upstream)?;
    Ok((reply, effective))
}

fn prepare(
    candidate: &ActiveRoute,
    span: &mut RequestSpan,
    body: &[u8],
    url: &str,
) -> Result<(ConvertedRequest, Option<String>), Failure> {
    let mapped = transform::apply_claude_model_mapping(
        body,
        candidate.claude_primary_model.as_deref(),
        candidate.claude_model_options.as_ref(),
    )
    .map_err(|error| {
        Failure::preparation(url, &format!("无法准备 Claude 模型路由：{error}"), 422)
    })?;
    let model = model_from_value(
        UpstreamProtocol::AnthropicMessages,
        &serde_json::from_slice(&mapped).unwrap_or_default(),
    );
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&mapped) {
        span.note_mapped_model(model_from_value(
            UpstreamProtocol::AnthropicMessages,
            &value,
        ));
    }
    let transport = ReasoningTransport::from_continuation_key(candidate.continuation_key);
    let mut converted = convert_request(
        UpstreamProtocol::AnthropicMessages,
        candidate.upstream_protocol,
        &mapped,
        candidate.max_output_tokens,
        Some(&transport),
        None,
    )
    .and_then(|converted| transform::minimal::apply(converted, candidate.responses_options))
    .map_err(|error| Failure::preparation(url, &format!("无法转换 Claude 请求：{error}"), 422))?;
    if let Some(key) = &candidate.connection.claude_prompt_cache_key {
        if matches!(
            candidate.upstream_protocol,
            UpstreamProtocol::Responses | UpstreamProtocol::ChatCompletions
        ) {
            let mut value: serde_json::Value = serde_json::from_slice(&converted.body)
                .map_err(|_| Failure::preparation(url, "上游请求编码无效", 422))?;
            value["prompt_cache_key"] = serde_json::json!(key);
            converted.body = serde_json::to_vec(&value)
                .map_err(|_| Failure::preparation(url, "上游请求编码无效", 422))?;
        }
    }
    crate::upstream_overrides::apply_body_override(&mut converted.body, &candidate.connection)
        .map_err(|error| Failure::preparation(url, &error, 422))?;
    super::accounts::prepare_body(candidate, &mut converted.body, body)
        .map_err(|error| Failure::preparation(url, &error, 422))?;
    transform::prepare_upstream_request(candidate, &mut converted.body)
        .map_err(|error| Failure::preparation(url, &error, 422))?;
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&converted.body) {
        span.note_mapped_model(model_from_value(candidate.upstream_protocol, &value));
    }
    if converted.body.len() as u64 > MAX_REQUEST_BYTES {
        return Err(Failure::preparation(
            url,
            "转换后的请求体超过本机协议网关限制",
            413,
        ));
    }
    Ok((converted, model))
}

fn timeouts(policy: &ClaudeTrafficSettings, stream: bool) -> Timeouts {
    let first_byte = if stream {
        policy.streaming_first_byte_timeout_seconds
    } else {
        policy.non_streaming_timeout_seconds
    };
    Timeouts {
        headers: Duration::from_secs(policy.headers_timeout_seconds.into()),
        first_byte: Duration::from_secs(first_byte.into()),
        idle: Duration::from_secs(
            if stream {
                policy.streaming_idle_timeout_seconds
            } else {
                policy.non_streaming_timeout_seconds
            }
            .into(),
        ),
        total: Duration::from_secs(
            if stream {
                86400
            } else {
                policy.non_streaming_timeout_seconds
            }
            .into(),
        ),
    }
}
