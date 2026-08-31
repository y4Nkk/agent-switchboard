//! Manual endpoint probing over the Windows-native WinHTTP stack.
//!
//! One probe answers exactly one question: does this URL answer HTTP(S)
//! requests, with which status, and how fast. It never selects or switches
//! anything. Using WinHTTP keeps the probe free of third-party TLS
//! dependencies; the system verifier and proxy settings apply. The shared
//! [`http_get`] helper carries that same stack to other outbound reads
//! (model fetch, update check).

use serde::Serialize;
use std::time::Instant;

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

/// A failed WinHTTP call, keeping the error code so retry decisions can
/// distinguish timeout-class jitter from immediate failures.
#[derive(Debug, PartialEq)]
struct ProbeError {
    code: u32,
    message: String,
}

impl ProbeError {
    fn is_timeout(&self) -> bool {
        self.code == windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_TIMEOUT
    }
}

/// Maps a WinHTTP last-error code to the failure class the user can act on.
fn failure_message(code: u32) -> &'static str {
    use windows_sys::Win32::Networking::WinHttp::{
        ERROR_WINHTTP_CANNOT_CONNECT, ERROR_WINHTTP_NAME_NOT_RESOLVED,
        ERROR_WINHTTP_SECURE_CHANNEL_ERROR, ERROR_WINHTTP_TIMEOUT,
    };
    match code {
        ERROR_WINHTTP_NAME_NOT_RESOLVED => "域名解析失败（DNS）",
        ERROR_WINHTTP_TIMEOUT => "连接超时",
        ERROR_WINHTTP_CANNOT_CONNECT => "连接被拒绝",
        ERROR_WINHTTP_SECURE_CHANNEL_ERROR => "TLS 握手失败",
        _ => "网络请求失败",
    }
}

unsafe fn winhttp_failure() -> ProbeError {
    let code = windows_sys::Win32::Foundation::GetLastError();
    ProbeError {
        code,
        message: failure_message(code).to_string(),
    }
}

/// Probe retry policy, separated from the WinHTTP layer for testing:
/// timeout-class failures get exactly one retry (network jitter), immediate
/// failures such as refused connections or DNS errors fail fast.
fn probe_with_retries(
    mut attempt: impl FnMut() -> Result<u16, ProbeError>,
) -> Result<u16, ProbeError> {
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

/// Splits a validated `http(s)://` URL into WinHTTP's connect/open pieces.
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

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn last_error() -> String {
    "系统网络请求失败".to_string()
}

/// Sends one GET and returns the HTTP status code; the response body is never
/// read, so returning here means the endpoint answered.
unsafe fn request_status(
    connect: *mut core::ffi::c_void,
    verb: &str,
    path: &str,
    secure: bool,
) -> Result<u16, ProbeError> {
    use windows_sys::Win32::Networking::WinHttp::{
        WinHttpCloseHandle, WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReceiveResponse,
        WinHttpSendRequest, WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER,
        WINHTTP_QUERY_STATUS_CODE,
    };

    let verb_w = wide(verb);
    let path_w = wide(path);
    let flags = if secure { WINHTTP_FLAG_SECURE } else { 0 };
    let request = WinHttpOpenRequest(
        connect,
        verb_w.as_ptr(),
        path_w.as_ptr(),
        std::ptr::null(),
        std::ptr::null(),
        std::ptr::null(),
        flags,
    );
    if request.is_null() {
        return Err(winhttp_failure());
    }
    let outcome = (|| {
        if WinHttpSendRequest(request, std::ptr::null(), 0, std::ptr::null(), 0, 0, 0) == 0 {
            return Err(winhttp_failure());
        }
        if WinHttpReceiveResponse(request, std::ptr::null_mut()) == 0 {
            return Err(winhttp_failure());
        }
        let mut status: u32 = 0;
        let mut length = std::mem::size_of::<u32>() as u32;
        let queried = WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            &mut status as *mut u32 as *mut core::ffi::c_void,
            &mut length,
            std::ptr::null_mut(),
        );
        if queried == 0 {
            return Err(winhttp_failure());
        }
        Ok(status as u16)
    })();
    WinHttpCloseHandle(request);
    outcome
}

/// Probes one URL for reachability. Any HTTP answer counts — the status code
/// (200/4xx/5xx alike) only proves the endpoint is alive; the probe sends no
/// model request and carries no credential, so it never validates
/// authentication or model configuration.
pub fn probe(url: &str) -> Result<ProbeResult, String> {
    let parsed = parse_url(url).ok_or_else(|| "端点必须是 http(s) URL".to_string())?;
    let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    use windows_sys::Win32::Networking::WinHttp::{
        WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpSetTimeouts,
        WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    };

    let agent_w = wide("Agent Switchboard probe");
    unsafe {
        let session = WinHttpOpen(
            agent_w.as_ptr(),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            std::ptr::null(),
            std::ptr::null(),
            0,
        );
        if session.is_null() {
            return Err("系统网络组件初始化失败".to_string());
        }
        let result = (|| {
            if WinHttpSetTimeouts(session, 5_000, 5_000, 10_000, 10_000) == 0 {
                return Err("系统网络组件配置失败".to_string());
            }
            let host_w = wide(&parsed.host);
            let connect = WinHttpConnect(session, host_w.as_ptr(), parsed.port, 0);
            if connect.is_null() {
                return Err("无法建立网络连接".to_string());
            }
            let started = Instant::now();
            let outcome =
                probe_with_retries(|| request_status(connect, "GET", &parsed.path, parsed.secure));
            let latency_ms = Some(started.elapsed().as_millis() as u64);
            WinHttpCloseHandle(connect);
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
        })();
        WinHttpCloseHandle(session);
        result
    }
}

/// One WinHTTP request returning the response status and body text. The
/// agent string doubles as the required `User-Agent`; callers pass extra
/// request headers as CRLF-joined lines.
pub fn http_request(
    method: &str,
    url: &str,
    headers: &str,
    body: &[u8],
) -> Result<(u16, String), String> {
    let parsed = parse_url(url).ok_or_else(|| "请求地址必须是 http(s) URL".to_string())?;

    use windows_sys::Win32::Networking::WinHttp::{
        WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders,
        WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
        WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER,
        WINHTTP_QUERY_STATUS_CODE,
    };

    let agent_w = wide("Agent Switchboard");
    unsafe {
        let session = WinHttpOpen(
            agent_w.as_ptr(),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            std::ptr::null(),
            std::ptr::null(),
            0,
        );
        if session.is_null() {
            return Err(last_error());
        }
        let result = (|| {
            if WinHttpSetTimeouts(session, 5_000, 5_000, 15_000, 15_000) == 0 {
                return Err(last_error());
            }
            let host_w = wide(&parsed.host);
            let connect = WinHttpConnect(session, host_w.as_ptr(), parsed.port, 0);
            if connect.is_null() {
                return Err(last_error());
            }
            let outcome = (|| {
                let flags = if parsed.secure {
                    WINHTTP_FLAG_SECURE
                } else {
                    0
                };
                let request = WinHttpOpenRequest(
                    connect,
                    wide(method).as_ptr(),
                    wide(&parsed.path).as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    flags,
                );
                if request.is_null() {
                    return Err(last_error());
                }
                let headers_w = wide(headers);
                let (body_ptr, body_len) = if body.is_empty() {
                    (std::ptr::null(), 0)
                } else {
                    (body.as_ptr().cast(), body.len() as u32)
                };
                let outcome = (|| {
                    if WinHttpSendRequest(
                        request,
                        headers_w.as_ptr(),
                        headers_w.len() as u32,
                        body_ptr,
                        body_len,
                        body_len,
                        0,
                    ) == 0
                    {
                        return Err(last_error());
                    }
                    if WinHttpReceiveResponse(request, std::ptr::null_mut()) == 0 {
                        return Err(last_error());
                    }
                    let mut status: u32 = 0;
                    let mut length = std::mem::size_of::<u32>() as u32;
                    if WinHttpQueryHeaders(
                        request,
                        WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                        std::ptr::null(),
                        &mut status as *mut u32 as *mut core::ffi::c_void,
                        &mut length,
                        std::ptr::null_mut(),
                    ) == 0
                    {
                        return Err(last_error());
                    }
                    let body = read_body(request)?;
                    Ok((status as u16, String::from_utf8_lossy(&body).into_owned()))
                })();
                WinHttpCloseHandle(request);
                outcome
            })();
            WinHttpCloseHandle(connect);
            outcome
        })();
        WinHttpCloseHandle(session);
        result
    }
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

/// Reads the full response body of an open request handle.
unsafe fn read_body(request: *mut core::ffi::c_void) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Networking::WinHttp::{WinHttpQueryDataAvailable, WinHttpReadData};
    let mut body = Vec::new();
    loop {
        let mut available: u32 = 0;
        if WinHttpQueryDataAvailable(request, &mut available) == 0 {
            return Err(last_error());
        }
        if available == 0 {
            return Ok(body);
        }
        let mut chunk = vec![0u8; available as usize];
        let mut read: u32 = 0;
        if WinHttpReadData(request, chunk.as_mut_ptr().cast(), available, &mut read) == 0 {
            return Err(last_error());
        }
        chunk.truncate(read as usize);
        body.extend_from_slice(&chunk);
    }
}

/// Fetches the OpenAI-compatible `GET /v1/models` list from a provider base
/// URL and returns the model ids in server order, deduplicated. The profile
/// API key travels only in request headers and is never logged or echoed in
/// errors. Nothing is cached.
pub fn fetch_models(base_url: &str, api_key: &str, _source: &str) -> Result<Vec<String>, String> {
    let parsed = parse_url(base_url).ok_or_else(|| "服务地址必须是 http(s) URL".to_string())?;
    let path = models_path_for(base_url);

    use windows_sys::Win32::Networking::WinHttp::{
        WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpSetTimeouts,
        WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    };

    let agent_w = wide("Agent Switchboard model fetch");
    unsafe {
        let session = WinHttpOpen(
            agent_w.as_ptr(),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            std::ptr::null(),
            std::ptr::null(),
            0,
        );
        if session.is_null() {
            return Err(last_error());
        }
        let result = (|| {
            if WinHttpSetTimeouts(session, 5_000, 5_000, 15_000, 15_000) == 0 {
                return Err(last_error());
            }
            let host_w = wide(&parsed.host);
            let connect = WinHttpConnect(session, host_w.as_ptr(), parsed.port, 0);
            if connect.is_null() {
                return Err(last_error());
            }
            let outcome = (|| {
                use windows_sys::Win32::Networking::WinHttp::{
                    WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReceiveResponse,
                    WinHttpSendRequest, WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER,
                    WINHTTP_QUERY_STATUS_CODE,
                };
                let path_w = wide(&path);
                let flags = if parsed.secure {
                    WINHTTP_FLAG_SECURE
                } else {
                    0
                };
                let request = WinHttpOpenRequest(
                    connect,
                    wide("GET").as_ptr(),
                    path_w.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    flags,
                );
                if request.is_null() {
                    return Err(last_error());
                }
                let headers_w = wide(&auth_headers(api_key));
                let headers_ptr = headers_w.as_ptr();
                let headers_len = headers_w.len() as u32;
                let outcome = (|| {
                    if WinHttpSendRequest(
                        request,
                        headers_ptr,
                        headers_len,
                        std::ptr::null(),
                        0,
                        0,
                        0,
                    ) == 0
                    {
                        return Err(last_error());
                    }
                    if WinHttpReceiveResponse(request, std::ptr::null_mut()) == 0 {
                        return Err(last_error());
                    }
                    let mut status: u32 = 0;
                    let mut length = std::mem::size_of::<u32>() as u32;
                    if WinHttpQueryHeaders(
                        request,
                        WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                        std::ptr::null(),
                        &mut status as *mut u32 as *mut core::ffi::c_void,
                        &mut length,
                        std::ptr::null_mut(),
                    ) == 0
                    {
                        return Err(last_error());
                    }
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
                    let body = read_body(request)?;
                    let text = String::from_utf8_lossy(&body);
                    parse_model_ids(&text)
                })();
                WinHttpCloseHandle(request);
                outcome
            })();
            WinHttpCloseHandle(connect);
            outcome
        })();
        WinHttpCloseHandle(session);
        result
    }
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

/// Extracts `data[].id` from an OpenAI-compatible models response.
fn parse_model_ids(text: &str) -> Result<Vec<String>, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| "模型列表响应不是有效 JSON".to_string())?;
    let data = value
        .get("data")
        .and_then(|data| data.as_array())
        .ok_or_else(|| "模型列表响应缺少 data 数组".to_string())?;
    let mut ids = Vec::new();
    for entry in data {
        if let Some(id) = entry.get("id").and_then(|id| id.as_str()) {
            if !id.is_empty() && !ids.iter().any(|seen: &String| seen == id) {
                ids.push(id.to_string());
            }
        }
    }
    if ids.is_empty() {
        return Err("模型列表为空".to_string());
    }
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn timeout_failure() -> ProbeError {
        ProbeError {
            code: windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_TIMEOUT,
            message: failure_message(
                windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_TIMEOUT,
            )
            .to_string(),
        }
    }

    fn refused_failure() -> ProbeError {
        ProbeError {
            code: windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_CANNOT_CONNECT,
            message: failure_message(
                windows_sys::Win32::Networking::WinHttp::ERROR_WINHTTP_CANNOT_CONNECT,
            )
            .to_string(),
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
    fn failure_codes_map_to_actionable_classes() {
        use windows_sys::Win32::Networking::WinHttp::{
            ERROR_WINHTTP_CANNOT_CONNECT, ERROR_WINHTTP_NAME_NOT_RESOLVED,
            ERROR_WINHTTP_SECURE_CHANNEL_ERROR, ERROR_WINHTTP_TIMEOUT,
        };
        assert_eq!(failure_message(ERROR_WINHTTP_TIMEOUT), "连接超时");
        assert_eq!(
            failure_message(ERROR_WINHTTP_NAME_NOT_RESOLVED),
            "域名解析失败（DNS）"
        );
        assert_eq!(failure_message(ERROR_WINHTTP_CANNOT_CONNECT), "连接被拒绝");
        assert_eq!(
            failure_message(ERROR_WINHTTP_SECURE_CHANNEL_ERROR),
            "TLS 握手失败"
        );
        assert_eq!(failure_message(0), "网络请求失败");
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
    fn model_ids_parse_in_order_and_dedupe() {
        let ids = parse_model_ids(
            r#"{"object":"list","data":[{"id":"gpt-5.2"},{"id":"gpt-5.2"},{"id":"claude-x"},{}]}"#,
        )
        .expect("ids");
        assert_eq!(ids, vec!["gpt-5.2".to_string(), "claude-x".to_string()]);
        assert!(parse_model_ids("[]").is_err());
        assert!(parse_model_ids("{}").is_err());
    }
}
