//! Codex can send Responses over WebSocket. This boundary terminates that
//! local transport, keeps its per-connection replay state private, and sends
//! only a normalized Responses request to the selected upstream protocol.

mod connections;
mod context;
mod forward;
mod request;

use self::context::{ConversationContext, PendingRequest, PreparedResponse};
use super::super::metrics::RequestSpan;
use super::super::transform::{ReasoningTransport, SseTranscoder};
use super::*;
use asb_core::contracts::ResponsesRequestMode;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::io::{Read, Write};
use std::sync::atomic::Ordering;
use tiny_http::{Header, Response, StatusCode};
use tungstenite::protocol::{Message, Role, WebSocket, WebSocketConfig};

const WEBSOCKET_ACCEPT_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

pub(crate) fn handle(
    request: Request,
    inner: Arc<GatewayInner>,
    client: Arc<UpstreamClient>,
    stop: Arc<std::sync::atomic::AtomicBool>,
) {
    if codex_request(request.url()).is_none_or(|path| !path.operation.is_responses()) {
        let endpoint = format!("{}{}", inner.configured_base_url(), request.url());
        let mut diagnostic = ProviderDiagnostic::new(
            ProviderFailureKind::WebsocketUnsupported,
            &endpoint,
            "本机协议网关不支持该 WebSocket endpoint",
        );
        diagnostic.transport = "websocket";
        diagnostic.status = Some(404);
        respond_diagnostic(request, UpstreamProtocol::Responses, 404, &diagnostic);
        return;
    }
    let mut span = RequestSpan::start_codex(Arc::clone(&inner.metrics), &inner.state_root);
    let key = match websocket_key(&request) {
        Ok(key) => key,
        Err(message) => {
            span.finish(Some(400), 0);
            respond_error(request, Some(UpstreamProtocol::Responses), 400, message);
            return;
        }
    };
    let Some((token, route)) = request_capability(&request, UpstreamProtocol::Responses)
        .and_then(|token| active_route(&inner, &token).map(|route| (token, route)))
    else {
        span.finish(Some(403), 0);
        respond_error(
            request,
            Some(UpstreamProtocol::Responses),
            403,
            "本机协议网关凭据无效",
        );
        return;
    };
    span.bind_route(
        &route.profile_id,
        &route.fingerprint,
        route.upstream_protocol,
    );
    let Some(_connection) = connections::ConnectionGuard::acquire(Arc::clone(&inner)) else {
        span.finish(Some(503), 0);
        respond_error(
            request,
            Some(UpstreamProtocol::Responses),
            503,
            "本机 WebSocket 连接数已满",
        );
        return;
    };
    upgrade_and_serve(
        request,
        inner,
        client,
        token,
        key,
        ConversationContext::for_route(&route.fingerprint),
        stop,
        span,
    );
}

fn upgrade_and_serve(
    request: Request,
    inner: Arc<GatewayInner>,
    client: Arc<UpstreamClient>,
    token: String,
    key: String,
    context: ConversationContext,
    stop: Arc<std::sync::atomic::AtomicBool>,
    span: RequestSpan,
) {
    let response = Response::empty(StatusCode(101)).with_header(
        Header::from_bytes(
            &b"Sec-WebSocket-Accept"[..],
            websocket_accept(&key).as_bytes(),
        )
        .expect("valid WebSocket accept header"),
    );
    let request_url = request.url().to_string();
    // The HTTP request is consumed by the upgrade. Keep the handshake facts
    // that a native Responses upstream may need for this connection.
    let request_headers = request.headers().to_vec();
    let stream = match request.upgrade("websocket", response) {
        Ok(stream) => stream,
        Err(_) => {
            span.finish(None, 0);
            log::debug!("本机 WebSocket 升级失败");
            return;
        }
    };
    let config = WebSocketConfig {
        write_buffer_size: 0,
        max_write_buffer_size: MAX_REQUEST_BYTES as usize + 1,
        max_message_size: Some(MAX_REQUEST_BYTES as usize),
        max_frame_size: Some(MAX_REQUEST_BYTES as usize),
        ..WebSocketConfig::default()
    };
    let socket = WebSocket::from_raw_socket(stream, Role::Server, Some(config));
    span.finish(Some(101), 0);
    serve_connection(
        socket,
        inner,
        client,
        token,
        context,
        request_url,
        request_headers,
        stop,
    );
}

fn serve_connection<S>(
    mut socket: WebSocket<S>,
    inner: Arc<GatewayInner>,
    client: Arc<UpstreamClient>,
    token: String,
    mut context: ConversationContext,
    request_url: String,
    request_headers: Vec<Header>,
    stop: Arc<std::sync::atomic::AtomicBool>,
) where
    S: Read + Write,
{
    while !inner.stopping.load(Ordering::Acquire) && !stop.load(Ordering::Acquire) {
        let message = match socket.read() {
            Ok(message) => message,
            Err(_) => break,
        };
        match message {
            Message::Text(text) => {
                let Some(_guard) = crate::gateway::InflightGuard::try_acquire(Arc::clone(&inner))
                else {
                    let _ = send_failed(
                        &mut socket,
                        "gateway_unavailable",
                        "网关正在切换或请求已满；请稍后重试",
                    );
                    break;
                };
                let mut span =
                    RequestSpan::start_codex(Arc::clone(&inner.metrics), &inner.state_root);
                let outcome = serve_message(
                    &mut socket,
                    &client,
                    &inner,
                    &token,
                    &request_url,
                    &request_headers,
                    &mut context,
                    &text,
                    &mut span,
                );
                span.finish(outcome.completed_status(), 0);
                if outcome == ExchangeOutcome::Disconnect {
                    break;
                }
            }
            Message::Ping(_) | Message::Pong(_) => {
                if socket.flush().is_err() {
                    break;
                }
            }
            Message::Close(_) => {
                let _ = socket.flush();
                break;
            }
            Message::Binary(_) | Message::Frame(_) => {
                if !send_failed(
                    &mut socket,
                    "invalid_request",
                    "Codex WebSocket 请求必须是文本消息",
                ) {
                    break;
                }
            }
        }
    }
    let _ = socket.close(None);
}

/// What happened to one WebSocket message exchange. `Served` delivered a
/// complete answer, `Rejected` delivered a failure frame and keeps the
/// connection, `Disconnect` means the socket died around the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExchangeOutcome {
    Served,
    Incomplete,
    Rejected,
    Disconnect,
}

impl ExchangeOutcome {
    /// The HTTP-style status for telemetry: `None` marks every exchange that
    /// did not deliver a complete converted answer.
    fn completed_status(self) -> Option<u16> {
        match self {
            Self::Served => Some(200),
            Self::Incomplete | Self::Rejected | Self::Disconnect => None,
        }
    }
}

fn serve_message<S>(
    socket: &mut WebSocket<S>,
    client: &UpstreamClient,
    inner: &GatewayInner,
    token: &str,
    request_url: &str,
    request_headers: &[Header],
    context: &mut ConversationContext,
    text: &str,
    span: &mut RequestSpan,
) -> ExchangeOutcome
where
    S: Read + Write,
{
    if let Some(outcome) = reject_unavailable(socket, inner, text) {
        return outcome;
    }
    let Some(route) = active_route(inner, token) else {
        let _ = send_failed(
            socket,
            "route_inactive",
            "本机协议网关路由已失效；请重新连接后重试",
        );
        return ExchangeOutcome::Disconnect;
    };
    span.bind_route(
        &route.profile_id,
        &route.fingerprint,
        route.upstream_protocol,
    );
    span.note_request_bytes(text.len() as u64);
    span.note_request_model(crate::gateway::usage_metadata::model_from_bytes(
        UpstreamProtocol::Responses,
        text.as_bytes(),
    ));
    let resolved = match request::prepare(inner, route.clone(), context, text) {
        Ok(resolved) => resolved,
        Err((code, message)) => {
            let diagnostic = request_diagnostic(inner, &route, request_url, &message);
            return reject_context(socket, &diagnostic, &code);
        }
    };
    let route = resolved.route;
    span.bind_route(&route.profile_id, &route.fingerprint, route.upstream_protocol);
    match resolved.prepared {
        PreparedResponse::Prewarm(response) => {
            if send_created_and_completed(socket, &response.value) {
                ExchangeOutcome::Served
            } else {
                ExchangeOutcome::Disconnect
            }
        }
        PreparedResponse::Upstream(request) => forward::execute(
            socket,
            client,
            inner,
            &route,
            resolved.candidates,
            context,
            request,
            request_url,
            request_headers,
            span,
        ),
    }
}

fn reject_unavailable<S: Read + Write>(
    socket: &mut WebSocket<S>,
    inner: &GatewayInner,
    text: &str,
) -> Option<ExchangeOutcome> {
    if text.len() as u64 > MAX_REQUEST_BYTES {
        return Some(
            if send_failed(socket, "request_too_large", "请求体超过本机协议网关限制") {
                ExchangeOutcome::Rejected
            } else {
                ExchangeOutcome::Disconnect
            },
        );
    }
    if crate::gateway::codex::policy::pending_path(&inner.state_root).exists() {
        return Some(
            if send_failed(
                socket,
                "codex_policy_recovery_required",
                "Codex 网关策略事务尚未完成，请先恢复",
            ) {
                ExchangeOutcome::Rejected
            } else {
                ExchangeOutcome::Disconnect
            },
        );
    }
    None
}

fn request_diagnostic(
    inner: &GatewayInner,
    route: &ActiveRoute,
    request_url: &str,
    message: &str,
) -> ProviderDiagnostic {
    let endpoint = upstream_url(route, &inner.configured_base_url(), request_url)
        .unwrap_or_else(|_| route.upstream_base_url.clone());
    ProviderDiagnostic::new(ProviderFailureKind::RequestParameters, &endpoint, message)
}

fn active_route(inner: &GatewayInner, token: &str) -> Option<ActiveRoute> {
    inner
        .routes
        .read()
        .ok()?
        .get(&AppKind::Codex)
        .filter(|route| constant_time_equal(route.client_token.as_bytes(), token.as_bytes()))
        .cloned()
}

mod decode;
mod exchange;
mod handshake;
mod send;
mod stream;

use decode::*;
use handshake::*;
use send::*;
