//! HTTP response bytes, bounded decoding and diagnostic/header projection.

use super::*;

/// Reads and decodes an upstream failure before its body reaches provider
/// diagnostics. Every Codex transport uses this function so compressed error
/// envelopes cannot be mistaken for opaque upstream failures.
pub(crate) fn read_upstream_diagnostic(
    mut upstream: UpstreamResponse,
    secrets: &[&str],
) -> ProviderDiagnostic {
    let endpoint = upstream.url().to_string();
    let status = upstream.status().as_u16();
    let headers = upstream.headers().clone();
    let encoded = match read_limited(&mut upstream, MAX_DIAGNOSTIC_BODY_BYTES as u64) {
        Ok(body) => body,
        Err(ReadLimitError::TooLarge) => {
            return http_diagnostic(&endpoint, status, &headers, &[], true, secrets);
        }
        Err(ReadLimitError::Io) => {
            let mut diagnostic = http_diagnostic(&endpoint, status, &headers, &[], true, secrets);
            diagnostic.message.push_str("；读取上游错误响应时连接中断");
            return diagnostic;
        }
    };
    match content_encoding::decode_response_body(&headers, &encoded, MAX_DIAGNOSTIC_BODY_BYTES) {
        Ok(body) => http_diagnostic(&endpoint, status, &headers, &body, false, secrets),
        Err(error) => {
            let mut diagnostic = http_diagnostic(&endpoint, status, &headers, &[], true, secrets);
            diagnostic.message = format!("无法解压上游错误响应：{error}");
            diagnostic
        }
    }
}

pub(crate) fn read_limited(reader: &mut dyn Read, limit: u64) -> Result<Vec<u8>, ReadLimitError> {
    let mut output = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut output)
        .map_err(|_| ReadLimitError::Io)?;
    if output.len() as u64 > limit {
        return Err(ReadLimitError::TooLarge);
    }
    Ok(output)
}

pub(crate) fn respond_error(
    request: Request,
    protocol: Option<UpstreamProtocol>,
    status: u16,
    message: &str,
) {
    let body = protocol
        .map(|protocol| convert_error(protocol, status, message))
        .unwrap_or_else(|| convert_error(UpstreamProtocol::ChatCompletions, status, message));
    let response = Response::from_data(body)
        .with_status_code(StatusCode(status))
        .with_header(content_type("application/json; charset=utf-8"));
    let _ = request.respond(response);
}

pub(crate) fn respond_bytes(
    request: Request,
    status: u16,
    content_type_value: &str,
    body: Vec<u8>,
) {
    let _ = respond_bytes_with_headers(request, status, content_type_value, body, Vec::new());
}

pub(super) fn respond_bytes_with_headers(
    request: Request,
    status: u16,
    content_type_value: &str,
    body: Vec<u8>,
    mut headers: Vec<Header>,
) -> std::io::Result<()> {
    headers.push(content_type(content_type_value));
    let response = Response::new(StatusCode(status), headers, Cursor::new(body), None, None);
    request.respond(response)
}

pub(super) fn allowed_response_headers(headers: &HeaderMap, secrets: &[&str]) -> Vec<Header> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            matches!(
                name.as_str(),
                "x-request-id"
                    | "retry-after"
                    | "openai-processing-ms"
                    | "x-ratelimit-limit-requests"
                    | "x-ratelimit-remaining-requests"
                    | "x-ratelimit-reset-requests"
                    | "x-ratelimit-limit-tokens"
                    | "x-ratelimit-remaining-tokens"
                    | "x-ratelimit-reset-tokens"
            )
            .then(|| {
                let value = value.to_str().ok()?;
                let value = crate::provider_diagnostics::redact_text(value, secrets);
                Header::from_bytes(name.as_str(), value.as_bytes()).ok()
            })
            .flatten()
        })
        .collect()
}

pub(crate) fn content_type(value: &str) -> Header {
    Header::from_bytes(&b"Content-Type"[..], value.as_bytes()).expect("valid content type")
}
