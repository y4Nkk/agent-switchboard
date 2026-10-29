use super::*;
use crate::gateway::codex::policy::CodexTrafficSettings;
use tiny_http::Header;

pub(in crate::gateway::server) struct Prepared {
    url: String,
    converted: ConvertedRequest,
    pub model: Option<String>,
    anthropic_beta: Option<String>,
}
pub(in crate::gateway::server) struct Failure {
    pub diagnostic: ProviderDiagnostic,
    pub status: u16,
    pub retryable: bool,
    pub penalize: bool,
}

pub(in crate::gateway::server) fn prepare(
    route: &ActiveRoute,
    gateway_base: &str,
    request_url: &str,
    body: &[u8],
    incoming: &[Header],
    history: &crate::gateway::codex::history::CodexToolHistory,
) -> Result<Prepared, Failure> {
    let invalid = |message: String| Failure {
        status: 422,
        retryable: false,
        penalize: false,
        diagnostic: ProviderDiagnostic::new(
            ProviderFailureKind::RequestParameters,
            &route.upstream_base_url,
            &message,
        ),
    };
    let mut body = super::super::codex::resolve_model_and_validate(
        route,
        CodexOperation::Responses,
        body.to_vec(),
    )
    .map_err(invalid)?;
    // Short Codex continuations only convert when the referenced tool calls
    // are restored first; cross-protocol bridges all need it. Native
    // Responses keeps `previous_response_id` for the upstream's own store.
    if route.upstream_protocol != UpstreamProtocol::Responses {
        let binding =
            crate::gateway::codex::history::HistoryBinding::for_request(&body, Some(incoming));
        let mut value: serde_json::Value =
            serde_json::from_slice(&body).map_err(|_| invalid("Codex 源请求不是 JSON".into()))?;
        let had_previous = value.get("previous_response_id").is_some();
        let changed = history.enrich(&binding, &mut value).map_err(invalid)?;
        if changed > 0 || had_previous {
            body =
                serde_json::to_vec(&value).map_err(|_| invalid("Codex 源请求无法编码".into()))?;
        }
    }
    let key = ReasoningTransport::from_continuation_key(route.continuation_key);
    let mut converted = convert_request(
        UpstreamProtocol::Responses,
        route.upstream_protocol,
        &body,
        route.max_output_tokens,
        Some(&key),
        route.codex.as_ref().map(|s| &s.capabilities.chat_reasoning),
    )
    .and_then(|request| crate::gateway::transform::minimal::apply(request, route.responses_options))
    .map_err(|error| invalid(error.to_string()))?;
    let anthropic_beta =
        crate::gateway::codex::request::prepare(route, &body, &mut converted.body, Some(incoming))
            .map_err(invalid)?;
    let model =
        crate::gateway::usage_metadata::model_from_bytes(route.upstream_protocol, &converted.body);
    let url = upstream_url(route, gateway_base, request_url).map_err(invalid)?;
    Ok(Prepared {
        url,
        converted,
        model,
        anthropic_beta,
    })
}

pub(in crate::gateway::server) fn send(
    client: &UpstreamClient,
    route: &ActiveRoute,
    incoming: &[Header],
    cancelled: &dyn Fn() -> bool,
    prepared: Prepared,
    settings: &CodexTrafficSettings,
) -> Result<(UpstreamResponse, bool), Failure> {
    let stream = prepared.converted.stream;
    let response = super::super::transport::send_with_timeouts(
        client,
        route,
        &prepared.url,
        reqwest::Method::POST,
        prepared.converted.body,
        Some(incoming),
        None,
        prepared.anthropic_beta.as_deref(),
        settings.timeouts(stream),
        Some(cancelled),
    )
    .map_err(classify)?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let diagnostic = read_upstream_diagnostic(response, &[&route.api_key, &route.client_token]);
        return Err(Failure {
            diagnostic,
            status,
            retryable: crate::gateway::is_retryable_http_status(status),
            penalize: crate::gateway::is_retryable_http_status(status),
        });
    }
    let upstream_stream = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("text/event-stream"));
    if stream != upstream_stream {
        return Err(classify(ProviderDiagnostic::new(
            ProviderFailureKind::StreamParse,
            &prepared.url,
            "Codex 上游没有按请求返回对应的 JSON/SSE 格式",
        )));
    }
    if stream && !response.initial_body_received() {
        return Err(classify(ProviderDiagnostic::new(
            ProviderFailureKind::Network,
            &prepared.url,
            "Codex 上游流在首包前结束",
        )));
    }
    let response = if stream {
        response
    } else {
        prepare_non_stream_response(response, route).map_err(classify)?
    };
    Ok((response, stream))
}

pub(in crate::gateway::server) fn classify(diagnostic: ProviderDiagnostic) -> Failure {
    let retryable = match diagnostic.kind {
        ProviderFailureKind::Dns
        | ProviderFailureKind::Tls
        | ProviderFailureKind::Network
        | ProviderFailureKind::Timeout
        | ProviderFailureKind::RateLimit
        | ProviderFailureKind::ResponseParse
        | ProviderFailureKind::StreamParse => true,
        ProviderFailureKind::Upstream => diagnostic
            .status
            .is_some_and(crate::gateway::is_retryable_http_status),
        _ => false,
    };
    let status = diagnostic.status.filter(|status| *status >= 400).unwrap_or(
        if diagnostic.kind == ProviderFailureKind::Timeout {
            504
        } else {
            502
        },
    );
    Failure {
        diagnostic,
        status,
        retryable,
        penalize: retryable,
    }
}

pub(in crate::gateway::server) fn resolve_route(
    route: &ActiveRoute,
    inner: &GatewayInner,
    headers: &[Header],
) -> Result<ActiveRoute, Failure> {
    super::super::codex_account::resolve(route, inner, Some(headers)).map_err(|(status, message)| {
        Failure {
            diagnostic: ProviderDiagnostic::new(
                ProviderFailureKind::Authentication,
                &route.upstream_base_url,
                &message,
            ),
            status,
            retryable: false,
            penalize: false,
        }
    })
}

pub(in crate::gateway::server) fn media_fallback_body(
    body: &[u8],
    failure: &Failure,
    enabled: bool,
    retried: &mut bool,
) -> Option<Vec<u8>> {
    if *retried || !enabled
        || !crate::gateway::codex::media::is_unsupported_image_failure(&failure.diagnostic)
    {
        return None;
    }
    let mut value: serde_json::Value = serde_json::from_slice(body).ok()?;
    if !crate::gateway::codex::media::contains_images(&value) {
        return None;
    }
    if crate::gateway::codex::media::replace_images_with_marker(&mut value) == 0 {
        return None;
    }
    let body = serde_json::to_vec(&value).ok()?;
    *retried = true;
    Some(body)
}
