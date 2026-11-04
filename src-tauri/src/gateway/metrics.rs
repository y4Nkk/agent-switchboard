//! The loopback gateway's in-memory status samples and per-request observation span.
//! Spans also feed the existing client-owned durable ledgers; no credentials enter
//! either metadata surface.

use asb_core::contracts::{AppKind, UpstreamProtocol};
use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super::request_ledger::{
    ClaudeFailoverAttempt, ClaudeFailoverOutcome, ClaudeRequestLedger, ClaudeRequestRecord,
};

use super::usage_metadata::{metadata_from_value, TokenUsage};
mod claude;
mod codex;

/// Bounded recent-request buffer. Older samples are dropped once full; the
/// cumulative counters remain exact so totals never silently lag the chart.
const SAMPLE_CAPACITY: usize = 512;

/// One completed gateway request, recorded exactly once at the boundary that
/// answered the client. `status` is the final accounting status: upstream HTTP
/// status unless projected to partial output (206), logical/protocol failure
/// (502), or client disconnection (499). WebSocket upgrades use 101.
/// `profile_id`, `route_revision`, and `upstream_protocol` are set together
/// only when the capability token matched an immutable route snapshot.
/// Rejected or unmatched traffic stays unattributed instead of wearing
/// placeholder values.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GatewaySample {
    pub(crate) at_ms: u64,
    pub(crate) app: AppKind,
    pub(crate) profile_id: Option<String>,
    /// The exact immutable route revision that accepted this request. This
    /// lets the status UI distinguish traffic accepted before and after a
    /// hot switch of the same provider profile.
    pub(crate) route_revision: Option<String>,
    pub(crate) client_protocol: UpstreamProtocol,
    pub(crate) upstream_protocol: Option<UpstreamProtocol>,
    pub(crate) status: Option<u16>,
    pub(crate) duration_ms: u64,
    pub(crate) request_bytes: u64,
    pub(crate) response_bytes: u64,
}

impl GatewaySample {
    fn failed(&self) -> bool {
        self.status.is_none_or(|status| status == 206 || status >= 400)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GatewayMetricsSnapshot {
    pub(crate) started_at_ms: u64,
    pub(crate) total_requests: u64,
    pub(crate) failed_requests: u64,
    pub(crate) samples: Vec<GatewaySample>,
}

pub(crate) struct GatewayMetrics {
    started_at_ms: u64,
    total_requests: AtomicU64,
    failed_requests: AtomicU64,
    samples: Mutex<VecDeque<GatewaySample>>,
}

impl GatewayMetrics {
    pub(crate) fn new() -> Self {
        Self {
            started_at_ms: unix_now_ms(),
            total_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            samples: Mutex::new(VecDeque::new()),
        }
    }

    pub(crate) fn record(&self, sample: GatewaySample) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        if sample.failed() {
            self.failed_requests.fetch_add(1, Ordering::Relaxed);
        }
        // A poisoned lock means a panic elsewhere already failed the process;
        // the deque append itself keeps no invariant that poisoning breaks,
        // so recovery never drops telemetry.
        let mut samples = self
            .samples
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if samples.len() >= SAMPLE_CAPACITY {
            samples.pop_front();
        }
        samples.push_back(sample);
    }

    pub(crate) fn snapshot(&self) -> GatewayMetricsSnapshot {
        let samples = self
            .samples
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .cloned()
            .collect();
        GatewayMetricsSnapshot {
            started_at_ms: self.started_at_ms,
            total_requests: self.total_requests.load(Ordering::Relaxed),
            failed_requests: self.failed_requests.load(Ordering::Relaxed),
            samples,
        }
    }
}

/// Collects the facts of one in-flight request so its outcome is recorded
/// exactly once, at the boundary that answers the client. The route is bound
/// only after the capability token matches, so rejected traffic is counted
/// without attributing it to any profile.
pub(crate) struct RequestSpan {
    metrics: Arc<GatewayMetrics>,
    ledger: Option<Arc<ClaudeRequestLedger>>,
    codex: Option<codex::CodexRecording>,
    app: AppKind,
    client_protocol: UpstreamProtocol,
    profile_id: Option<String>,
    route_revision: Option<String>,
    upstream_protocol: Option<UpstreamProtocol>,
    request_model: Option<String>,
    mapped_model: Option<String>,
    usage: TokenUsage,
    first_byte_at: Option<Instant>,
    first_token_at: Option<Instant>,
    response_model: Option<String>,
    billing: asb_core::contracts::ClaudeBilling,
    prices: Option<Result<super::claude_pricing::ClaudePriceBook, String>>,
    failover_attempts: Vec<ClaudeFailoverAttempt>,
    request_bytes: u64,
    completion: Option<Arc<std::sync::atomic::AtomicU16>>,
    secrets: Vec<String>,
    started: Instant,
}

impl RequestSpan {
    pub(crate) fn start(
        metrics: Arc<GatewayMetrics>,
        app: AppKind,
        client_protocol: UpstreamProtocol,
    ) -> Self {
        Self {
            metrics,
            ledger: None,
            codex: None,
            app,
            client_protocol,
            profile_id: None,
            route_revision: None,
            upstream_protocol: None,
            request_model: None,
            mapped_model: None,
            usage: TokenUsage::default(),
            first_byte_at: None,
            first_token_at: None,
            response_model: None,
            billing: Default::default(),
            prices: None,
            failover_attempts: Vec::new(),
            request_bytes: 0,
            completion: None,
            secrets: Vec::new(),
            started: Instant::now(),
        }
    }

    pub(crate) fn start_with_ledger(
        metrics: Arc<GatewayMetrics>,
        ledger: Arc<ClaudeRequestLedger>,
        app: AppKind,
        client_protocol: UpstreamProtocol,
    ) -> Self {
        let mut span = Self::start(metrics, app, client_protocol);
        span.ledger = Some(ledger);
        span
    }

    /// Binds one request to the exact route snapshot that admitted it. The
    /// revision must travel with the profile id so a later route replacement
    /// cannot cause traffic to be attributed to the currently active route.
    pub(crate) fn bind_route(
        &mut self,
        profile_id: &str,
        route_revision: &str,
        upstream_protocol: UpstreamProtocol,
    ) {
        self.profile_id = Some(profile_id.to_string());
        self.route_revision = Some(route_revision.to_string());
        self.upstream_protocol = Some(upstream_protocol);
    }

    pub(crate) fn note_request_bytes(&mut self, bytes: u64) {
        self.request_bytes = bytes;
    }

    pub(crate) fn note_request_model(&mut self, model: Option<String>) {
        if model.is_some() {
            self.request_model = model;
        }
    }

    pub(crate) fn note_mapped_model(&mut self, model: Option<String>) {
        if model.is_some() {
            self.mapped_model = model;
        }
    }

    pub(crate) fn note_response_value(&mut self, protocol: UpstreamProtocol, value: &Value) {
        let (model, usage) = metadata_from_value(protocol, value);
        self.note_response_model(model);
        self.note_usage(usage);
    }

    pub(crate) fn note_response_model(&mut self, model: Option<String>) {
        if model.is_some() {
            self.response_model = model;
        }
    }

    pub(crate) fn note_first_token(&mut self) {
        self.first_token_at.get_or_insert_with(Instant::now);
    }

    pub(crate) fn protect_secrets(&mut self, secrets: &[&str]) {
        for secret in secrets.iter().filter(|secret| !secret.is_empty()) {
            if !self.secrets.iter().any(|existing| existing == secret) {
                self.secrets.push((*secret).into());
            }
        }
    }

    pub(crate) fn bind_claude_billing(
        &mut self,
        root: &std::path::Path,
        billing: Option<&asb_core::contracts::ClaudeBilling>,
    ) {
        self.billing = billing.cloned().unwrap_or_default();
        self.prices
            .get_or_insert_with(|| super::claude_pricing::ClaudePriceBook::load(root));
    }

    pub(crate) fn note_usage(&mut self, usage: TokenUsage) {
        self.usage.merge_from(&usage);
    }

    pub(crate) fn note_first_byte(&mut self) {
        if self.first_byte_at.is_none() {
            self.first_byte_at = Some(Instant::now());
        }
    }

    pub(crate) fn note_failover_attempt(&mut self, attempt: ClaudeFailoverAttempt) {
        if self.failover_attempts.len() < 32 {
            self.failover_attempts.push(attempt);
        }
    }

    /// `status` is `None` only when no HTTP status applies to the failure, as
    /// with a WebSocket exchange that died before an answer was delivered.
    pub(crate) fn observe_completion(&mut self) -> Arc<std::sync::atomic::AtomicU16> {
        let completion = Arc::new(std::sync::atomic::AtomicU16::new(0));
        self.completion = Some(completion.clone());
        completion
    }

    pub(crate) fn finish(mut self, status: Option<u16>, response_bytes: u64) {
        if let Some(completion) = &self.completion {
            completion.store(status.unwrap_or(0), std::sync::atomic::Ordering::Release);
        }
        if let Some(attempt) = self.failover_attempts.last_mut() {
            if attempt.outcome == ClaudeFailoverOutcome::Success
                && !status.is_some_and(|s| (200..300).contains(&s))
            {
                attempt.status = status;
                attempt.outcome = ClaudeFailoverOutcome::Failure;
            }
        }
        let duration_ms = self.started.elapsed().as_millis() as u64;
        let first_byte_latency_ms = self
            .first_byte_at
            .map(|at| at.duration_since(self.started).as_millis() as u64);
        self.metrics.record(GatewaySample {
            at_ms: unix_now_ms(),
            app: self.app,
            profile_id: self.profile_id.clone(),
            route_revision: self.route_revision.clone(),
            client_protocol: self.client_protocol,
            upstream_protocol: self.upstream_protocol,
            status,
            duration_ms,
            request_bytes: self.request_bytes,
            response_bytes,
        });
        match self.app {
            AppKind::Codex => self.finish_codex(status, duration_ms, first_byte_latency_ms),
            AppKind::Claude => self.finish_claude(status, duration_ms, first_byte_latency_ms),
        }
    }
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

