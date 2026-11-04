//! Transparent Codex auxiliary responses with credential-free usage observation.
use super::*;
/// Relays a typed Codex sibling operation which is already in the upstream
/// protocol. These operations have no Responses-to-other-protocol conversion
/// contract, so a successful upstream status and JSON body remain intact.
pub(crate) fn respond_passthrough(
    request: Request,
    mut span: RequestSpan,
    client_protocol: UpstreamProtocol,
    route: &ActiveRoute,
    mut upstream: UpstreamResponse,
) {
    let metadata_protocol = super::super::codex_request(request.url())
        .filter(|path| path.operation == crate::gateway::server::CodexOperation::ChatCompletions)
        .map_or(route.upstream_protocol, |_| {
            UpstreamProtocol::ChatCompletions
        });
    let status = upstream.status().as_u16();
    let headers = upstream.headers().clone();
    if !(200..300).contains(&status) {
        let diagnostic = read_upstream_diagnostic(upstream, &[&route.api_key, &route.client_token]);
        span.finish(Some(status), 0);
        respond_diagnostic(request, client_protocol, status, &diagnostic);
        return;
    }
    let upstream_stream = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("text/event-stream"));
    if upstream_stream {
        let source = match content_encoding::decode_response_stream(
            &headers,
            upstream,
            MAX_RESPONSE_BYTES as usize,
        ) {
            Ok(source) => source,
            Err(error) => {
                span.finish(Some(502), 0);
                respond_error(
                    request,
                    Some(client_protocol),
                    502,
                    &format!("无法解压上游 SSE：{error}"),
                );
                return;
            }
        };
        respond_passthrough_sse(
            request,
            span,
            status,
            allowed_response_headers(&headers, &[&route.api_key, &route.client_token]),
            source,
            metadata_protocol,
        );
        return;
    }
    let (_, body) = match read_decoded(&mut upstream) {
        Ok(body) => body,
        Err((_, message)) => {
            span.finish(Some(502), 0);
            respond_error(request, Some(client_protocol), 502, &message);
            return;
        }
    };
    if !body.is_empty() {
        span.note_first_byte();
    }
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&body) {
        span.note_response_value(metadata_protocol, &value);
    }
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/json; charset=utf-8");
    span.finish(Some(status), body.len() as u64);
    let _ = respond_bytes_with_headers(
        request,
        status,
        content_type,
        body,
        allowed_response_headers(&headers, &[&route.api_key, &route.client_token]),
    );
}

fn respond_passthrough_sse(
    request: Request,
    mut span: RequestSpan,
    status: u16,
    mut headers: Vec<Header>,
    mut source: Box<dyn Read>,
    metadata_protocol: UpstreamProtocol,
) {
    headers.push(content_type("text/event-stream; charset=utf-8"));
    headers.push(Header::from_bytes(b"Cache-Control", b"no-cache").expect("valid header"));
    let mut writer = match request.stream_response_with_headers(status, &headers) {
        Ok(writer) => writer,
        Err(_) => {
            span.finish(None, 0);
            return;
        }
    };
    let mut metadata = crate::gateway::usage_metadata::SseMetadata::default();
    let mut buffer = [0_u8; 8 * 1024];
    let mut written = 0_u64;
    loop {
        let count = match source.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(_) => {
                span.finish(Some(502), written);
                return;
            }
        };
        span.note_first_byte();
        for value in metadata.push(&buffer[..count]) {
            span.note_response_value(metadata_protocol, &value);
        }
        written += count as u64;
        if written > MAX_RESPONSE_BYTES {
            span.finish(Some(502), written);
            return;
        }
        if writer
            .write_all(&buffer[..count])
            .and_then(|_| writer.flush())
            .is_err()
        {
            span.finish(None, written);
            return;
        }
    }
    let _ = writer.flush();
    span.finish(Some(status), written);
}
