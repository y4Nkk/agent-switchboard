//! Manual endpoint probing and the shared outbound HTTP transport.
//!
//! One probe answers exactly one question: does this URL answer HTTP(S)
//! requests, with which status, and how fast. It never selects or switches
//! anything. The transport is [`reqwest`] with rustls verifying against the
//! OS root store, so the system trust and proxy settings apply the way the
//! native stack did. The shared [`http_get`] helper carries that same stack
//! to other outbound reads (model fetch, update check, quota, OAuth).

use serde::Serialize;
use std::error::Error as _;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Outcome grade of one manual probe, ported from the CC Switch reachability
/// pattern: any HTTP answer proves reachability, latency above the threshold
/// grades as slow, only network-level failures (DNS / refused / TLS / timeout)
/// grade as unreachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProbeGrade {
    Ok,
    Slow,
    Unreachable,
}

/// Reachability degrades to "slow" past this TTFB, mirroring the reference
/// 6000 ms scale: probes answer in well under a second, so only genuinely
/// slow paths get flagged.
const SLOW_THRESHOLD_MS: u64 = 6_000;

fn grade_for(latency_ms: u64) -> ProbeGrade {
    if latency_ms > SLOW_THRESHOLD_MS {
        ProbeGrade::Slow
    } else {
        ProbeGrade::Ok
    }
}

/// Result of one manual endpoint probe, surfaced to the UI as-is.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub grade: ProbeGrade,
    pub status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    /// RFC 3339 UTC timestamp of the probe.
    pub at: String,
}

/// The failure class a transport error belongs to. Retry decisions and the
/// user-facing message both key off this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureKind {
    Dns,
    Timeout,
    Connect,
    Tls,
    Other,
}

/// A failed transport call. The message is only the failure class — the raw
/// reqwest error text may embed the URL and is never surfaced.
#[derive(Debug, PartialEq)]
struct ProbeFailure {
    kind: FailureKind,
    message: String,
}

impl ProbeFailure {
    fn is_timeout(&self) -> bool {
        self.kind == FailureKind::Timeout
    }
}

/// Maps a failure class to the message the user can act on.
fn failure_message(kind: FailureKind) -> &'static str {
    match kind {
        FailureKind::Dns => "域名解析失败（DNS）",
        FailureKind::Timeout => "连接超时",
        FailureKind::Connect => "连接被拒绝",
        FailureKind::Tls => "TLS 握手失败",
        FailureKind::Other => "网络请求失败",
    }
}

/// The process-wide transport. The connect budget mirrors the native stack;
/// the per-attempt and per-request total budgets are applied where the calls
/// are made (see [`probe`] and [`http_request`]).
fn client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .user_agent("Agent Switchboard")
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("static network client configuration is valid")
    })
}

/// Total budget of one probe attempt: the 5 s connect phase plus a receive
/// phase, matching the native stack's 10 s receive timeout.
const PROBE_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);

/// Total budget of one payload-carrying request, matching the native stack's
/// 15 s send and receive timeouts.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Walks a transport error's source chain and assigns the failure class the
/// user can act on. reqwest exposes no typed DNS/TLS predicates, so the chain
/// text is matched against the stable error vocabulary of hyper/rustls/OS
/// errors; anything unrecognised lands in the generic class.
fn classify(error: &reqwest::Error) -> ProbeFailure {
    let mut texts = Vec::new();
    let mut current: Option<&dyn std::error::Error> = error.source();
    while let Some(source) = current {
        texts.push(source.to_string());
        current = source.source();
    }
    let kind = classify_kind(&texts);
    ProbeFailure {
        kind,
        message: failure_message(kind).to_string(),
    }
}

fn classify_kind(source_texts: &[String]) -> FailureKind {
    let lower: Vec<String> = source_texts
        .iter()
        .map(|text| text.to_ascii_lowercase())
        .collect();
    let matches = |needles: &[&str]| {
        lower
            .iter()
            .any(|text| needles.iter().any(|needle| text.contains(needle)))
    };
    if error_is_timeout(source_texts) {
        return FailureKind::Timeout;
    }
    if matches(&[
        "dns error",
        "failed to lookup",
        "name or service not known",
        "nodename nor servname",
        "temporary failure in name resolution",
        "no such host",
    ]) {
        FailureKind::Dns
    } else if matches(&[
        "certificate",
        "invalid peer",
        "unknown issuer",
        "tls",
        "ssl",
    ]) {
        FailureKind::Tls
    } else if matches(&[
        "refused",
        "unreachable",
        "reset by peer",
        "tcp connect",
        "connect error",
        "client error (connect)",
    ]) {
        FailureKind::Connect
    } else {
        FailureKind::Other
    }
}

fn error_is_timeout(source_texts: &[String]) -> bool {
    source_texts.iter().any(|text| {
        let text = text.to_ascii_lowercase();
        text.contains("timed out") || text.contains("operation was canceled due to timeouts")
    })
}

/// Probe retry policy, separated from the transport layer for testing:
/// timeout-class failures get exactly one retry (network jitter), immediate
/// failures such as refused connections or DNS errors fail fast.
fn probe_with_retries(
    mut attempt: impl FnMut() -> Result<u16, ProbeFailure>,
) -> Result<u16, ProbeFailure> {
    match attempt() {
        Err(failure) if failure.is_timeout() => attempt(),
        first => first,
    }
}

struct ParsedUrl {
    host: String,
    port: u16,
    path: String,
    secure: bool,
}

/// Splits a validated `http(s)://` URL into scheme, authority and path.
fn parse_url(url: &str) -> Option<ParsedUrl> {
    let (secure, rest) = if let Some(rest) = url.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        (false, rest)
    } else {
        return None;
    };
    let split = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..split];
    let mut path = rest[split..].to_string();
    if path.is_empty() || path.starts_with('?') || path.starts_with('#') {
        path.insert(0, '/');
    }
    // IPv6 literals keep their brackets out of the port split.
    let (host, port) = if let Some(close) = authority.find(']') {
        let host = authority[..=close].to_string();
        let tail = &authority[close + 1..];
        let port = tail
            .strip_prefix(':')
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(if secure { 443 } else { 80 });
        (host, port)
    } else {
        match authority.rsplit_once(':') {
            Some((host, digits))
                if !host.is_empty()
                    && !digits.is_empty()
                    && digits.bytes().all(|b| b.is_ascii_digit()) =>
            {
                (host.to_string(), digits.parse::<u16>().ok()?)
            }
            _ => (authority.to_string(), if secure { 443 } else { 80 }),
        }
    };
    if host.is_empty() {
        return None;
    }
    Some(ParsedUrl {
        host,
        port,
        path,
        secure,
    })
}

fn request_url(parsed: &ParsedUrl) -> String {
    let scheme = if parsed.secure { "https" } else { "http" };
    format!("{scheme}://{}:{}{}", parsed.host, parsed.port, parsed.path)
}

/// Probes one URL for reachability. Any HTTP answer counts — the status code
/// (200/4xx/5xx alike) only proves the endpoint is alive; the probe sends no
/// model request and carries no credential, so it never validates
/// authentication or model configuration. The response body is never read.
pub fn probe(url: &str) -> Result<ProbeResult, String> {
    let parsed = parse_url(url).ok_or_else(|| "端点必须是 http(s) URL".to_string())?;
    let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let url = request_url(&parsed);

    let started = Instant::now();
    let outcome = probe_with_retries(|| {
        client()
            .get(&url)
            .timeout(PROBE_ATTEMPT_TIMEOUT)
            .send()
            .map(|response| response.status().as_u16())
            .map_err(|error| classify(&error))
    });
    let latency_ms = Some(started.elapsed().as_millis() as u64);
    Ok(match outcome {
        Ok(status) => ProbeResult {
            grade: grade_for(latency_ms.unwrap_or(0)),
            status: Some(status),
            latency_ms,
            error: None,
            at,
        },
        Err(failure) => ProbeResult {
            grade: ProbeGrade::Unreachable,
            status: None,
            latency_ms,
            error: Some(failure.message),
            at,
        },
    })
}

/// Parses the CRLF-joined request-header convention into a header map. An
/// empty string maps to no headers; anything malformed is rejected loudly
/// instead of silently dropped.
fn header_map(raw: &str) -> Result<reqwest::header::HeaderMap, String> {
    let mut map = reqwest::header::HeaderMap::new();
    for line in raw.split("\r\n") {
        if line.trim().is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| "请求头无效".to_string())?;
        let name = reqwest::header::HeaderName::from_bytes(name.trim().as_bytes())
            .map_err(|_| "请求头无效".to_string())?;
        let value = reqwest::header::HeaderValue::from_bytes(value.trim().as_bytes())
            .map_err(|_| "请求头无效".to_string())?;
        map.insert(name, value);
    }
    Ok(map)
}

/// One request returning the response status and body text. The shared
/// client doubles as the required `User-Agent`; callers pass extra request
/// headers as CRLF-joined lines. The whole request is budgeted at 15 s.
pub fn http_request(
    method: &str,
    url: &str,
    headers: &str,
    body: &[u8],
) -> Result<(u16, String), String> {
    let parsed = parse_url(url).ok_or_else(|| "请求地址必须是 http(s) URL".to_string())?;
    let method =
        reqwest::Method::from_bytes(method.as_bytes()).map_err(|_| "请求方式无效".to_string())?;
    let headers = header_map(headers)?;

    let mut request = client()
        .request(method, request_url(&parsed))
        .timeout(REQUEST_TIMEOUT)
        .headers(headers);
    if !body.is_empty() {
        request = request.body(body.to_vec());
    }
    let response = request.send().map_err(|error| classify(&error).message)?;
    let status = response.status().as_u16();
    let bytes = response.bytes().map_err(|error| classify(&error).message)?;
    Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
}

/// Convenience wrapper for the existing outbound read-only callers.
pub fn http_get(url: &str, headers: &str) -> Result<(u16, String), String> {
    http_request("GET", url, headers, &[])
}

/// Builds the OpenAI-compatible model-list path for a provider base URL:
/// a base already ending in a `/v{digits}` segment gets `/models` appended,
/// anything else gets `/v1/models`.
fn models_path_for(base_url: &str) -> String {
    let mut path = parse_url(base_url)
        .map(|parsed| parsed.path)
        .unwrap_or_else(|| "/".to_string());
    let trimmed = path.trim_end_matches('/');
    let versioned = trimmed.rsplit('/').next().is_some_and(|segment| {
        let digits = segment.strip_prefix('v').unwrap_or("");
        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
    });
    if !path.ends_with('/') {
        path.push('/');
    }
    path.push_str(if versioned { "models" } else { "v1/models" });
    path
}

/// One model from the provider's `/v1/models` list: the requestable id plus
/// the optional `owned_by` vendor used to group the picker menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModel {
    pub id: String,
    pub owned_by: Option<String>,
}

/// Fetches the OpenAI-compatible `GET /v1/models` list from a provider base
/// URL and returns the models in server order, deduplicated. The profile
/// API key travels only in request headers and is never logged or echoed in
/// errors. Nothing is cached.
pub fn fetch_models(base_url: &str, api_key: &str) -> Result<Vec<ProviderModel>, String> {
    let parsed = parse_url(base_url).ok_or_else(|| "服务地址必须是 http(s) URL".to_string())?;
    let path = models_path_for(base_url);
    let url = format!(
        "{}://{}:{}{path}",
        if parsed.secure { "https" } else { "http" },
        parsed.host,
        parsed.port
    );
    let (status, body) = http_get(&url, &auth_headers(api_key))?;

    if !(200..300).contains(&status) {
        if status == 401 || status == 403 {
            return Err(format!(
                "服务地址拒绝了 API 密钥（HTTP {status}），请确认密钥仍然有效；可手动填写模型名"
            ));
        }
        return Err(format!(
            "服务地址返回 HTTP {status}，无法获取模型列表；可手动填写模型名"
        ));
    }
    parse_models(&body)
}

/// Builds the credential request headers. `Authorization: Bearer` is the
/// OpenAI-compatible form, `x-api-key` the Anthropic form; both are sent so
/// one fetch serves either ecosystem. `anthropic-version` is only required
/// by the Anthropic API itself and ignored elsewhere. The value must never
/// be logged or echoed in diagnostics.
pub(crate) fn auth_headers(credential: &str) -> String {
    format!(
        "Authorization: Bearer {credential}\r\nx-api-key: {credential}\r\nanthropic-version: 2023-06-01"
    )
}

/// Extracts `data[].id` plus the optional `data[].owned_by` vendor from an
/// OpenAI-compatible models response. A missing, non-string, or blank
/// `owned_by` stays `None` so the picker groups it under "其他".
fn parse_models(text: &str) -> Result<Vec<ProviderModel>, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| "模型列表响应不是有效 JSON".to_string())?;
    let data = value
        .get("data")
        .and_then(|data| data.as_array())
        .ok_or_else(|| "模型列表响应缺少 data 数组".to_string())?;
    let mut models: Vec<ProviderModel> = Vec::new();
    for entry in data {
        if let Some(id) = entry.get("id").and_then(|id| id.as_str()) {
            let owned_by = entry
                .get("owned_by")
                .and_then(|vendor| vendor.as_str())
                .map(str::trim)
                .filter(|vendor| !vendor.is_empty())
                .map(str::to_string);
            if !id.is_empty() && !models.iter().any(|seen: &ProviderModel| seen.id == id) {
                models.push(ProviderModel {
                    id: id.to_string(),
                    owned_by,
                });
            }
        }
    }
    if models.is_empty() {
        return Err("模型列表为空".to_string());
    }
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn parses_http_and_https_urls_with_and_without_ports() {
        let https = parse_url("https://api.example.com/v1").expect("https");
        assert_eq!(https.host, "api.example.com");
        assert_eq!(https.port, 443);
        assert_eq!(https.path, "/v1");
        assert!(https.secure);

        let http = parse_url("http://gateway.local:8443").expect("http");
        assert_eq!(http.port, 8443);
        assert_eq!(http.path, "/");
        assert!(!http.secure);

        let v6 = parse_url("https://[::1]:9443/base?query=1").expect("ipv6");
        assert_eq!(v6.host, "[::1]");
        assert_eq!(v6.port, 9443);
        assert_eq!(v6.path, "/base?query=1");
    }

    #[test]
    fn rejects_urls_without_a_host() {
        assert!(parse_url("https:///path").is_none());
        assert!(parse_url("ftp://example.com").is_none());
    }

    #[test]
    fn reachable_latencies_grade_ok_until_the_slow_threshold() {
        assert_eq!(grade_for(0), ProbeGrade::Ok);
        assert_eq!(grade_for(6_000), ProbeGrade::Ok);
        assert_eq!(grade_for(6_001), ProbeGrade::Slow);
        assert_eq!(grade_for(45_000), ProbeGrade::Slow);
    }

    fn timeout_failure() -> ProbeFailure {
        ProbeFailure {
            kind: FailureKind::Timeout,
            message: failure_message(FailureKind::Timeout).to_string(),
        }
    }

    fn refused_failure() -> ProbeFailure {
        ProbeFailure {
            kind: FailureKind::Connect,
            message: failure_message(FailureKind::Connect).to_string(),
        }
    }

    #[test]
    fn timeout_failures_get_exactly_one_retry() {
        let mut attempts = 0;
        let outcome = probe_with_retries(|| {
            attempts += 1;
            if attempts == 1 {
                Err(timeout_failure())
            } else {
                Ok(204)
            }
        });
        assert_eq!(outcome, Ok(204));
        assert_eq!(attempts, 2);
    }

    #[test]
    fn immediate_failures_do_not_retry() {
        let mut attempts = 0;
        let outcome = probe_with_retries(|| {
            attempts += 1;
            Err(refused_failure())
        });
        assert!(outcome.is_err());
        assert_eq!(attempts, 1);
    }

    #[test]
    fn a_second_timeout_is_final() {
        let mut attempts = 0;
        let outcome = probe_with_retries(|| {
            attempts += 1;
            Err(timeout_failure())
        });
        assert!(outcome.is_err());
        assert_eq!(attempts, 2);
    }

    #[test]
    fn failure_kinds_map_to_actionable_classes() {
        assert_eq!(failure_message(FailureKind::Timeout), "连接超时");
        assert_eq!(failure_message(FailureKind::Dns), "域名解析失败（DNS）");
        assert_eq!(failure_message(FailureKind::Connect), "连接被拒绝");
        assert_eq!(failure_message(FailureKind::Tls), "TLS 握手失败");
        assert_eq!(failure_message(FailureKind::Other), "网络请求失败");
    }

    fn chain(texts: &[&str]) -> Vec<String> {
        texts.iter().map(|text| text.to_string()).collect()
    }

    #[test]
    fn classify_kind_maps_realistic_transport_errors() {
        assert_eq!(
            classify_kind(&chain(&[
                "client error (Connect)",
                "dns error: failed to lookup address information: Name or service not known",
            ])),
            FailureKind::Dns
        );
        assert_eq!(
            classify_kind(&chain(&[
                "client error (Connect)",
                "invalid peer certificate: UnknownIssuer",
            ])),
            FailureKind::Tls
        );
        assert_eq!(
            classify_kind(&chain(&[
                "client error (Connect)",
                "tcp connect error: Connection refused (os error 111)",
            ])),
            FailureKind::Connect
        );
        assert_eq!(
            classify_kind(&chain(&["operation timed out"])),
            FailureKind::Timeout
        );
        assert_eq!(
            classify_kind(&chain(&["something unfamiliar"])),
            FailureKind::Other
        );
    }

    #[test]
    fn unreachable_endpoint_grades_unreachable() {
        // Port 1 on loopback is closed in every supported test environment.
        let result = probe("http://127.0.0.1:1/health").expect("probe result");
        assert_eq!(result.grade, ProbeGrade::Unreachable);
        assert_eq!(result.status, None);
        assert_eq!(result.error.as_deref(), Some("连接被拒绝"));
    }

    #[test]
    fn http_request_passes_non_2xx_status_and_body_through() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback server");
        let address = listener.local_addr().expect("loopback address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .expect("set read timeout");
            let mut request = Vec::new();
            let mut chunk = [0u8; 512];
            loop {
                let read = stream.read(&mut chunk).expect("read request");
                assert_ne!(read, 0, "client closed before completing the request");
                request.extend_from_slice(&chunk[..read]);
                if request.windows(4).any(|part| part == b"\r\n\r\n") {
                    stream
                        .write_all(
                            b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 16\r\n\r\n{\"error\":\"boom\"}",
                        )
                        .expect("write response");
                    return;
                }
            }
        });

        let (status, body) = http_request("GET", &format!("http://{address}/usage"), "", &[])
            .expect("request reaches a 500 responder");
        assert_eq!(status, 500);
        assert_eq!(body, "{\"error\":\"boom\"}");
        server.join().expect("join loopback server");
    }

    #[test]
    fn http_request_sends_nonempty_headers_and_body_to_a_loopback_server() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback server");
        let address = listener.local_addr().expect("loopback address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .expect("set read timeout");
            let mut request = Vec::new();
            let mut chunk = [0u8; 512];
            loop {
                let read = stream.read(&mut chunk).expect("read request");
                assert_ne!(read, 0, "client closed before completing the request");
                request.extend_from_slice(&chunk[..read]);

                let Some(headers_end) = request.windows(4).position(|part| part == b"\r\n\r\n")
                else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..headers_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("Content-Length:")
                            .or_else(|| line.strip_prefix("content-length:"))
                    })
                    .map(str::trim)
                    .map(|value| value.parse::<usize>().expect("numeric content length"))
                    .unwrap_or(0);
                if request.len() >= headers_end + 4 + content_length {
                    stream
                        .write_all(b"HTTP/1.1 201 Created\r\nContent-Length: 2\r\n\r\nok")
                        .expect("write loopback response");
                    return request;
                }
            }
        });

        let (status, body) = http_request(
            "POST",
            &format!("http://{address}/quota"),
            "X-ASB-Test: header-value\r\nContent-Type: text/plain\r\n",
            b"ping",
        )
        .expect("request succeeds with explicit headers");

        assert_eq!(status, 201);
        assert_eq!(body, "ok");
        // hyper normalizes header names to lowercase on the wire; header
        // names are case-insensitive per RFC 9110, so compare likewise.
        let request = String::from_utf8(server.join().expect("join loopback server"))
            .expect("UTF-8 loopback request")
            .to_ascii_lowercase();
        assert!(request.starts_with("post /quota http/1.1\r\n"));
        assert!(request.contains("x-asb-test: header-value\r\n"));
        assert!(request.contains("content-type: text/plain\r\n"));
        assert!(request.ends_with("\r\n\r\nping"));
    }

    #[test]
    fn model_fetch_uses_the_shared_transport() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback server");
        let address = listener.local_addr().expect("loopback address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept model request");
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .expect("set read timeout");
            let mut request = Vec::new();
            let mut chunk = [0u8; 512];
            loop {
                let read = stream.read(&mut chunk).expect("read model request");
                assert_ne!(read, 0, "client closed before completing the request");
                request.extend_from_slice(&chunk[..read]);
                if request.windows(4).any(|part| part == b"\r\n\r\n") {
                    stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 44\r\n\r\n{\"data\":[{\"id\":\"model-a\"},{\"id\":\"model-b\"}]}",
                        )
                        .expect("write model response");
                    return request;
                }
            }
        });

        let models = fetch_models(&format!("http://{address}/api"), "test-credential")
            .expect("fetch models through shared transport");

        assert_eq!(
            models,
            vec![
                ProviderModel {
                    id: "model-a".to_string(),
                    owned_by: None,
                },
                ProviderModel {
                    id: "model-b".to_string(),
                    owned_by: None,
                },
            ]
        );
        let request = String::from_utf8(server.join().expect("join loopback server"))
            .expect("UTF-8 loopback request")
            .to_ascii_lowercase();
        assert!(request.starts_with("get /api/v1/models http/1.1\r\n"));
        assert!(request.contains("authorization: bearer test-credential\r\n"));
    }

    #[test]
    fn model_list_paths_follow_the_base_shape() {
        assert_eq!(models_path_for("https://relay.example"), "/v1/models");
        assert_eq!(models_path_for("https://relay.example/"), "/v1/models");
        assert_eq!(models_path_for("https://relay.example/v1"), "/v1/models");
        assert_eq!(models_path_for("https://relay.example/v2/"), "/v2/models");
        assert_eq!(
            models_path_for("https://relay.example/api/v3"),
            "/api/v3/models"
        );
    }

    #[test]
    fn credential_headers_carry_both_ecosystems() {
        let headers = auth_headers("sk-test");
        assert!(headers.contains("Authorization: Bearer sk-test"));
        assert!(headers.contains("x-api-key: sk-test"));
        assert!(headers.contains("anthropic-version: 2023-06-01"));
    }

    #[test]
    fn models_parse_in_order_and_dedupe_with_vendor() {
        let models = parse_models(
            r#"{"object":"list","data":[
                {"id":"gpt-5.2","owned_by":"openai"},
                {"id":"gpt-5.2","owned_by":"duplicate-vendor"},
                {"id":"claude-x"},
                {"id":"blank","owned_by":"  "},
                {"id":"typed","owned_by":7},
                {}
            ]}"#,
        )
        .expect("models");
        assert_eq!(
            models,
            vec![
                ProviderModel {
                    id: "gpt-5.2".to_string(),
                    owned_by: Some("openai".to_string()),
                },
                ProviderModel {
                    id: "claude-x".to_string(),
                    owned_by: None,
                },
                ProviderModel {
                    id: "blank".to_string(),
                    owned_by: None,
                },
                ProviderModel {
                    id: "typed".to_string(),
                    owned_by: None,
                },
            ]
        );
        assert!(parse_models("[]").is_err());
        assert!(parse_models("{}").is_err());
    }
}
