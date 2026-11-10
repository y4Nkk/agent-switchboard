use asb_core::contracts::AppKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MeasurementScope { Generation, Transport }

impl MeasurementScope {
    pub(super) fn key(self) -> &'static str {
        match self { Self::Generation => "generation", Self::Transport => "transport" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RequestOutcome { Succeeded, Failed, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum NativeTimeSource { Event, Observed, SpanEnd }

/// An observed native operation, not an inferred ASB provider request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RequestUsageRow {
    pub id: String,
    pub app: AppKind,
    pub scope: MeasurementScope,
    pub at_ms: u64,
    pub time_source: NativeTimeSource,
    pub event_name: String,
    pub event_kind: Option<String>,
    pub model: Option<String>,
    pub session_id: Option<String>,
    pub request_id: Option<String>,
    pub outcome: RequestOutcome,
    pub status: Option<u16>,
    pub error_type: Option<String>,
    pub duration_ms: Option<f64>,
    pub first_token_ms: Option<f64>,
    pub first_content_ms: Option<f64>,
    pub output_tokens_per_second: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_creation_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub estimated_cost_usd: Option<String>,
    pub attempt_count: Option<u64>,
}

impl RequestUsageRow {
    pub(super) fn validate(&self) -> Result<(), String> {
        let span = match (self.app, self.scope, self.event_name.as_str()) {
            (AppKind::Claude, MeasurementScope::Generation, "claude_code.llm_request") => true,
            (AppKind::Codex, MeasurementScope::Generation, "codex.sse_event")
            | (AppKind::Codex, MeasurementScope::Transport, "codex.api_request" | "codex.websocket_request")
            | (AppKind::Claude, MeasurementScope::Transport, "api_request" | "api_error") => false,
            _ => return Err("原生性能记录的客户端、事件或计量范围无效".into()),
        };
        if span != (self.time_source == NativeTimeSource::SpanEnd)
            || (self.app == AppKind::Codex && self.scope == MeasurementScope::Generation && self.duration_ms.is_some())
            || (self.scope == MeasurementScope::Transport && (self.first_token_ms.is_some() || self.first_content_ms.is_some())) {
            return Err("原生性能记录的时间来源或计时边界无效".into());
        }
        if self.id.is_empty() || self.id.len() > 128 || self.at_ms == 0 || self.at_ms > i64::MAX as u64 {
            return Err("原生性能记录身份或时间无效".into());
        }
        for text in std::iter::once(self.event_name.as_str()).chain(self.model.as_deref())
            .chain(self.event_kind.as_deref()).chain(self.session_id.as_deref())
            .chain(self.request_id.as_deref()).chain(self.error_type.as_deref()) {
            if text.is_empty() || text.len() > 512 || text.chars().any(char::is_control) {
                return Err("原生性能记录字段无效".into());
            }
        }
        if [self.duration_ms, self.first_token_ms, self.first_content_ms, self.output_tokens_per_second]
            .into_iter().flatten().any(|value| !value.is_finite() || value < 0.0)
            || self.attempt_count == Some(0) {
            return Err("原生性能记录计数或耗时无效".into());
        }
        if self.estimated_cost_usd.as_ref().is_some_and(|cost| asb_core::contracts::decimal_micros(cost).is_err()) {
            return Err("原生性能记录费用无效".into());
        }
        let mut normalized = self.clone();
        normalized.output_tokens_per_second = None;
        normalized.normalize();
        if normalized.output_tokens_per_second != self.output_tokens_per_second {
            return Err("原生性能记录的速度与测量范围不一致".into());
        }
        Ok(())
    }

    pub(super) fn normalize(&mut self) {
        if self.scope == MeasurementScope::Generation && self.outcome == RequestOutcome::Succeeded {
            self.output_tokens_per_second = self.output_tokens.zip(self.duration_ms)
                .filter(|(tokens, duration)| *tokens > 0 && *duration > 0.0)
                .map(|(tokens, duration)| tokens as f64 * 1000.0 / duration);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReceiverStatus {
    pub endpoint: &'static str,
    pub listening: bool,
    pub last_export_at_ms: Option<u64>,
    pub accepted_records: u64,
    pub ignored_records: u64,
    pub error: Option<String>,
}

impl Default for ReceiverStatus {
    fn default() -> Self {
        Self { endpoint: super::ENDPOINT, listening: false, last_export_at_ms: None,
            accepted_records: 0, ignored_records: 0, error: None }
    }
}
