//! Response rendering after a Codex attempt selected an upstream.
use super::super::diagnostics::{embedded_error, response_diagnostic};
use super::super::respond::convert_safe_json;
use super::*;
use crate::provider_diagnostics::redact_text;

pub(super) fn relay_response<S: Read + Write>(
    socket: &mut WebSocket<S>,
    upstream: UpstreamResponse,
    route: &ActiveRoute,
    context: &mut ConversationContext,
    request: &PendingRequest,
    span: &mut RequestSpan,
) -> ExchangeOutcome {
    let response = match read_response(upstream, route, span) {
        Ok(response) => response,
        Err(diagnostic) => return reject_diagnostic(socket, &diagnostic),
    };
    if response_is_failure(&response) {
        let event = match response.get("status").and_then(Value::as_str) {
            Some("incomplete") => "response.incomplete",
            _ => "response.failed",
        };
        return if send_value(socket, &json!({"type": event, "response": response})) {
            if event == "response.incomplete" {
                ExchangeOutcome::Incomplete
            } else {
                ExchangeOutcome::Rejected
            }
        } else {
            ExchangeOutcome::Disconnect
        };
    }
    if let Err(error) = context.record_completed(request, &response) {
        return if send_failed(socket, error.code, &error.message) {
            ExchangeOutcome::Rejected
        } else {
            ExchangeOutcome::Disconnect
        };
    }
    if send_created_and_completed(socket, &response) {
        ExchangeOutcome::Served
    } else {
        ExchangeOutcome::Disconnect
    }
}

fn read_response(
    mut upstream: UpstreamResponse,
    route: &ActiveRoute,
    span: &mut RequestSpan,
) -> Result<Value, ProviderDiagnostic> {
    let mut diagnostic = response_diagnostic(
        &upstream,
        ProviderFailureKind::ResponseParse,
        "无法转换上游响应",
        &[&route.api_key, &route.client_token],
    );
    let (_, body) = match super::super::respond::read_decoded(&mut upstream) {
        Ok(body) => body,
        Err((kind, message)) => {
            diagnostic.kind = kind;
            diagnostic.message = message;
            return Err(diagnostic);
        }
    };
    if let Ok(value) = serde_json::from_slice::<Value>(&body) {
        span.note_response_value(route.upstream_protocol, &value);
    }
    let native = route.upstream_protocol == UpstreamProtocol::Responses;
    if !native {
        if let Some(error) =
            embedded_error(&body, &diagnostic, &[&route.api_key, &route.client_token])
        {
            return Err(error);
        }
    }
    let body = match convert_safe_json(
        body,
        native,
        UpstreamProtocol::Responses,
        route,
    ) {
        Ok(body) => body,
        Err(error) => {
            diagnostic.message = redact_text(
                &format!("无法转换上游响应：{error}"),
                &[&route.api_key, &route.client_token],
            );
            return Err(diagnostic);
        }
    };
    let response: Value = match serde_json::from_slice(&body) {
        Ok(response) => response,
        Err(error) => {
            diagnostic.message = format!("转换后的响应不是有效 JSON：{error}");
            return Err(diagnostic);
        }
    };
    Ok(response)
}

fn response_is_failure(response: &Value) -> bool {
    response
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| matches!(status, "failed" | "incomplete"))
        || response.get("error").is_some_and(|error| !error.is_null())
}

pub(super) fn relay_stream<S: Read + Write>(
    socket: &mut WebSocket<S>,
    upstream: super::super::UpstreamResponse,
    route: &ActiveRoute,
    context: &mut ConversationContext,
    request: &PendingRequest,
    span: &mut RequestSpan,
) -> ExchangeOutcome {
    let mut diagnostic = response_diagnostic(
        &upstream,
        ProviderFailureKind::StreamParse,
        "无法转换上游 SSE 流",
        &[&route.api_key, &route.client_token],
    );
    let reasoning_transport = ReasoningTransport::from_continuation_key(route.continuation_key);
    let headers = upstream.headers().clone();
    let source = match crate::gateway::content_encoding::decode_response_stream(
        &headers,
        upstream,
        MAX_RESPONSE_BYTES as usize,
    ) {
        Ok(source) => source,
        Err(error) => {
            diagnostic.message = format!("无法解压上游 SSE：{error}");
            return reject_diagnostic(socket, &diagnostic);
        }
    };
    let stream = match SseTranscoder::new(
        source,
        route.upstream_protocol,
        UpstreamProtocol::Responses,
        MAX_RESPONSE_BYTES,
        Some(&reasoning_transport),
    ) {
        Ok(stream) => {
            stream.with_native_responses_dialect(
                crate::gateway::transform::NativeResponsesDialect::for_route(route),
            ).with_diagnostics(diagnostic.clone(), &[&route.api_key, &route.client_token])
        }
        Err(error) => {
            diagnostic.message = format!("无法转换上游 SSE 流：{error}");
            return reject_diagnostic(socket, &diagnostic);
        }
    };
    super::stream::send_stream(socket, stream, route, context, request, diagnostic, span)
}

#[cfg(test)]
mod tests;
