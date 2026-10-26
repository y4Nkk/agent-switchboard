//! Incremental SSE conversion for the two client-facing protocols.
//!
//! A routing gateway must not wait for an entire generation before it starts
//! responding. This reader validates one upstream SSE frame at a time and
//! releases each corresponding client frame as soon as it is available.

#[path = "stream/anthropic/mod.rs"]
mod anthropic;
#[path = "stream/chat.rs"]
mod chat;
#[path = "stream/diagnostics.rs"]
mod diagnostics;
#[path = "stream/output.rs"]
mod output;
#[path = "stream/responses/mod.rs"]
mod responses;

use super::sse::render_event;
use super::{CanonicalResponse, ReasoningTransport, StopReason, TransformError};
use crate::gateway::usage_metadata::{metadata_from_value, TokenUsage};
use crate::provider_diagnostics::ProviderDiagnostic;
use asb_core::contracts::UpstreamProtocol;
use serde_json::Value;
use std::io::{self, Read};

const SOURCE_READ_CHUNK: usize = 8 * 1024;

/// A bounded, one-way transcode reader. `tiny_http` pulls from it while it
/// writes a chunked client response, which means an upstream token delta is
/// not held until the upstream closes the connection.
pub(crate) struct SseTranscoder<R> {
    source: R,
    mode: StreamMode,
    source_protocol: UpstreamProtocol,
    target: UpstreamProtocol,
    max_source_bytes: u64,
    source_bytes: u64,
    source_buffer: Vec<u8>,
    pending: Vec<u8>,
    pending_offset: usize,
    source_finished: bool,
    terminal: bool,
    direct_completed: bool,
    diagnostic: Option<ProviderDiagnostic>,
    secrets: Vec<String>,
    model: Option<String>,
    usage: TokenUsage,
    failed: bool,
    has_token: bool,
    native_responses_dialect: super::NativeResponsesDialect,
}

enum StreamMode {
    Direct,
    Converting(StreamTransformer),
}

enum StreamTransformer {
    ChatToResponses(chat::ChatToResponses),
    ChatToAnthropic(chat::ChatToAnthropic),
    AnthropicToResponses(anthropic::AnthropicToResponses),
    ResponsesToAnthropic(responses::ResponsesToAnthropic),
    GeminiToAnthropic(super::claude_gemini::stream::GeminiToAnthropic),
}

impl StreamTransformer {
    fn new(
        from: UpstreamProtocol,
        to: UpstreamProtocol,
        reasoning_transport: Option<ReasoningTransport>,
    ) -> Result<Self, TransformError> {
        match (from, to) {
            (UpstreamProtocol::GeminiGenerateContent, UpstreamProtocol::AnthropicMessages) => {
                Ok(Self::GeminiToAnthropic(
                    super::claude_gemini::stream::GeminiToAnthropic::new(reasoning_transport),
                ))
            }
            (UpstreamProtocol::ChatCompletions, UpstreamProtocol::Responses) => Ok(
                Self::ChatToResponses(chat::ChatToResponses::new(reasoning_transport)),
            ),
            (UpstreamProtocol::ChatCompletions, UpstreamProtocol::AnthropicMessages) => Ok(
                Self::ChatToAnthropic(chat::ChatToAnthropic::new(reasoning_transport)),
            ),
            (UpstreamProtocol::AnthropicMessages, UpstreamProtocol::Responses) => {
                Ok(Self::AnthropicToResponses(
                    anthropic::AnthropicToResponses::new(reasoning_transport),
                ))
            }
            (UpstreamProtocol::Responses, UpstreamProtocol::AnthropicMessages) => {
                Ok(Self::ResponsesToAnthropic(
                    responses::ResponsesToAnthropic::new(reasoning_transport),
                ))
            }
            _ => Err(TransformError("该 SSE 协议组合不支持转换".to_string())),
        }
    }

    fn on_frame(&mut self, frame: Frame) -> Result<Vec<u8>, TransformError> {
        match self {
            Self::ChatToResponses(transformer) => transformer.on_frame(frame),
            Self::ChatToAnthropic(transformer) => transformer.on_frame(frame),
            Self::AnthropicToResponses(transformer) => transformer.on_frame(frame),
            Self::ResponsesToAnthropic(transformer) => transformer.on_frame(frame),
            Self::GeminiToAnthropic(transformer) => transformer.on_frame(frame),
        }
    }

    fn finish(&mut self) -> Result<Vec<u8>, TransformError> {
        match self {
            Self::ChatToResponses(transformer) => transformer.finish(),
            Self::ChatToAnthropic(transformer) => transformer.finish(),
            Self::AnthropicToResponses(transformer) => transformer.finish(),
            Self::ResponsesToAnthropic(transformer) => transformer.finish(),
            Self::GeminiToAnthropic(transformer) => transformer.finish(),
        }
    }
}

impl<R> SseTranscoder<R>
where
    R: Read,
{
    pub(crate) fn new(
        source: R,
        from: UpstreamProtocol,
        target: UpstreamProtocol,
        max_source_bytes: u64,
        reasoning_transport: Option<&ReasoningTransport>,
    ) -> Result<Self, TransformError> {
        let mode = if from == target {
            StreamMode::Direct
        } else {
            StreamMode::Converting(StreamTransformer::new(
                from,
                target,
                reasoning_transport.cloned(),
            )?)
        };
        Ok(Self {
            source,
            mode,
            source_protocol: from,
            target,
            max_source_bytes,
            source_bytes: 0,
            source_buffer: Vec::new(),
            pending: Vec::new(),
            pending_offset: 0,
            source_finished: false,
            terminal: false,
            direct_completed: false,
            diagnostic: None,
            secrets: Vec::new(),
            model: None,
            usage: TokenUsage::default(),
            failed: false,
            has_token: false,
            native_responses_dialect: Default::default(),
        })
    }

    pub(crate) fn with_native_responses_dialect(
        mut self,
        dialect: super::NativeResponsesDialect,
    ) -> Self {
        self.native_responses_dialect = dialect;
        self
    }

    pub(crate) fn model(&self) -> Option<String> {
        self.model.clone()
    }

    pub(crate) fn has_token(&self) -> bool {
        self.has_token
    }

    pub(crate) fn usage(&self) -> TokenUsage {
        self.usage.clone()
    }

    fn pending_is_empty(&self) -> bool {
        self.pending_offset == self.pending.len()
    }

    fn append(&mut self, bytes: Vec<u8>) {
        if self.pending_is_empty() {
            self.pending.clear();
            self.pending_offset = 0;
        }
        self.pending.extend(bytes);
    }

    fn consume_source(&mut self, bytes: &[u8]) {
        self.source_bytes = self.source_bytes.saturating_add(bytes.len() as u64);
        if self.source_bytes > self.max_source_bytes {
            self.fail("上游响应超过本机协议网关限制");
            return;
        }
        self.source_buffer.extend_from_slice(bytes);
        while let Some((index, delimiter_length)) = frame_boundary(&self.source_buffer) {
            let frame_bytes = self.source_buffer[..index].to_vec();
            self.source_buffer.drain(..index + delimiter_length);
            match parse_frame(&frame_bytes).and_then(|frame| {
                if let Some(frame) = &frame {
                    self.observe_frame(frame);
                }
                self.convert_frame(frame, &frame_bytes)
            }) {
                Ok(bytes) => self.append(bytes),
                Err(error) => {
                    self.fail(&format!("无法转换上游 SSE：{error}"));
                    break;
                }
            }
            if self.terminal {
                break;
            }
        }
    }

    fn observe_frame(&mut self, frame: &Frame) {
        let Ok(value) = serde_json::from_str::<Value>(&frame.data) else {
            return;
        };
        self.has_token |= super::usage::frame_has_token(self.source_protocol, &value);
        let (model, usage) = metadata_from_value(self.source_protocol, &value);
        if model.is_some() {
            self.model = model;
        }
        self.usage.merge_from(&usage);
    }

    fn convert_frame(
        &mut self,
        frame: Option<Frame>,
        bytes: &[u8],
    ) -> Result<Vec<u8>, TransformError> {
        let Some(frame) = frame else {
            return Ok(if matches!(&self.mode, StreamMode::Direct) {
                self.redact_direct_frame(bytes)?
            } else {
                vec![]
            });
        };
        if matches!(&self.mode, StreamMode::Direct) {
            return self.convert_direct_frame(frame, bytes);
        }
        if !self.is_token_limit_incomplete(&frame) && self.upstream_failed(&frame) {
            return Ok(Vec::new());
        }
        match &mut self.mode {
            StreamMode::Converting(transformer) => transformer.on_frame(frame),
            StreamMode::Direct => unreachable!("direct mode handled above"),
        }
    }

    fn convert_direct_frame(
        &mut self,
        frame: Frame,
        bytes: &[u8],
    ) -> Result<Vec<u8>, TransformError> {
        if self.direct_completed {
            return Err(TransformError(
                "上游 SSE 在终止事件后继续发送数据".to_string(),
            ));
        }
        if frame.data.trim() == "[DONE]" {
            if self.target != UpstreamProtocol::ChatCompletions || frame.event.is_some() {
                return Err(TransformError(
                    "当前原生 SSE 协议不支持 [DONE] 终止帧".to_string(),
                ));
            }
            self.direct_completed = true;
            return self.redact_direct_frame(bytes);
        }
        let mut value = json_data(&frame, "上游 SSE data")?;
        let kind = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| TransformError("上游 SSE 事件缺少 type".to_string()))?;
        if frame.event.as_deref().is_some_and(|event| event != kind) {
            return Err(TransformError("上游 SSE event 与 type 不一致".to_string()));
        }
        self.direct_completed = matches!(
            kind,
            "response.completed"
                | "response.failed"
                | "response.incomplete"
                | "error"
                | "message_stop"
        );
        if self.source_protocol == UpstreamProtocol::Responses
            && self.native_responses_dialect.restore(&mut value)
        {
            return self.restore_direct_frame(bytes, &value);
        }
        self.redact_direct_frame(bytes)
    }

    fn restore_direct_frame(&self, bytes: &[u8], value: &Value) -> Result<Vec<u8>, TransformError> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| TransformError("上游 SSE 不是 UTF-8 文本".to_string()))?;
        let data = format!("data: {value}");
        let mut inserted = false;
        let lines = text.lines().filter_map(|line| {
            if !line.starts_with("data:") {
                return Some(line);
            }
            if std::mem::replace(&mut inserted, true) {
                None
            } else {
                Some(data.as_str())
            }
        }).collect::<Vec<_>>();
        self.redact_direct_frame(lines.join("\n").as_bytes())
    }

    fn redact_direct_frame(&self, bytes: &[u8]) -> Result<Vec<u8>, TransformError> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| TransformError("上游 SSE 不是 UTF-8 文本".to_string()))?;
        let secrets = self.secrets.iter().map(String::as_str).collect::<Vec<_>>();
        let text = crate::provider_diagnostics::redact_text(text, &secrets);
        Ok([text.as_bytes(), b"\n\n"].concat())
    }

    fn finish_source(&mut self) {
        if self.terminal {
            return;
        }
        if self
            .source_buffer
            .iter()
            .any(|byte| !byte.is_ascii_whitespace())
        {
            self.fail("上游 SSE 在完整帧前结束");
            return;
        }
        let result = match &mut self.mode {
            StreamMode::Direct if self.direct_completed => Ok(Vec::new()),
            StreamMode::Direct => Err(TransformError("上游 SSE 流未给出终止事件".to_string())),
            StreamMode::Converting(transformer) => transformer.finish(),
        };
        match result {
            Ok(bytes) => {
                self.append(bytes);
                self.terminal = true;
            }
            Err(error) => self.fail(&format!("无法转换上游 SSE：{error}")),
        }
    }
}

impl<R> Read for SseTranscoder<R>
where
    R: Read,
{
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        while self.pending_is_empty() && !self.terminal {
            if self.source_finished {
                self.finish_source();
                break;
            }
            let mut buffer = [0_u8; SOURCE_READ_CHUNK];
            match self.source.read(&mut buffer) {
                Ok(0) => {
                    self.source_finished = true;
                    self.finish_source();
                }
                Ok(read) => self.consume_source(&buffer[..read]),
                Err(error) => self.fail_io(&error),
            }
        }
        if self.pending_is_empty() {
            return Ok(0);
        }
        let count = output
            .len()
            .min(self.pending.len().saturating_sub(self.pending_offset));
        output[..count]
            .copy_from_slice(&self.pending[self.pending_offset..self.pending_offset + count]);
        self.pending_offset += count;
        Ok(count)
    }
}

pub(super) struct Frame {
    pub(super) event: Option<String>,
    pub(super) data: String,
}

pub(super) fn json_data(frame: &Frame, context: &str) -> Result<Value, TransformError> {
    serde_json::from_str(&frame.data)
        .map_err(|_| TransformError(format!("{context} 不是有效 JSON")))
}

pub(super) fn append_event(output: &mut Vec<u8>, event: &str, value: Value) {
    output.extend(render_event(event, &value));
}

pub(super) fn parse_responses_complete(
    value: &Value,
    reasoning_transport: Option<&ReasoningTransport>,
) -> Result<CanonicalResponse, TransformError> {
    super::response::parse_response(UpstreamProtocol::Responses, value, reasoning_transport)
}

pub(super) fn anthropic_stop_reason(stop: StopReason) -> &'static str {
    match stop {
        StopReason::ToolUse => "tool_use",
        StopReason::MaxTokens => "max_tokens",
        StopReason::StopSequence => "stop_sequence",
        StopReason::EndTurn => "end_turn",
    }
}

fn frame_boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    let mut index = 0;
    while index < buffer.len() {
        if buffer[index..].starts_with(b"\r\n\r\n") {
            return Some((index, 4));
        }
        if buffer[index..].starts_with(b"\n\n") {
            return Some((index, 2));
        }
        index += 1;
    }
    None
}

fn parse_frame(bytes: &[u8]) -> Result<Option<Frame>, TransformError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| TransformError("上游 SSE 不是 UTF-8 文本".to_string()))?;
    let mut event = None;
    let mut data = Vec::new();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("event:") {
            event = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("data:") {
            data.push(value.trim_start().to_string());
        } else if line.starts_with(':')
            || line.starts_with("id:")
            || line.starts_with("retry:")
            || line.trim().is_empty()
        {
            continue;
        } else {
            return Err(TransformError("上游 SSE 包含无效帧行".to_string()));
        }
    }
    Ok((!data.is_empty()).then(|| Frame {
        event,
        data: data.join("\n"),
    }))
}
