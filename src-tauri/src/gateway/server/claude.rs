//! Claude routing, retries and health completion. No Codex operations enter here.

mod accounts;
mod attempt;
mod query;
mod response;
use super::*;
use crate::gateway::GatewayController;
use crate::gateway::failover::{self, ClaudeFailoverPolicy};
use crate::gateway::provider_health::RequestPermit;
use crate::gateway::request_ledger::{ClaudeFailoverAttempt, ClaudeFailoverOutcome};
use crate::gateway::usage_metadata::model_from_value;
use std::sync::atomic::Ordering;

pub(super) fn forward_claude_request(
    request: Request,
    mut span: RequestSpan,
    protocol: UpstreamProtocol,
    candidates: Vec<ActiveRoute>,
    inner: Arc<GatewayInner>,
    client: Arc<UpstreamClient>,
    body: Vec<u8>,
) {
    let input = read_input(&inner, protocol, &body);
    let (mut policy, model) = match input {
        Ok(input) => input,
        Err((status, error)) => {
            span.finish(Some(status), 0);
            respond_error(request, Some(protocol), status, &error);
            return;
        }
    };
    let controller = GatewayController { inner: inner.clone() };
    let (candidates, body, fixed) = match controller.resolve_claude_target(candidates, body) {
        Ok(resolved) => resolved,
        Err(error) => {
            span.finish(Some(422), 0);
            respond_error(request, Some(protocol), 422, &error);
            return;
        }
    };
    if fixed { policy.enabled = false; }
    if let Err(error) = crate::gateway::claude_targets::validate_candidate_roles(&candidates) {
        span.finish(Some(422), 0);
        respond_error(request, Some(protocol), 422, &error);
        return;
    }
    span.note_request_model(model);
    Forwarding {
        request,
        span,
        protocol,
        candidates,
        inner,
        client,
        body,
        policy,
    }
    .run();
}

fn read_input(
    inner: &GatewayInner,
    protocol: UpstreamProtocol,
    body: &[u8],
) -> Result<(ClaudeFailoverPolicy, Option<String>), (u16, String)> {
    let policy = failover::load(&inner.state_root).map_err(|error| (503, error))?;
    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| (422, "Claude 请求体必须是 JSON 对象".into()))?;
    if !value.is_object() {
        return Err((422, "Claude 请求体必须是 JSON 对象".into()));
    }
    Ok((policy, model_from_value(protocol, &value)))
}

struct Forwarding {
    request: Request,
    span: RequestSpan,
    protocol: UpstreamProtocol,
    candidates: Vec<ActiveRoute>,
    inner: Arc<GatewayInner>,
    client: Arc<UpstreamClient>,
    body: Vec<u8>,
    policy: ClaudeFailoverPolicy,
}

impl Forwarding {
    fn run(mut self) {
        let routes = request_candidates(std::mem::take(&mut self.candidates), &self.policy);
        let limit = self.policy.effective_attempts();
        let mut attempted = 0;
        let mut last_failure = None;
        for candidate in routes.iter().cycle().take(routes.len() * limit) {
            if self.request.is_cancelled() {
                self.span.finish(None, 0);
                return;
            }
            if attempted >= limit {
                break;
            }
            let health = self
                .inner
                .health_for_config(candidate, self.policy.traffic.health_config());
            let Some(permit) = health.try_acquire() else {
                continue;
            };
            attempted += 1;
            self.bind(candidate);
            let result = attempt::send(
                &self.request,
                &mut self.span,
                candidate,
                &self.inner,
                &self.client,
                &self.body,
                &self.policy.traffic,
            );
            match result {
                Ok((reply, effective)) => {
                    self.respond(&effective, permit, reply);
                    return;
                }
                Err(failure) => {
                    if self.request.is_cancelled() {
                        drop(permit);
                        self.span.finish(None, 0);
                        return;
                    }
                    let retry = self.failed(candidate, permit, &failure, attempted < limit);
                    last_failure = Some(failure);
                    if !retry {
                        break;
                    }
                }
            }
        }
        self.finish(last_failure);
    }

    fn bind(&mut self, route: &ActiveRoute) {
        self.span.bind_route(
            &route.profile_id,
            &route.fingerprint,
            route.upstream_protocol,
        );
        self.span.bind_claude_billing(
            &self.inner.state_root,
            route.connection.claude_billing.as_ref(),
        );
        self.span
            .protect_secrets(&[&route.api_key, &route.client_token]);
    }

    fn respond(
        mut self,
        route: &ActiveRoute,
        permit: RequestPermit<'_>,
        reply: response::Prepared,
    ) {
        note_attempt(
            &mut self.span,
            route,
            Some(200),
            ClaudeFailoverOutcome::Success,
        );
        let outcome = self.span.observe_completion();
        reply.respond(self.request, self.span, route);
        match outcome.load(Ordering::Acquire) {
            200..=299 => {
                permit.record_success();
                self.inner.record_endpoint_success(route);
            }
            0 => drop(permit),
            status if crate::gateway::is_retryable_http_status(status) => permit.record_failure(),
            _ => drop(permit),
        }
    }

    fn failed(
        &mut self,
        route: &ActiveRoute,
        permit: RequestPermit<'_>,
        failure: &attempt::Failure,
        another_attempt: bool,
    ) -> bool {
        if failure.penalize {
            permit.record_failure();
        } else {
            drop(permit);
        }
        let retry = failure.retryable && another_attempt;
        note_attempt(
            &mut self.span,
            route,
            Some(failure.status),
            if retry {
                ClaudeFailoverOutcome::RetryableFailure
            } else {
                ClaudeFailoverOutcome::Failure
            },
        );
        retry
    }

    fn finish(self, failure: Option<attempt::Failure>) {
        match failure {
            Some(failure) => {
                self.span.finish(Some(failure.status), 0);
                respond_diagnostic(
                    self.request,
                    self.protocol,
                    failure.status,
                    &failure.diagnostic,
                );
            }
            None => {
                self.span.finish(Some(503), 0);
                respond_error(
                    self.request,
                    Some(self.protocol),
                    503,
                    "Claude 供应商均处于熔断冷却状态",
                );
            }
        }
    }
}

fn note_attempt(
    span: &mut RequestSpan,
    route: &ActiveRoute,
    status: Option<u16>,
    outcome: ClaudeFailoverOutcome,
) {
    span.note_failover_attempt(ClaudeFailoverAttempt {
        profile_id: route.profile_id.clone(),
        route_revision: route.fingerprint.clone(),
        upstream_protocol: route.upstream_protocol,
        status,
        outcome,
    });
}

fn request_candidates(routes: Vec<ActiveRoute>, policy: &ClaudeFailoverPolicy) -> Vec<ActiveRoute> {
    let Some(selected) = routes.first() else {
        return Vec::new();
    };
    let available = routes
        .iter()
        .map(|route| route.profile_id.clone())
        .collect::<Vec<_>>();
    policy
        .candidate_provider_ids(&selected.profile_id, &available)
        .into_iter()
        .flat_map(|id| {
            routes
                .iter()
                .filter(move |route| route.profile_id == id)
                .cloned()
        })
        .collect()
}
