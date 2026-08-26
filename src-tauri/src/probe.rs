//! Manual endpoint probing over the Windows-native WinHTTP stack.
//!
//! One probe answers exactly one question: does this URL answer HTTP(S)
//! requests, with which status, and how fast. It never selects or switches
//! anything. Using WinHTTP keeps the probe free of third-party TLS
//! dependencies; the system verifier and proxy settings apply.

use serde::Serialize;
use std::time::Instant;

/// Result of one manual endpoint probe, surfaced to the UI as-is.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub url: String,
    pub reachable: bool,
    pub status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
    /// RFC 3339 UTC timestamp of the probe.
    pub at: String,
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

fn head_requires_get(status: u16) -> bool {
    matches!(status, 405 | 501)
}

/// Sends one request with `verb` and returns the HTTP status code.
unsafe fn request_status(
    connect: *mut core::ffi::c_void,
    verb: &str,
    path: &str,
    secure: bool,
) -> Result<u16, String> {
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
        return Err(last_error());
    }
    let outcome = (|| {
        if WinHttpSendRequest(request, std::ptr::null(), 0, std::ptr::null(), 0, 0, 0) == 0 {
            return Err(last_error());
        }
        if WinHttpReceiveResponse(request, std::ptr::null_mut()) == 0 {
            return Err(last_error());
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
            return Err(last_error());
        }
        Ok(status as u16)
    })();
    WinHttpCloseHandle(request);
    outcome
}

/// Probes one URL with HEAD, falling back to GET when the endpoint rejects
/// HEAD. Any HTTP answer counts as reachable, whatever the status says.
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
            return Err(last_error());
        }
        let result = (|| {
            if WinHttpSetTimeouts(session, 5_000, 5_000, 10_000, 10_000) == 0 {
                return Err(last_error());
            }
            let host_w = wide(&parsed.host);
            let connect = WinHttpConnect(session, host_w.as_ptr(), parsed.port, 0);
            if connect.is_null() {
                return Err(last_error());
            }
            let started = Instant::now();
            let status = match request_status(connect, "HEAD", &parsed.path, parsed.secure) {
                Ok(status) if head_requires_get(status) => {
                    // Some gateways reject HEAD outright; GET still proves
                    // reachability and TLS.
                    request_status(connect, "GET", &parsed.path, parsed.secure)
                }
                Ok(status) => Ok(status),
                Err(_) => request_status(connect, "GET", &parsed.path, parsed.secure),
            };
            let latency_ms = Some(started.elapsed().as_millis() as u64);
            WinHttpCloseHandle(connect);
            match status {
                Ok(status) => Ok(ProbeResult {
                    url: url.to_string(),
                    reachable: true,
                    status: Some(status),
                    latency_ms,
                    error: None,
                    at,
                }),
                Err(_) => Ok(ProbeResult {
                    url: url.to_string(),
                    reachable: false,
                    status: None,
                    latency_ms,
                    error: Some("无法连接端点，请检查地址、代理和网络".to_string()),
                    at,
                }),
            }
        })();
        WinHttpCloseHandle(session);
        result
    }
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
    fn rejected_head_statuses_fall_back_to_get() {
        assert!(head_requires_get(405));
        assert!(head_requires_get(501));
        assert!(!head_requires_get(200));
        assert!(!head_requires_get(404));
    }
}
