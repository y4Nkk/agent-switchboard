use super::super::{anthropic_stop_reason, append_event, Frame};
use super::{merge_identity, parse_chat_frame, AnthropicCall, CallDelta, ChatSource, TextOutput};
use crate::gateway::transform::{usage, ReasoningTransport, TransformError};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(in crate::gateway::transform::stream) struct ChatToAnthropic {
    source: ChatSource,
    started: bool,
    text: Option<TextOutput>,
    calls: BTreeMap<u64, AnthropicCall>,
    reasoning_transport: Option<ReasoningTransport>,
    next_content_index: u64,
    completed: bool,
}

impl ChatToAnthropic {
    pub(in crate::gateway::transform::stream) fn new(
        reasoning_transport: Option<ReasoningTransport>,
    ) -> Self {
        Self {
            source: ChatSource::default(),
            started: false,
            text: None,
            calls: BTreeMap::new(),
            reasoning_transport,
            next_content_index: 0,
            completed: false,
        }
    }

    pub(in crate::gateway::transform::stream) fn on_frame(
        &mut self,
        frame: Frame,
    ) -> Result<Vec<u8>, TransformError> {
        let update = parse_chat_frame(&frame)?;
        self.source.apply(&update)?;
        let mut output = Vec::new();
        self.start(&mut output)?;
        for text in update.text {
            if !text.is_empty() {
                self.flush_reasoning(&mut output)?;
                self.push_text(&mut output, &text)?;
            }
        }
        for call in update.calls {
            self.flush_reasoning(&mut output)?;
            self.push_call(&mut output, call)?;
        }
        if update.done {
            self.complete(&mut output)?;
        }
        Ok(output)
    }

    pub(in crate::gateway::transform::stream) fn finish(
        &mut self,
    ) -> Result<Vec<u8>, TransformError> {
        if self.completed {
            Ok(Vec::new())
        } else {
            Err(TransformError("Chat SSE 缺少 [DONE] 终止帧".to_string()))
        }
    }

    fn start(&mut self, output: &mut Vec<u8>) -> Result<(), TransformError> {
        if self.started {
            return Ok(());
        }
        let Ok((id, model)) = self.source.identity() else {
            return Ok(());
        };
        append_event(
            output,
            "message_start",
            json!({
                "type": "message_start",
                "message": {
                    "id": id,
                    "type": "message",
                    "role": "assistant",
                    "model": model,
                    "content": [],
                    "stop_reason": null,
                    "stop_sequence": null,
                    "usage": usage::anthropic_message_start_json(&self.source.usage),
                },
            }),
        );
        self.started = true;
        Ok(())
    }

    fn push_text(&mut self, output: &mut Vec<u8>, delta: &str) -> Result<(), TransformError> {
        self.start(output)?;
        if !self.started {
            return Err(TransformError(
                "Chat SSE 在 id 和 model 前发送文本".to_string(),
            ));
        }
        if self.text.is_none() {
            let index = self.next_content_index;
            self.next_content_index += 1;
            append_event(
                output,
                "content_block_start",
                json!({
                    "type": "content_block_start",
                    "index": index,
                    "content_block": { "type": "text", "text": "" },
                }),
            );
            self.text = Some(TextOutput {
                index,
                text: String::new(),
            });
        }
        let text = self.text.as_mut().expect("created above");
        text.text.push_str(delta);
        append_event(
            output,
            "content_block_delta",
            json!({
                "type": "content_block_delta",
                "index": text.index,
                "delta": { "type": "text_delta", "text": delta },
            }),
        );
        Ok(())
    }

    fn flush_reasoning(&mut self, output: &mut Vec<u8>) -> Result<(), TransformError> {
        let Some(content) = self.source.pending_reasoning.take() else {
            return Ok(());
        };
        self.start(output)?;
        if !self.started {
            return Err(TransformError(
                "Chat SSE 在 id 和 model 前发送推理内容".to_string(),
            ));
        }
        let transport = self
            .reasoning_transport
            .as_ref()
            .ok_or_else(|| TransformError("当前转换缺少本机推理续接通道".to_string()))?;
        let reasoning = transport.from_chat_content(content)?;
        let index = self.next_content_index;
        self.next_content_index += 1;
        append_event(
            output,
            "content_block_start",
            json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {
                    "type": "redacted_thinking",
                    "data": reasoning.continuation,
                },
            }),
        );
        append_event(
            output,
            "content_block_stop",
            json!({ "type": "content_block_stop", "index": index }),
        );
        Ok(())
    }

    fn push_call(&mut self, output: &mut Vec<u8>, delta: CallDelta) -> Result<(), TransformError> {
        {
            let call = self.calls.entry(delta.index).or_default();
            merge_identity(&mut call.id, delta.id.as_deref(), "Chat tool_call.id")?;
            merge_identity(
                &mut call.name,
                delta.name.as_deref(),
                "Chat tool_call.function.name",
            )?;
            if let Some(arguments) = delta.arguments {
                call.arguments.push_str(&arguments);
            }
        }
        self.start_call(output, delta.index)?;
        let (content_index, arguments) = {
            let call = self.calls.get_mut(&delta.index).expect("entry exists");
            let Some(content_index) = call.content_index else {
                return Ok(());
            };
            if call.sent_arguments == call.arguments.len() {
                return Ok(());
            }
            let arguments = call.arguments[call.sent_arguments..].to_string();
            call.sent_arguments = call.arguments.len();
            (content_index, arguments)
        };
        append_event(
            output,
            "content_block_delta",
            json!({
                "type": "content_block_delta",
                "index": content_index,
                "delta": { "type": "input_json_delta", "partial_json": arguments },
            }),
        );
        Ok(())
    }

    fn start_call(
        &mut self,
        output: &mut Vec<u8>,
        source_index: u64,
    ) -> Result<(), TransformError> {
        self.start(output)?;
        if !self.started {
            return Err(TransformError(
                "Chat SSE 在 id 和 model 前发送工具调用".to_string(),
            ));
        }
        let details = self.calls.get(&source_index).and_then(|call| {
            (call.content_index.is_none())
                .then(|| call.id.clone().zip(call.name.clone()))
                .flatten()
        });
        let Some((id, name)) = details else {
            return Ok(());
        };
        let content_index = self.next_content_index;
        self.next_content_index += 1;
        self.calls
            .get_mut(&source_index)
            .expect("entry exists")
            .content_index = Some(content_index);
        append_event(
            output,
            "content_block_start",
            json!({
                "type": "content_block_start",
                "index": content_index,
                "content_block": { "type": "tool_use", "id": id, "name": name, "input": {} },
            }),
        );
        Ok(())
    }

    fn complete(&mut self, output: &mut Vec<u8>) -> Result<(), TransformError> {
        if self.completed {
            return Err(TransformError("Chat SSE 重复 [DONE] 终止帧".to_string()));
        }
        self.start(output)?;
        if !self.started {
            return Err(TransformError(
                "Chat SSE 在缺少 id 或 model 时结束".to_string(),
            ));
        }
        self.flush_reasoning(output)?;
        if let Some(text) = &self.text {
            append_event(
                output,
                "content_block_stop",
                json!({ "type": "content_block_stop", "index": text.index }),
            );
        }
        for source_index in self.calls.keys().copied().collect::<Vec<_>>() {
            self.complete_call(output, source_index)?;
        }
        append_event(
            output,
            "message_delta",
            json!({
                "type": "message_delta",
                "delta": { "stop_reason": anthropic_stop_reason(self.source.stop()), "stop_sequence": null },
                "usage": usage::anthropic_message_delta_json(&self.source.usage),
            }),
        );
        append_event(output, "message_stop", json!({ "type": "message_stop" }));
        self.completed = true;
        Ok(())
    }

    fn complete_call(
        &mut self,
        output: &mut Vec<u8>,
        source_index: u64,
    ) -> Result<(), TransformError> {
        self.start_call(output, source_index)?;
        let call = self.calls.get_mut(&source_index).expect("entry exists");
        let content_index = call
            .content_index
            .ok_or_else(|| TransformError("Chat SSE 工具调用缺少 id 或名称".to_string()))?;
        if call.sent_arguments < call.arguments.len() {
            let delta = &call.arguments[call.sent_arguments..];
            call.sent_arguments = call.arguments.len();
            append_event(
                output,
                "content_block_delta",
                json!({
                    "type": "content_block_delta",
                    "index": content_index,
                    "delta": { "type": "input_json_delta", "partial_json": delta },
                }),
            );
        }
        serde_json::from_str::<Value>(&call.arguments)
            .map_err(|_| TransformError("Chat SSE 工具参数不是完整 JSON".to_string()))?;
        append_event(
            output,
            "content_block_stop",
            json!({ "type": "content_block_stop", "index": content_index }),
        );
        Ok(())
    }
}
