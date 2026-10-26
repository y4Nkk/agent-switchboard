//! Upstream response handling: converted JSON bodies, chunked SSE relaying,
//! error rendering, and the shared size-limited reader.

use super::super::metrics::RequestSpan;
use super::super::transform::{convert_error, convert_response, ReasoningTransport, SseTranscoder};
use super::super::{content_encoding, ActiveRoute};
use super::diagnostics::{embedded_error, respond_diagnostic, response_diagnostic};
use super::UpstreamResponse;
use super::MAX_RESPONSE_BYTES;
pub(in crate::gateway::server) mod codex_operations;
mod wire;
use crate::gateway::http::Request;
use crate::provider_diagnostics::{
    http_diagnostic, ProviderDiagnostic, ProviderFailureKind, MAX_DIAGNOSTIC_BODY_BYTES,
};
use asb_core::contracts::UpstreamProtocol;
use reqwest::header::{HeaderMap, CONTENT_TYPE};
use std::io::{Cursor, Read, Write};
use tiny_http::{Header, Response, StatusCode};
use wire::{allowed_response_headers, respond_bytes_with_headers};
pub(crate) use wire::{
    content_type, read_limited, read_upstream_diagnostic, respond_bytes, respond_error,
};

pub(crate) fn respond_upstream(
    request: Request,
    mut span: RequestSpan,
    client_protocol: UpstreamProtocol,
    route: &ActiveRoute,
    requested_stream: bool,
    upstream: UpstreamResponse,
    mut history: Option<crate::gateway::codex::history::StreamRecorder>,
) {
    let status = upstream.status().as_u16();
    let upstream_stream = upstream
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("text/event-stream"));
    if !(200..300).contains(&status) {
        if upstream.initial_body_received() {
            span.note_first_byte();
        }
        let diagnostic = read_upstream_diagnostic(upstream, &[&route.api_key, &route.client_token]);
        span.finish(Some(status), 0);
        respond_diagnostic(request, client_protocol, status, &diagnostic);
        return;
    }
    let mut diagnostic = response_diagnostic(
        &upstream,
        ProviderFailureKind::ResponseParse,
        "上游响应解析失败",
        &[&route.api_key, &route.client_token],
    );
    if requested_stream != upstream_stream {
        span.finish(Some(502), 0);
        diagnostic.kind = ProviderFailureKind::StreamParse;
        diagnostic.message = if requested_stream {
            "上游服务未按请求返回 SSE 流"
        } else {
            "上游服务返回了未请求的 SSE 流"
        }
        .to_string();
        respond_diagnostic(request, client_protocol, 502, &diagnostic);
        return;
    }
    if requested_stream {
        respond_sse(
            request,
            span,
            client_protocol,
            route,
            upstream,
            diagnostic,
            history.as_mut(),
        );
        return;
    }
    respond_json(
        request,
        span,
        client_protocol,
        route,
        upstream,
        diagnostic,
        history.as_mut(),
    );
}

/// Buffers one non-streaming success before conversion so a provider that
/// returns an error envelope with HTTP 2xx can participate in failover. The
/// returned response keeps the original bytes for the normal responder.
// Claude keeps its existing candidate preflight behavior. Codex native
// Responses are restored for strict upstream dialects immediately before final
// rendering, so only Codex cross-protocol responses are safe to preflight
// against their client protocol here.
fn requires_non_stream_preflight(
    app: asb_core::contracts::AppKind,
    upstream_protocol: UpstreamProtocol,
) -> bool {
    app == asb_core::contracts::AppKind::Claude
        || upstream_protocol != UpstreamProtocol::native_for(app)
}

pub(crate) fn prepare_non_stream_response(
    mut upstream: UpstreamResponse,
    route: &ActiveRoute,
) -> Result<UpstreamResponse, ProviderDiagnostic> {
    let mut diagnostic = response_diagnostic(
        &upstream,
        ProviderFailureKind::ResponseParse,
        "上游响应解析失败",
        &[&route.api_key, &route.client_token],
    );
    let (encoded, body) = read_decoded(&mut upstream).map_err(|(kind, message)| {
        diagnostic.kind = kind;
        diagnostic.message = message;
        diagnostic.clone()
    })?;
    if let Some(error) = embedded_error(&body, &diagnostic, &[&route.api_key, &route.client_token])
    {
        return Err(error);
    }
    let client_protocol = UpstreamProtocol::native_for(route.app);
    if requires_non_stream_preflight(route.app, route.upstream_protocol) {
        let reasoning = ReasoningTransport::from_continuation_key(route.continuation_key);
        convert_response(
            route.upstream_protocol,
            client_protocol,
            &body,
            Some(&reasoning),
        )
        .map_err(|error| {
            diagnostic.message = format!("上游响应无法转换为客户端协议：{error}");
            diagnostic.clone()
        })?;
    }
    Ok(upstream.with_buffered_body(encoded))
}

fn respond_json(
    request: Request,
    mut span: RequestSpan,
    client_protocol: UpstreamProtocol,
    route: &ActiveRoute,
    mut upstream: UpstreamResponse,
    mut diagnostic: ProviderDiagnostic,
    history: Option<&mut crate::gateway::codex::history::StreamRecorder>,
) {
    let status = upstream.status().as_u16();
    let headers = upstream.headers().clone();
    let (_, body) = match read_decoded(&mut upstream) {
        Ok(body) => body,
        Err((kind, message)) => {
            span.finish(Some(502), 0);
            diagnostic.kind = kind;
            diagnostic.message = message;
            respond_diagnostic(request, client_protocol, 502, &diagnostic);
            return;
        }
    };
    if !body.is_empty() {
        span.note_first_byte();
    }
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) {
        span.note_response_value(route.upstream_protocol, &value);
    }
    let native = client_protocol == UpstreamProtocol::Responses
        && route.upstream_protocol == UpstreamProtocol::Responses;
    let response_headers = if native {
        allowed_response_headers(&headers, &[&route.api_key, &route.client_token])
    } else {
        Vec::new()
    };
    if !native {
        if let Some(error) =
            embedded_error(&body, &diagnostic, &[&route.api_key, &route.client_token])
        {
            span.finish(Some(502), 0);
            respond_diagnostic(request, client_protocol, 502, &error);
            return;
        }
    }
    let output = convert_safe_json(body, native, client_protocol, route);
    match output {
        Ok(body) => {
            // Bridge responses are indexed for later Codex continuations;
            // recording never affects the bytes already rendered.
            if let Some(recorder) = history {
                recorder.record_json(&body);
            }
            let client_status = native.then_some(status).unwrap_or(200);
            span.finish(Some(client_status), body.len() as u64);
            respond_bytes_with_headers(
                request,
                client_status,
                "application/json; charset=utf-8",
                body,
                response_headers,
            )
        }
        Err(error) => {
            span.finish(Some(502), 0);
            diagnostic.message = crate::provider_diagnostics::redact_text(
                &format!("无法转换上游响应：{error}"),
                &[&route.api_key, &route.client_token],
            );
            respond_diagnostic(request, client_protocol, 502, &diagnostic);
        }
    }
}

pub(in crate::gateway::server) fn convert_safe_json(
    body: Vec<u8>,
    native: bool,
    client_protocol: UpstreamProtocol,
    route: &ActiveRoute,
) -> Result<Vec<u8>, String> {
    let mut body = if native {
        redact_native_body(&body, &[&route.api_key, &route.client_token])?
    } else {
        body
    };
    if native {
        super::super::transform::NativeResponsesDialect::for_route(route).restore_json(&mut body);
    }
    let transport = ReasoningTransport::from_continuation_key(route.continuation_key);
    convert_response(
        route.upstream_protocol,
        client_protocol,
        &body,
        Some(&transport),
    )
    .map_err(|error| error.to_string())
}

fn respond_sse(
    request: Request,
    mut span: RequestSpan,
    client_protocol: UpstreamProtocol,
    route: &ActiveRoute,
    upstream: UpstreamResponse,
    mut diagnostic: ProviderDiagnostic,
    history: Option<&mut crate::gateway::codex::history::StreamRecorder>,
) {
    diagnostic.kind = ProviderFailureKind::StreamParse;
    if upstream.initial_body_received() {
        span.note_first_byte();
    }
    let reasoning_transport = ReasoningTransport::from_continuation_key(route.continuation_key);
    let headers = upstream.headers().clone();
    let native = client_protocol == UpstreamProtocol::Responses
        && route.upstream_protocol == UpstreamProtocol::Responses;
    let status = native.then_some(upstream.status().as_u16()).unwrap_or(200);
    let source = match content_encoding::decode_response_stream(
        &headers,
        upstream,
        MAX_RESPONSE_BYTES as usize,
    ) {
        Ok(source) => source,
        Err(error) => {
            span.finish(Some(502), 0);
            diagnostic.message = format!("无法解压上游 SSE：{error}");
            respond_diagnostic(request, client_protocol, 502, &diagnostic);
            return;
        }
    };
    match SseTranscoder::new(
        source,
        route.upstream_protocol,
        client_protocol,
        MAX_RESPONSE_BYTES,
        Some(&reasoning_transport),
    ) {
        Ok(stream) => respond_stream(
            request,
            span,
            stream.with_native_responses_dialect(
                super::super::transform::NativeResponsesDialect::for_route(route),
            ).with_diagnostics(diagnostic, &[&route.api_key, &route.client_token]),
            status,
            // The stream remains under validation after the HTTP response
            // begins. Its later diagnostic cannot retract upstream headers,
            // so preserve no provider headers on this parsed path.
            native
                .then(|| allowed_response_headers(&headers, &[&route.api_key, &route.client_token]))
                .unwrap_or_default(),
            history,
        ),
        Err(error) => {
            span.finish(Some(502), 0);
            diagnostic.message = format!("无法转换上游 SSE：{error}");
            respond_diagnostic(request, client_protocol, 502, &diagnostic);
        }
    }
}

pub(crate) fn redact_native_body(body: &[u8], secrets: &[&str]) -> Result<Vec<u8>, String> {
    let text =
        std::str::from_utf8(body).map_err(|_| "原生 Responses 响应不是有效 UTF-8".to_string())?;
    Ok(crate::provider_diagnostics::redact_text(text, secrets).into_bytes())
}

pub(super) fn respond_stream<R>(
    request: Request,
    mut span: RequestSpan,
    mut stream: SseTranscoder<R>,
    status: u16,
    mut headers: Vec<Header>,
    mut history: Option<&mut crate::gateway::codex::history::StreamRecorder>,
) where
    R: Read,
{
    headers.push(content_type("text/event-stream; charset=utf-8"));
    headers.push(Header::from_bytes(b"Cache-Control", b"no-cache").expect("valid header"));
    let mut writer = match request.stream_response_with_headers(status, &headers) {
        Ok(writer) => writer,
        Err(_) => {
            span.finish(None, 0);
            return;
        }
    };
    let mut buffer = [0_u8; 8 * 1024];
    let mut written: u64 = 0;
    let mut write_failed = false;
    loop {
        let count = match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) => {
                stream.fail_io(&error);
                continue;
            }
        };
        span.note_first_byte();
        if let Some(recorder) = history.as_deref_mut() {
            recorder.feed(&buffer[..count]);
        }
        if stream.has_token() {
            span.note_first_token();
        }
        if writer
            .write_all(&buffer[..count])
            .and_then(|_| writer.flush())
            .is_err()
        {
            write_failed = true;
            break;
        }
        written += count as u64;
    }
    let _ = writer.flush();
    span.note_response_model(stream.model());
    span.note_usage(stream.usage());
    let status = if write_failed {
        None
    } else {
        Some(if stream.failed() { 502 } else { status })
    };
    span.finish(status, written);
}

pub(crate) enum ReadLimitError {
    TooLarge,
    Io,
}

pub(in crate::gateway::server) fn read_decoded(
    upstream: &mut UpstreamResponse,
) -> Result<(Vec<u8>, Vec<u8>), (ProviderFailureKind, String)> {
    let encoded = read_limited(upstream, MAX_RESPONSE_BYTES).map_err(|error| match error {
        ReadLimitError::TooLarge => (
            ProviderFailureKind::Upstream,
            "上游响应超过本机协议网关限制".into(),
        ),
        ReadLimitError::Io => (
            ProviderFailureKind::Network,
            "读取上游响应时连接中断".into(),
        ),
    })?;
    let body = content_encoding::decode_response_body(
        upstream.headers(),
        &encoded,
        MAX_RESPONSE_BYTES as usize,
    )
    .map_err(|error| {
        (
            ProviderFailureKind::ResponseParse,
            format!("无法解压上游响应：{error}"),
        )
    })?;
    Ok((encoded, body))
}

#[cfg(test)]
mod tests;
