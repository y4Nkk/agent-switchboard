//! Loopback HTTP boundary. It authenticates a client capability before any
//! body is forwarded, keeps upstream credentials local, and emits only
//! gateway-owned diagnostics.

mod admission;
mod claude;
mod codex;
mod codex_account;
mod codex_forward;
mod codex_reasoning;
mod compact;
mod diagnostics;
mod respond;
mod route;
pub(crate) mod transport;
pub(crate) mod websocket;

use super::http::Request;
use super::metrics::RequestSpan;
use super::transform::{convert_request, ConvertedRequest, ReasoningTransport};
use super::{constant_time_equal, content_encoding, ActiveRoute, BoundListener, GatewayInner};
use asb_core::contracts::{AppKind, UpstreamProtocol};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use tiny_http::Method;

use crate::provider_diagnostics::{ProviderDiagnostic, ProviderFailureKind};
use diagnostics::respond_diagnostic;
use route::{json_content_type, request_header};

pub(super) use respond::{
    prepare_non_stream_response, read_upstream_diagnostic, respond_error, respond_upstream,
};
pub(super) use route::{
    client_protocol, codex_request, request_capability, upstream_codex_operation_url,
    upstream_compact_url, upstream_url, CodexOperation, UpstreamClient, UpstreamResponse,
};
pub(super) use transport::send_upstream_request;

pub(super) const MAX_REQUEST_BYTES: u64 = 8 * 1024 * 1024;
pub(super) const MAX_RESPONSE_BYTES: u64 = 32 * 1024 * 1024;

/// The client behind a served path. Chat Completions is upstream-only here,
/// so it has no client and its traffic is not counted as provider traffic.
fn span_app(protocol: UpstreamProtocol) -> Option<AppKind> {
    match protocol {
        UpstreamProtocol::Responses => Some(AppKind::Codex),
        UpstreamProtocol::AnthropicMessages => Some(AppKind::Claude),
        UpstreamProtocol::ChatCompletions | UpstreamProtocol::GeminiGenerateContent => None,
    }
}

pub(super) fn serve(
    listener: BoundListener,
    inner: Arc<GatewayInner>,
    ready: SyncSender<Result<(), String>>,
) {
    super::http::serve(listener, inner, ready);
}

pub(crate) fn handle(request: Request, inner: Arc<GatewayInner>, client: Arc<UpstreamClient>) {
    let (protocol, app, codex_operation) = match admission::classify(&request) {
        Ok(kind) => kind,
        Err((protocol, status, message)) => {
            respond_error(request, protocol, status, message);
            return;
        }
    };
    let mut span = if app == AppKind::Claude {
        RequestSpan::start_with_ledger(
            Arc::clone(&inner.metrics),
            Arc::clone(&inner.claude_request_ledger),
            app,
            protocol,
        )
    } else {
        RequestSpan::start_codex(Arc::clone(&inner.metrics), &inner.state_root)
    };
    if app == AppKind::Codex
        && crate::gateway::codex::policy::pending_path(&inner.state_root).exists()
    {
        span.finish(Some(503), 0);
        respond_error(
            request,
            Some(protocol),
            503,
            "Codex 网关策略事务尚未完成，请先恢复",
        );
        return;
    }
    if !codex_operation.is_some_and(CodexOperation::is_model_list) && !json_content_type(&request) {
        span.finish(Some(415), 0);
        respond_error(
            request,
            Some(protocol),
            415,
            "请求必须使用 application/json",
        );
        return;
    }
    let Some((route, candidates)) = admission::authorized_route(&request, &inner, protocol, app)
    else {
        span.finish(Some(403), 0);
        respond_error(request, Some(protocol), 403, "本机协议网关凭据无效");
        return;
    };
    // Body-bearing Codex requests resolve their target before credentials.
    let route = if codex_operation == Some(CodexOperation::Models) {
        match codex_account::resolve(&route, &inner, Some(request.headers())) {
            Ok(route) => route,
            Err((status, message)) => {
                span.finish(Some(status), 0);
                respond_error(request, Some(protocol), status, &message);
                return;
            }
        }
    } else {
        route
    };
    span.bind_route(&route.profile_id, &route.fingerprint, route.upstream_protocol);
    if let Some(operation @ CodexOperation::Models) = codex_operation {
        if let Err(error) = codex::ensure_operation(&route, operation) {
            span.finish(Some(501), 0);
            respond_error(request, Some(protocol), 501, &error);
            return;
        }
        codex::respond_models(request, span, &route, &inner);
        return;
    }
    forward_request(
        request,
        span,
        protocol,
        route,
        candidates,
        inner,
        client,
        codex_operation,
    );
}

fn forward_request(
    mut request: Request,
    mut span: RequestSpan,
    protocol: UpstreamProtocol,
    route: ActiveRoute,
    candidates: Vec<ActiveRoute>,
    inner: Arc<GatewayInner>,
    client: Arc<UpstreamClient>,
    codex_operation: Option<CodexOperation>,
) {
    if route.app == AppKind::Claude {
        let body = match admission::read_body(&mut request) {
            Ok(body) => body,
            Err((status, message)) => {
                span.finish(Some(status), 0);
                respond_error(request, Some(protocol), status, &message);
                return;
            }
        };
        span.note_request_bytes(body.len() as u64);
        claude::forward_claude_request(request, span, protocol, candidates, inner, client, body);
    } else if let Some(operation) = codex_operation {
        codex_forward::forward_operation(
            request, span, route, candidates, inner, client, operation,
        );
    } else {
        span.finish(Some(404), 0);
        respond_error(
            request,
            Some(protocol),
            404,
            "本机协议网关不处理该 Codex 操作",
        );
    }
}

#[cfg(test)]
// Opt-in external verification harness; tests enable it per-run.
#[allow(dead_code)]
mod live_support;
