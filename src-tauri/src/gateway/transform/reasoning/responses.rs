use super::*;

impl ReasoningTransport {
    /// Preserve the provider-owned item separately from the gateway envelope.
    /// An empty summary still carries a native identity and must not fail.
    pub(crate) fn from_responses_item(&self, item: Value) -> Result<Reasoning, TransformError> {
        validate_item(&item)?;
        self.seal(Record {
            protocol: UpstreamProtocol::Responses,
            text: visible_text(&item)?,
            signature: None,
            redacted: None,
            responses_item: Some(item),
            gemini_turn: None,
        })
    }
}

impl Reasoning {
    /// This is an upstream input, never the local encrypted continuation sent
    /// to a client. A foreign protocol's trace is not a Responses item.
    pub(crate) fn responses_input_item(&self) -> Result<Value, TransformError> {
        self.responses_item.clone().ok_or_else(|| {
            TransformError("推理记录不是该 Responses 上游的原始数据项，不能跨协议回放".into())
        })
    }
}

pub(super) fn validate_item(item: &Value) -> Result<(), TransformError> {
    let object = item
        .as_object()
        .ok_or_else(|| TransformError("Responses reasoning 必须是对象".into()))?;
    if object.get("type").and_then(Value::as_str) != Some("reasoning") {
        return Err(TransformError("Responses reasoning 数据项类型无效".into()));
    }
    if let Some(id) = object.get("id") {
        if id.as_str().is_none_or(str::is_empty) {
            return Err(TransformError(
                "Responses reasoning.id 必须是非空字符串".into(),
            ));
        }
    }
    if let Some(status) = object.get("status").filter(|value| !value.is_null()) {
        if status.as_str() != Some("completed") {
            return Err(TransformError(
                "Responses reasoning.status 必须是 completed".into(),
            ));
        }
    }
    match object.get("encrypted_content") {
        None | Some(Value::Null) => {}
        Some(Value::String(value)) if !ReasoningTransport::is_continuation(value) => {}
        Some(Value::String(_)) => {
            return Err(TransformError(
                "上游 reasoning 不能冒充本机推理续接载荷".into(),
            ))
        }
        Some(_) => {
            return Err(TransformError(
                "Responses reasoning.encrypted_content 必须是字符串或 null".into(),
            ))
        }
    }
    visible_text(item)?;
    Ok(())
}

fn visible_text(item: &Value) -> Result<String, TransformError> {
    let mut text = String::new();
    for field in ["summary", "content"] {
        let Some(value) = item.get(field).filter(|value| !value.is_null()) else {
            continue;
        };
        let values = value
            .as_array()
            .ok_or_else(|| TransformError(format!("Responses reasoning.{field} 必须是数组")))?;
        for value in values {
            let part = value.as_object().ok_or_else(|| {
                TransformError(format!("Responses reasoning.{field} 包含非对象数据"))
            })?;
            if matches!(
                part.get("type").and_then(Value::as_str),
                Some("summary_text" | "reasoning_text")
            ) {
                let content = part.get("text").and_then(Value::as_str).ok_or_else(|| {
                    TransformError(format!("Responses reasoning.{field}.text 必须是字符串"))
                })?;
                text.push_str(content);
            }
        }
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn native_reasoning_preserves_empty_summary_and_opaque_fields() {
        let transport = ReasoningTransport::from_continuation_key([9; 32]);
        for item in [
            json!({"type":"reasoning","id":"rs_empty","summary":[]}),
            json!({"type":"reasoning","id":"rs_opaque","summary":[],"encrypted_content":"native-opaque","vendor_extension":{"keep":true}}),
            json!({"type":"reasoning","id":"rs_summary","summary":[{"type":"summary_text","text":"native summary"}]}),
        ] {
            let encoded = transport.from_responses_item(item.clone()).unwrap();
            let replayed = transport.from_continuation(encoded.continuation).unwrap();
            assert_eq!(replayed.responses_input_item().unwrap(), item);
        }
    }

    #[test]
    fn native_reasoning_cannot_replay_under_a_different_backend_key() {
        let first = ReasoningTransport::from_continuation_key([9; 32]);
        let second = ReasoningTransport::from_continuation_key([10; 32]);
        let item = json!({"type":"reasoning","id":"rs_opaque","summary":[],"encrypted_content":"native-opaque"});
        let encoded = first.from_responses_item(item).unwrap();
        assert!(second.from_continuation(encoded.continuation).is_err());
    }

    #[test]
    fn a_chat_trace_never_becomes_an_upstream_responses_ciphertext() {
        let transport = ReasoningTransport::from_continuation_key([9; 32]);
        assert!(transport
            .from_chat_content("trace".into())
            .unwrap()
            .responses_input_item()
            .is_err());
        assert!(transport
            .from_responses_item(
                json!({"type":"reasoning","summary":[],"encrypted_content":"asb-reasoning-v4.fake"})
            )
            .is_err());
    }
}
