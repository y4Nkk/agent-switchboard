//! The Claude Code official login: the OAuth 2.0 authorization-code flow with
//! PKCE that the Claude CLI itself uses, captured through one loopback
//! redirect. A single-use `127.0.0.1` listener receives the browser callback;
//! everything else (authorize URL, token exchange, credential write) follows
//! the same request shapes as the CLI's public client.

use std::net::TcpListener;
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;

use serde::Deserialize;
use tiny_http::{Header, Response, Server};

use crate::official_login::credentials::{
    code_challenge, random_state, random_verifier, ClaudeTokens,
};
use crate::official_login::{percent_decode, percent_encode, SESSION_EXPIRY, USER_AGENT};
use crate::probe::http_request;

pub(crate) const CLAUDE_CLIENT_ID: &str = "9d1c250a-e61b-44d9-9ed4-8d0c5bd5fae0";
/// The scopes the Claude CLI requests for a subscription login.
pub(crate) const CLAUDE_SCOPES: &str = "org:create_api_key user:profile user:inference";

const LISTEN_ERROR: &str = "无法启动本地登录监听";
const UNRECOGNIZED_RESPONSE: &str = "登录服务响应无法识别";
const EXCHANGE_REJECTED: &str = "登录凭据交换被拒绝";
const DENIED_BY_BROWSER: &str = "浏览器端拒绝了本次登录";
const STATE_MISMATCH: &str = "登录回跳校验失败，请重新开始";

const COMPLETED_PAGE: &str = "<!DOCTYPE html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><title>Agent Switchboard</title></head><body style=\"font-family:system-ui;display:grid;place-items:center;height:100vh;margin:0\"><p>登录完成，请返回应用。</p></body></html>";

/// Endpoints as a value so tests point the flow at loopback fakes.
pub(crate) struct ClaudeOAuthEndpoints {
    pub(crate) authorize_base: String,
    pub(crate) token_url: String,
}

impl Default for ClaudeOAuthEndpoints {
    fn default() -> Self {
        Self {
            authorize_base: "https://claude.ai/oauth/authorize".to_string(),
            token_url: "https://console.anthropic.com/v1/oauth/token".to_string(),
        }
    }
}

/// What the browser delivered to the loopback callback.
pub(crate) enum Callback {
    Code { code: String, state: String },
    Denied,
}

/// One started Claude login: the URL to open, the loopback listener awaiting
/// the callback, and the PKCE material bound to it. The worker thread owns
/// the only strong reference to the loopback server, so the listening socket
/// closes and tiny_http's accept thread ends by itself once the callback is
/// served or the login window expires — no caller teardown is required.
pub(crate) struct ClaudeListener {
    server: Weak<Server>,
    pub(crate) callback: Arc<Mutex<Option<Callback>>>,
    pub(crate) authorize_url: String,
    pub(crate) redirect_uri: String,
    pub(crate) verifier: String,
    pub(crate) state: String,
}

impl ClaudeListener {
    /// Wakes the worker out of its wait so a cancelled login tears the
    /// listener down immediately. After the worker exited on its own this is
    /// a no-op.
    pub(crate) fn stop(&self) {
        if let Some(server) = self.server.upgrade() {
            server.unblock();
        }
    }
}

/// Binds the loopback listener and builds the authorize URL for it.
pub(crate) fn begin(endpoints: &ClaudeOAuthEndpoints) -> Result<ClaudeListener, String> {
    let verifier = random_verifier();
    let state = random_state();
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| LISTEN_ERROR.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|_| LISTEN_ERROR.to_string())?
        .port();
    let server = Server::from_listener(listener, None).map_err(|_| LISTEN_ERROR.to_string())?;
    let redirect_uri = format!("http://localhost:{port}/callback");
    let authorize_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
        endpoints.authorize_base,
        percent_encode(CLAUDE_CLIENT_ID),
        percent_encode(&redirect_uri),
        percent_encode(CLAUDE_SCOPES),
        percent_encode(&code_challenge(&verifier)),
        percent_encode(&state),
    );

    let callback = Arc::new(Mutex::new(None));
    let server = Arc::new(server);
    let worker_server = Arc::clone(&server);
    let slot = Arc::clone(&callback);
    std::thread::Builder::new()
        .name("asb-claude-login".to_string())
        .spawn(move || serve_callback(worker_server, slot))
        .map_err(|_| LISTEN_ERROR.to_string())?;

    Ok(ClaudeListener {
        server: Arc::downgrade(&server),
        callback,
        authorize_url,
        redirect_uri,
        verifier,
        state,
    })
}

/// Takes one delivered callback, validating the OAuth state. The slot is
/// consumed either way so a failed login cannot be replayed.
pub(crate) fn consume_callback(
    callback: &Mutex<Option<Callback>>,
    expected_state: &str,
) -> Result<Option<String>, String> {
    match callback.lock().expect("callback slot").take() {
        None => Ok(None),
        Some(Callback::Denied) => Err(DENIED_BY_BROWSER.to_string()),
        Some(Callback::Code { state, .. }) if state != expected_state => {
            Err(STATE_MISMATCH.to_string())
        }
        Some(Callback::Code { code, .. }) => Ok(Some(code)),
    }
}

/// Exchanges one callback code for tokens.
pub(crate) fn exchange_code(
    endpoints: &ClaudeOAuthEndpoints,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> Result<ClaudeTokens, String> {
    let payload = serde_json::json!({
        "grant_type": "authorization_code",
        "code": code,
        "redirect_uri": redirect_uri,
        "client_id": CLAUDE_CLIENT_ID,
        "code_verifier": verifier,
    });
    let headers = format!(
        "Content-Type: application/json\r\nAccept: application/json\r\nUser-Agent: {USER_AGENT}\r\n"
    );
    let (status, body) = http_request(
        "POST",
        &endpoints.token_url,
        &headers,
        payload.to_string().as_bytes(),
    )
    .map_err(|_| EXCHANGE_REJECTED.to_string())?;
    if status != 200 {
        return Err(EXCHANGE_REJECTED.to_string());
    }
    #[derive(Deserialize)]
    struct TokenBody {
        access_token: Option<String>,
        refresh_token: Option<String>,
        expires_in: Option<i64>,
        scope: Option<String>,
    }
    let parsed: TokenBody = serde_json::from_str(&body).map_err(|_| UNRECOGNIZED_RESPONSE)?;
    let (Some(access_token), Some(refresh_token), Some(expires_in)) =
        (parsed.access_token, parsed.refresh_token, parsed.expires_in)
    else {
        return Err(UNRECOGNIZED_RESPONSE.to_string());
    };
    Ok(ClaudeTokens {
        access_token,
        refresh_token,
        expires_in,
        // A missing scope falls back to exactly what was requested.
        scope: parsed.scope.unwrap_or_else(|| CLAUDE_SCOPES.to_string()),
    })
}

/// Serves loopback requests until the callback arrives or the login window
/// closes. Exiting drops the only strong reference to the server, which
/// closes the listening socket and ends tiny_http's accept thread with it.
fn serve_callback(server: Arc<Server>, callback: Arc<Mutex<Option<Callback>>>) {
    let deadline = Instant::now() + SESSION_EXPIRY;
    loop {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return;
        };
        match server.recv_timeout(remaining) {
            Ok(Some(request)) => {
                let path = request.url().to_string();
                if let Some(query) = path.strip_prefix("/callback?") {
                    let params = query_params(query);
                    let code = params
                        .iter()
                        .find(|(key, _)| key == "code")
                        .map(|(_, value)| value.clone());
                    let state = params
                        .iter()
                        .find(|(key, _)| key == "state")
                        .map(|(_, value)| value.clone());
                    if let Some(outcome) = match (code, state) {
                        (Some(code), Some(state)) => Some(Callback::Code { code, state }),
                        _ => Some(Callback::Denied),
                    } {
                        *callback.lock().expect("callback slot") = Some(outcome);
                    }
                    let _ = request.respond(completed_response());
                    return;
                }
                let _ = request.respond(Response::from_string("not found").with_status_code(404));
            }
            // The window closed, or stop() unblocked the wait for an
            // immediate teardown on cancel.
            Ok(None) => return,
            // The listening socket closed underneath the wait.
            Err(_) => return,
        }
    }
}

fn completed_response() -> Response<std::io::Cursor<Vec<u8>>> {
    let content_type =
        Header::from_bytes("Content-Type", "text/html; charset=utf-8").expect("static header");
    Response::from_string(COMPLETED_PAGE).with_header(content_type)
}

fn query_params(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((percent_decode(key)?, percent_decode(value)?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::official_login::fake_oauth::FakeServer;

    fn endpoints(server: &FakeServer) -> ClaudeOAuthEndpoints {
        ClaudeOAuthEndpoints {
            authorize_base: format!("http://{}/authorize", server.address),
            token_url: format!("http://{}/token", server.address),
        }
    }

    /// Delivers one browser-style callback to the listener and reads the
    /// response so the connection is complete before assertions run.
    fn fetch_callback(port: u16, query: &str) -> String {
        let mut stream =
            std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect listener");
        std::io::Write::write_all(
            &mut stream,
            format!(
                "GET /callback?{query} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
            )
            .as_bytes(),
        )
        .expect("send callback");
        let mut response = String::new();
        std::io::Read::read_to_string(&mut stream, &mut response).expect("read response");
        response
    }

    fn port_of(listener: &ClaudeListener) -> u16 {
        listener
            .redirect_uri
            .strip_prefix("http://localhost:")
            .and_then(|rest| rest.strip_suffix("/callback"))
            .and_then(|port| port.parse::<u16>().ok())
            .expect("ephemeral port in redirect")
    }

    #[test]
    fn begin_builds_the_pkce_authorize_url_and_captures_one_callback() {
        let listener = begin(&ClaudeOAuthEndpoints::default()).expect("listener");
        let port = port_of(&listener);

        let url = &listener.authorize_url;
        assert!(url.starts_with("https://claude.ai/oauth/authorize?response_type=code&"));
        assert!(url.contains(&format!("client_id={}", percent_encode(CLAUDE_CLIENT_ID))));
        assert!(url.contains(&format!(
            "redirect_uri={}",
            percent_encode(&format!("http://localhost:{port}/callback"))
        )));
        assert!(url.contains(&format!("scope={}", percent_encode(CLAUDE_SCOPES))));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains(&format!("state={}", percent_encode(&listener.state))));
        assert_eq!(listener.verifier.len(), 43);

        assert_eq!(
            consume_callback(&listener.callback, &listener.state).expect("pending"),
            None
        );

        let response = fetch_callback(
            port,
            &format!("code=abc123&state={}", percent_encode(&listener.state)),
        );
        assert!(response.contains("登录完成"));

        let code = consume_callback(&listener.callback, &listener.state)
            .expect("validated callback")
            .expect("delivered code");
        assert_eq!(code, "abc123");
        listener.stop();
    }

    #[test]
    fn denied_and_state_mismatched_callbacks_fail_closed() {
        let listener = begin(&ClaudeOAuthEndpoints::default()).expect("listener");
        let port = port_of(&listener);

        fetch_callback(port, "error=access_denied");
        assert_eq!(
            consume_callback(&listener.callback, &listener.state),
            Err(DENIED_BY_BROWSER.to_string())
        );
        listener.stop();

        let listener = begin(&ClaudeOAuthEndpoints::default()).expect("listener");
        let port = port_of(&listener);
        fetch_callback(port, "code=zz&state=forged");
        assert_eq!(
            consume_callback(&listener.callback, &listener.state),
            Err(STATE_MISMATCH.to_string())
        );
        listener.stop();
    }

    #[test]
    fn the_listener_socket_closes_once_the_callback_is_served() {
        let listener = begin(&ClaudeOAuthEndpoints::default()).expect("listener");
        let port = port_of(&listener);
        fetch_callback(
            port,
            &format!("code=abc123&state={}", percent_encode(&listener.state)),
        );

        // The worker owns the only strong server reference, so serving the
        // callback ends it and closes the listening socket. Teardown races
        // the response, so refuse only counts once the port stops accepting.
        let closed = (0..100).any(|_| {
            if std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            false
        });
        assert!(closed, "listener socket must close after the callback");
    }

    #[test]
    fn exchange_posts_the_pkce_body_and_defaults_a_missing_scope() {
        let server = FakeServer::start(&[
            "{\"access_token\":\"at\",\"refresh_token\":\"rt\",\"expires_in\":3600}".to_string(),
        ]);
        let tokens = exchange_code(
            &endpoints(&server),
            "abc123",
            "the-verifier",
            "http://localhost:9/callback",
        )
        .expect("tokens");
        assert_eq!(tokens.access_token, "at");
        assert_eq!(tokens.refresh_token, "rt");
        assert_eq!(tokens.expires_in, 3600);
        assert_eq!(tokens.scope, CLAUDE_SCOPES);

        let request = server.request(0);
        assert!(request.starts_with("POST /token HTTP/1.1\r\n"));
        let sent = request.rsplit("\r\n\r\n").next().expect("json body");
        let sent: serde_json::Value = serde_json::from_str(sent).expect("json body");
        assert_eq!(sent["grant_type"], "authorization_code");
        assert_eq!(sent["code"], "abc123");
        assert_eq!(sent["code_verifier"], "the-verifier");
        assert_eq!(sent["redirect_uri"], "http://localhost:9/callback");
        assert_eq!(sent["client_id"], CLAUDE_CLIENT_ID);
    }

    #[test]
    fn rejected_exchange_and_incomplete_responses_fail_with_fixed_messages() {
        let rejected = FakeServer::start(&[":400:{\"error\":\"invalid_grant\"}".to_string()]);
        assert!(matches!(
            exchange_code(&endpoints(&rejected), "c", "v", "http://localhost:9/callback"),
            Err(message) if message == EXCHANGE_REJECTED
        ));

        let incomplete = FakeServer::start(&["{\"access_token\":\"at\"}".to_string()]);
        assert!(matches!(
            exchange_code(&endpoints(&incomplete), "c", "v", "http://localhost:9/callback"),
            Err(message) if message == UNRECOGNIZED_RESPONSE
        ));
    }
}
