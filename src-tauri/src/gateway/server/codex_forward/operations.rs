//! Codex auxiliary operations and compaction, separate from Claude forwarding.
use super::*;

pub(in crate::gateway::server) fn forward(
    mut request: Request,
    mut span: RequestSpan,
    route: ActiveRoute,
    candidates: Vec<ActiveRoute>,
    inner: Arc<GatewayInner>,
    client: Arc<UpstreamClient>,
    operation: CodexOperation,
) {
    let (route, candidates, body) = match read_request(
        &mut request, &mut span, &inner, route, candidates, operation,
    ) {
        Ok(resolved) => resolved,
        Err((status, message)) => {
            span.finish(Some(status), 0);
            respond_error(request, Some(UpstreamProtocol::Responses), status, &message);
            return;
        }
    };
    if operation == CodexOperation::Responses
        && !crate::gateway::compaction::is_v2(&body).unwrap_or(false)
    {
        span.note_request_bytes(body.len() as u64);
        super::respond(request, span, route, candidates, inner, client, body);
        return;
    }
    let route = match super::super::codex_account::resolve(&route, &inner, Some(request.headers())) {
        Ok(route) => route,
        Err((status, message)) => {
            span.finish(Some(status), 0);
            respond_error(request, Some(UpstreamProtocol::Responses), status, &message);
            return;
        }
    };
    let url = match operation_url(
        &route,
        &inner.configured_base_url(),
        request.url(),
        operation,
    ) {
        Ok(url) => url,
        Err(message) => {
            span.finish(Some(502), 0);
            let diagnostic = ProviderDiagnostic::new(
                ProviderFailureKind::Endpoint,
                &route.upstream_base_url,
                &message,
            );
            respond_diagnostic(request, UpstreamProtocol::Responses, 502, &diagnostic);
            return;
        }
    };
    Operation {
        request,
        span,
        route,
        url,
        client,
    }
    .dispatch(operation, body, inner);
}

fn read_request(
    request: &mut Request,
    span: &mut RequestSpan,
    inner: &GatewayInner,
    route: ActiveRoute,
    candidates: Vec<ActiveRoute>,
    operation: CodexOperation,
) -> Result<(ActiveRoute, Vec<ActiveRoute>, Vec<u8>), (u16, String)> {
    let body = super::super::admission::read_body(request)?;
    span.note_request_bytes(body.len() as u64);
    span.note_request_model(crate::gateway::usage_metadata::model_from_bytes(
        UpstreamProtocol::Responses, &body,
    ));
    let resolved = super::resolve_request_route(inner, route, candidates, operation, body)?;
    span.bind_route(&resolved.0.profile_id, &resolved.0.fingerprint, resolved.0.upstream_protocol);
    Ok(resolved)
}

fn operation_url(
    route: &ActiveRoute,
    gateway: &str,
    request: &str,
    operation: CodexOperation,
) -> Result<String, String> {
    if operation.is_compact() && route.upstream_protocol == UpstreamProtocol::Responses {
        upstream_compact_url(route, gateway, request)
    } else if !operation.is_responses() && !operation.is_compact() {
        upstream_codex_operation_url(route, gateway, operation, request)
    } else {
        upstream_url(route, gateway, request)
    }
}

struct Operation {
    request: Request,
    span: RequestSpan,
    route: ActiveRoute,
    url: String,
    client: Arc<UpstreamClient>,
}
impl Operation {
    fn dispatch(mut self, operation: CodexOperation, body: Vec<u8>, inner: Arc<GatewayInner>) {
        let body =
            match super::super::codex::resolve_model_and_validate(&self.route, operation, body) {
                Ok(body) => body,
                Err(error) => {
                    self.reject(422, &error);
                    return;
                }
            };
        self.span.note_request_bytes(body.len() as u64);
        self.span
            .note_mapped_model(crate::gateway::usage_metadata::model_from_bytes(
                UpstreamProtocol::Responses,
                &body,
            ));
        if let Err((status, message)) = self
            .span
            .admit_codex_route(&self.route, operation != CodexOperation::Models)
        {
            self.reject(status, &message);
            return;
        }
        if !operation.is_responses() && !operation.is_compact() {
            self.send_operation(operation, body);
            return;
        }
        if self.route.upstream_protocol != UpstreamProtocol::Responses
            && (operation.is_compact() || crate::gateway::compaction::is_v2(&body).unwrap_or(false))
        {
            super::super::compact::respond(
                self.request,
                self.span,
                &self.route,
                &inner,
                &self.client,
                &body,
                operation.is_compact(),
            );
            return;
        }
        self.send_converted(body);
    }
    fn reject(self, status: u16, message: &str) {
        self.span.finish(Some(status), 0);
        respond_error(
            self.request,
            Some(UpstreamProtocol::Responses),
            status,
            message,
        );
    }
    fn send_converted(mut self, body: Vec<u8>) {
        let transport = ReasoningTransport::from_continuation_key(self.route.continuation_key);
        let mut converted = match convert_request(
            UpstreamProtocol::Responses,
            self.route.upstream_protocol,
            &body,
            self.route.max_output_tokens,
            Some(&transport),
            self.route
                .codex
                .as_ref()
                .map(|snapshot| &snapshot.capabilities.chat_reasoning),
        )
        .and_then(|converted| {
            crate::gateway::transform::minimal::apply(converted, self.route.responses_options)
        }) {
            Ok(converted) => converted,
            Err(error) => {
                self.span.finish(Some(422), 0);
                let diagnostic = ProviderDiagnostic::new(
                    ProviderFailureKind::RequestParameters,
                    &self.url,
                    &format!("无法转换请求：{error}"),
                );
                respond_diagnostic(self.request, UpstreamProtocol::Responses, 422, &diagnostic);
                return;
            }
        };
        let generated_beta = match crate::gateway::codex::request::prepare(
            &self.route,
            &body,
            &mut converted.body,
            Some(self.request.headers()),
        ) {
            Ok(beta) => beta,
            Err(message) => {
                self.reject(422, &message);
                return;
            }
        };
        self.span
            .note_mapped_model(crate::gateway::usage_metadata::model_from_bytes(
                self.route.upstream_protocol,
                &converted.body,
            ));
        if converted.body.len() as u64 > MAX_REQUEST_BYTES {
            self.reject(413, "转换后的请求体超过本机协议网关限制");
            return;
        }
        self.span.note_codex_attempt(&self.route, None, false);
        let version = request_header(&self.request, "anthropic-version");
        let beta = crate::gateway::codex::request::combine_beta(
            request_header(&self.request, "anthropic-beta"),
            generated_beta,
        );
        match send_upstream_request(
            &self.client,
            &self.route,
            &self.url,
            reqwest::Method::POST,
            converted.body,
            Some(self.request.headers()),
            version.as_deref(),
            beta.as_deref(),
        ) {
            Ok(upstream) => respond_upstream(
                self.request,
                self.span,
                UpstreamProtocol::Responses,
                &self.route,
                converted.stream,
                upstream,
                None,
            ),
            Err(diagnostic) => self.fail(diagnostic),
        }
    }
    fn send_operation(mut self, operation: CodexOperation, mut body: Vec<u8>) {
        if let Err(message) =
            crate::upstream_overrides::apply_body_override(&mut body, &self.route.connection)
        {
            self.reject(422, &message);
            return;
        }
        self.span
            .note_mapped_model(crate::gateway::usage_metadata::model_from_bytes(
                self.route.upstream_protocol,
                &body,
            ));
        if operation != CodexOperation::Models {
            self.span.note_codex_attempt(&self.route, None, false);
        }
        let method = if operation.is_model_list() { reqwest::Method::GET } else { reqwest::Method::POST };
        match send_upstream_request(
            &self.client,
            &self.route,
            &self.url,
            method,
            body,
            Some(self.request.headers()),
            None,
            None,
        ) {
            Ok(upstream) => super::super::respond::codex_operations::respond_passthrough(
                self.request,
                self.span,
                UpstreamProtocol::Responses,
                &self.route,
                upstream,
            ),
            Err(diagnostic) => self.fail(diagnostic),
        }
    }
    fn fail(self, diagnostic: ProviderDiagnostic) {
        self.span.finish(Some(502), 0);
        respond_diagnostic(self.request, UpstreamProtocol::Responses, 502, &diagnostic);
    }
}
