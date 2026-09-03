//! Official client login: the Codex device-code flow and the Claude OAuth
//! PKCE flow. This module owns the only sanctioned writes to the clients'
//! native credential caches (`auth.json`, `.credentials.json`); they happen
//! when the user explicitly starts or retries an official login. Tokens never
//! reach profile storage, the renderer, logs, or error messages.

pub(crate) mod claude;
pub(crate) mod codex;
pub(crate) mod credentials;

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use asb_core::contracts::AppKind;
use serde::Serialize;

/// Identifies the app to the vendor OAuth endpoints, mirroring how the
/// official CLIs send their own product user agents.
pub(crate) const USER_AGENT: &str = "agent-switchboard-official-login";

/// How long one login may stay uncompleted before poll reports a timeout.
pub(crate) const SESSION_EXPIRY: Duration = Duration::from_secs(600);

/// One in-flight official login. Tokens never live here: they exist only
/// inside a single poll call between the vendor exchange and the native
/// credential write.
pub(crate) enum LoginSession {
    Codex {
        device_auth_id: String,
        user_code: String,
        started_at: Instant,
    },
    Claude {
        listener: claude::ClaudeListener,
        started_at: Instant,
    },
}

impl LoginSession {
    /// Stops any loopback listener still waiting for a browser callback.
    pub(crate) fn stop_listener(&self) {
        if let Self::Claude { listener, .. } = self {
            listener.stop();
        }
    }

    pub(crate) fn expired(&self, now: Instant) -> bool {
        let started_at = match self {
            Self::Codex { started_at, .. } | Self::Claude { started_at, .. } => started_at,
        };
        now.duration_since(*started_at) >= SESSION_EXPIRY
    }
}

/// Per-client login sessions; one login per client runs at a time. A Claude
/// listener ends itself once its login window closes, so every entry
/// touchpoint (start, poll, cancel) can reconcile an expired leftover into a
/// clean removal without leaving threads or sockets behind.
pub(crate) fn sessions() -> &'static Mutex<BTreeMap<AppKind, LoginSession>> {
    static SESSIONS: OnceLock<Mutex<BTreeMap<AppKind, LoginSession>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub(crate) fn take_session(target: AppKind) -> Option<LoginSession> {
    sessions().lock().expect("login sessions").remove(&target)
}

/// Renderer-facing start payload: the device code to enter (Codex) or the
/// authorize URL to open (Claude).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialLoginStart {
    pub user_code: Option<String>,
    pub verification_url: String,
}

/// Renderer-facing login phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OfficialLoginPhase {
    Pending,
    Completed,
    Failed,
}

/// Renderer-facing poll result. Only status, codes, URLs, and fixed messages
/// cross this boundary — never a token or an upstream response body.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialLoginStatus {
    pub phase: OfficialLoginPhase,
    pub user_code: Option<String>,
    pub verification_url: String,
    pub message: Option<String>,
}

impl OfficialLoginStatus {
    pub(crate) fn pending(user_code: Option<String>, url: String) -> Self {
        Self {
            phase: OfficialLoginPhase::Pending,
            user_code,
            verification_url: url,
            message: None,
        }
    }

    pub(crate) fn completed() -> Self {
        Self {
            phase: OfficialLoginPhase::Completed,
            user_code: None,
            verification_url: String::new(),
            message: None,
        }
    }

    pub(crate) fn failed(message: String) -> Self {
        Self {
            phase: OfficialLoginPhase::Failed,
            user_code: None,
            verification_url: String::new(),
            message: Some(message),
        }
    }
}

/// Percent-encodes one form or query value (RFC 3986 unreserved set kept
/// literal).
pub(crate) fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}

/// Reverses [`percent_encode`]; `None` on malformed escapes or invalid UTF-8.
pub(crate) fn percent_decode(value: &str) -> Option<String> {
    fn hex_digit(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 3 <= bytes.len() => {
                let high = hex_digit(bytes[index + 1])?;
                let low = hex_digit(bytes[index + 2])?;
                decoded.push(high * 16 + low);
                index += 3;
            }
            // A truncated escape is malformed input, not a literal percent.
            b'%' => return None,
            other => {
                decoded.push(other);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
pub(crate) mod fake_oauth {
    //! A minimal sequential fake OAuth server for the loopback transport
    //! tests, mirroring the raw `TcpListener` pattern used by `probe`.

    use std::io::{Read, Write};
    use std::net::SocketAddr;
    use std::sync::mpsc;
    use std::thread::JoinHandle;

    pub(crate) struct FakeServer {
        pub(crate) address: SocketAddr,
        requests: mpsc::Receiver<String>,
        server: Option<JoinHandle<()>>,
    }

    impl FakeServer {
        /// Serves `bodies` — one canned JSON response per accepted connection,
        /// all with status 200 unless the body starts with `:status:` (for
        /// example `:404:{}`). Each handled request is recorded verbatim.
        pub(crate) fn start(bodies: &[String]) -> Self {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
            let address = listener.local_addr().expect("loopback address");
            let bodies = bodies.to_vec();
            let total = bodies.len();
            let (sender, requests) = mpsc::channel();
            let server = std::thread::spawn(move || {
                for (index, body) in bodies.iter().enumerate() {
                    let Ok((mut stream, _)) = listener.accept() else {
                        return;
                    };
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
                    let Some(request) = read_request(&mut stream) else {
                        return;
                    };
                    let (status, payload) = match body.strip_prefix(':') {
                        Some(rest) => match rest.split_once(':') {
                            Some((code, payload)) => {
                                (code.parse::<u16>().unwrap_or(200), payload.to_string())
                            }
                            None => (200, body.clone()),
                        },
                        None => (200, body.clone()),
                    };
                    let response = format!(
                        "HTTP/1.1 {status} FAKE\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
                        payload.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.flush();
                    if sender.send(request).is_err() || index + 1 == total {
                        return;
                    }
                }
            });
            Self {
                address,
                requests,
                server: Some(server),
            }
        }

        pub(crate) fn request(&self, index: usize) -> String {
            self.requests
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap_or_else(|_| panic!("fake server request {index} never arrived"))
        }
    }

    impl Drop for FakeServer {
        fn drop(&mut self) {
            // Connect once so a still-waiting accept returns and the thread
            // can exit.
            let _ = std::net::TcpStream::connect(self.address);
            if let Some(server) = self.server.take() {
                let _ = server.join();
            }
        }
    }

    fn content_length(headers: &str) -> Option<usize> {
        headers.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())
                .flatten()
        })
    }

    fn read_request(stream: &mut std::net::TcpStream) -> Option<String> {
        let mut request = Vec::new();
        let mut chunk = [0u8; 512];
        loop {
            let read = stream.read(&mut chunk).ok()?;
            if read == 0 {
                return None;
            }
            request.extend_from_slice(&chunk[..read]);
            if let Some(headers_end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..headers_end]);
                let content_length = content_length(&headers).unwrap_or(0);
                if request.len() >= headers_end + 4 + content_length {
                    return Some(String::from_utf8_lossy(&request).into_owned());
                }
            }
        }
    }

    #[test]
    fn content_length_field_name_is_case_insensitive() {
        assert_eq!(content_length("Content-Length: 42"), Some(42));
        assert_eq!(content_length("content-length: 42"), Some(42));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encoding_round_trips_the_rfc3986_unreserved_set() {
        let value = "aZ09-._~ code+value/1";
        let encoded = percent_encode(value);
        assert_eq!(encoded, "aZ09-._~%20code%2Bvalue%2F1");
        assert_eq!(percent_decode(&encoded).as_deref(), Some(value));
        assert_eq!(percent_decode("%zz"), None);
        assert_eq!(percent_decode("%2"), None);
    }

    #[test]
    fn login_sessions_expire_after_the_window() {
        let fresh = LoginSession::Codex {
            device_auth_id: "device".to_string(),
            user_code: "CODE".to_string(),
            started_at: Instant::now(),
        };
        assert!(!fresh.expired(Instant::now()));

        let stale = LoginSession::Codex {
            device_auth_id: "device".to_string(),
            user_code: "CODE".to_string(),
            started_at: Instant::now() - SESSION_EXPIRY - Duration::from_secs(1),
        };
        assert!(stale.expired(Instant::now()));
        // Stopping a session without a listener is a no-op.
        stale.stop_listener();
    }
}
