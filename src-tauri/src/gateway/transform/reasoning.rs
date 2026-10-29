use super::TransformError;
use aes_gcm::aead::{
    rand_core::{OsRng, RngCore},
    Aead, KeyInit, Payload as AeadPayload,
};
use aes_gcm::{Aes256Gcm, Nonce};
use asb_core::contracts::UpstreamProtocol;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
mod responses;

const CONTINUATION_PREFIX: &str = "asb-reasoning-v4.";
/// Any version of this gateway's reasoning payload. Another version is a stale
/// payload that must be rejected, never mistaken for an upstream block.
const CONTINUATION_FAMILY: &str = "asb-reasoning-";
const AAD: &[u8] = b"agent-switchboard/reasoning-continuation/v4";
const NONCE_BYTES: usize = 12;

/// What one upstream reasoning trace contains once this gateway has read it.
///
/// A trace is replayed verbatim to the backend that produced it: Anthropic
/// signs its thinking blocks, and an opaque block is only readable by its own
/// backend. Sealing the whole record keeps that replay possible across turns
/// without ever handing a client the readable text.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    protocol: UpstreamProtocol,
    /// Empty Chat traces remain distinct from a missing reasoning field;
    /// opaque records carry their native data alongside an empty trace.
    text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    redacted: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    responses_item: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    gemini_turn: Option<super::claude_gemini::replay::GeminiTurn>,
}

impl Record {
    fn validate(&self) -> Result<(), TransformError> {
        if self.text.is_empty()
            && self.protocol != UpstreamProtocol::ChatCompletions
            && self.redacted.is_none()
            && self.responses_item.is_none()
            && self.gemini_turn.is_none()
        {
            return Err(TransformError("推理续接载荷不含内容".to_string()));
        }
        if let Some(turn) = &self.gemini_turn {
            turn.validate()?;
        }
        if let Some(item) = &self.responses_item {
            responses::validate_item(item)?;
        }
        Ok(())
    }
}

/// The gateway-local representation of one upstream reasoning trace. The
/// cleartext is only used while rendering an upstream request; the opaque
/// continuation is the only form sent to Codex or Claude Code.
#[derive(Clone)]
pub(crate) struct Reasoning {
    pub(crate) protocol: UpstreamProtocol,
    pub(crate) content: String,
    pub(crate) continuation: String,
    /// The upstream signature that authenticates `content` for its own backend.
    pub(crate) signature: Option<String>,
    /// An upstream opaque block, replayable only to the backend that issued it.
    pub(crate) redacted: Option<String>,
    pub(crate) responses_item: Option<Value>,
    pub(crate) gemini_turn: Option<super::claude_gemini::replay::GeminiTurn>,
}

/// Uses a backend-bound continuation key independent of access capabilities.
#[derive(Clone)]
pub(crate) struct ReasoningTransport {
    key: [u8; 32],
    continuation_key: [u8; 32],
}

impl ReasoningTransport {
    pub(crate) fn from_continuation_key(continuation_key: [u8; 32]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"agent-switchboard/reasoning-key/v2:");
        hasher.update(continuation_key);
        Self {
            key: hasher.finalize().into(),
            continuation_key,
        }
    }

    pub(crate) fn continuation_key(&self) -> &[u8; 32] {
        &self.continuation_key
    }

    pub(crate) fn from_chat_content(&self, content: String) -> Result<Reasoning, TransformError> {
        self.seal(Record {
            protocol: UpstreamProtocol::ChatCompletions,
            text: content,
            signature: None,
            redacted: None,
            responses_item: None,
            gemini_turn: None,
        })
    }

    /// Seals one Anthropic thinking block together with the signature that
    /// makes it replayable to the backend that produced it.
    pub(crate) fn from_anthropic_thinking(
        &self,
        text: String,
        signature: Option<String>,
    ) -> Result<Reasoning, TransformError> {
        self.seal(Record {
            protocol: UpstreamProtocol::AnthropicMessages,
            text,
            signature: signature.filter(|value| !value.is_empty()),
            redacted: None,
            responses_item: None,
            gemini_turn: None,
        })
    }

    /// Seals an upstream opaque block so the same backend receives it verbatim
    /// on the next turn. No other backend can read it back.
    pub(crate) fn from_redacted(&self, data: String) -> Result<Reasoning, TransformError> {
        self.seal(Record {
            protocol: UpstreamProtocol::AnthropicMessages,
            text: String::new(),
            signature: None,
            redacted: Some(data),
            responses_item: None,
            gemini_turn: None,
        })
    }

    pub(crate) fn from_gemini_turn(
        &self,
        turn: super::claude_gemini::replay::GeminiTurn,
    ) -> Result<Reasoning, TransformError> {
        self.seal(Record {
            protocol: UpstreamProtocol::GeminiGenerateContent,
            text: String::new(),
            signature: None,
            redacted: None,
            responses_item: None,
            gemini_turn: Some(turn),
        })
    }

    /// Opens a payload this gateway issued to a client.
    pub(crate) fn from_continuation(
        &self,
        continuation: String,
    ) -> Result<Reasoning, TransformError> {
        let record: Record = serde_json::from_slice(&self.open(&continuation)?)
            .map_err(|_| TransformError("推理续接载荷格式无效".to_string()))?;
        record.validate()?;
        Ok(Reasoning {
            protocol: record.protocol,
            content: record.text,
            continuation,
            signature: record.signature,
            redacted: record.redacted,
            responses_item: record.responses_item,
            gemini_turn: record.gemini_turn,
        })
    }

    /// Whether a client- or upstream-supplied payload claims to be a gateway
    /// continuation. A stale version is rejected by `open` rather than read as
    /// an upstream opaque block.
    pub(crate) fn is_continuation(value: &str) -> bool {
        value.starts_with(CONTINUATION_FAMILY)
    }

    fn seal(&self, record: Record) -> Result<Reasoning, TransformError> {
        record.validate()?;
        let cleartext = serde_json::to_vec(&record)
            .map_err(|_| TransformError("无法编码推理续接内容".to_string()))?;
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|_| TransformError("无法初始化推理续接加密器".to_string()))?;
        let mut nonce = [0_u8; NONCE_BYTES];
        OsRng.fill_bytes(&mut nonce);
        let encrypted = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                AeadPayload {
                    msg: &cleartext,
                    aad: AAD,
                },
            )
            .map_err(|_| TransformError("无法加密推理续接内容".to_string()))?;
        let mut payload = Vec::with_capacity(nonce.len() + encrypted.len());
        payload.extend_from_slice(&nonce);
        payload.extend_from_slice(&encrypted);
        Ok(Reasoning {
            protocol: record.protocol,
            content: record.text,
            continuation: format!("{CONTINUATION_PREFIX}{}", URL_SAFE_NO_PAD.encode(payload)),
            signature: record.signature,
            redacted: record.redacted,
            responses_item: record.responses_item,
            gemini_turn: record.gemini_turn,
        })
    }

    fn open(&self, continuation: &str) -> Result<Vec<u8>, TransformError> {
        let encoded = continuation
            .strip_prefix(CONTINUATION_PREFIX)
            .ok_or_else(|| TransformError("推理续接版本无效，当前仅接受 v4；请重新开始会话".to_string()))?;
        let payload = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| {
            TransformError("推理续接载荷格式无效；请从同一供应商路由继续会话".to_string())
        })?;
        let (nonce, encrypted) = payload.split_at_checked(NONCE_BYTES).ok_or_else(|| {
            TransformError("推理续接载荷格式无效；请从同一供应商路由继续会话".to_string())
        })?;
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|_| TransformError("无法初始化推理续接解密器".to_string()))?;
        cipher
            .decrypt(
                Nonce::from_slice(nonce),
                AeadPayload {
                    msg: encrypted,
                    aad: AAD,
                },
            )
            .map_err(|_| TransformError("推理续接载荷无效；请从同一供应商路由继续会话".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuation_round_trips_without_exposing_reasoning() {
        let transport = ReasoningTransport::from_continuation_key([1; 32]);
        let reasoning = transport
            .from_chat_content("private chain of thought".to_string())
            .expect("encrypt reasoning");

        assert!(!reasoning.continuation.contains(&reasoning.content));
        assert_eq!(
            transport
                .from_continuation(reasoning.continuation)
                .expect("decrypt reasoning")
                .content,
            "private chain of thought"
        );
    }

    #[test]
    fn an_anthropic_signature_survives_the_client_round_trip() {
        let transport = ReasoningTransport::from_continuation_key([4; 32]);
        let reasoning = transport
            .from_anthropic_thinking("private plan".to_string(), Some("sig-1".to_string()))
            .expect("seal signed thinking");
        let replayed = transport
            .from_continuation(reasoning.continuation)
            .expect("open the client replay");
        assert_eq!(replayed.content, "private plan");
        assert_eq!(replayed.signature.as_deref(), Some("sig-1"));
        assert!(replayed.redacted.is_none());
    }

    #[test]
    fn an_opaque_upstream_block_round_trips_without_readable_text() {
        let transport = ReasoningTransport::from_continuation_key([5; 32]);
        let reasoning = transport
            .from_redacted("opaque-upstream-data".to_string())
            .expect("seal opaque block");
        assert!(reasoning.content.is_empty());
        let replayed = transport
            .from_continuation(reasoning.continuation)
            .expect("open the client replay");
        assert_eq!(replayed.redacted.as_deref(), Some("opaque-upstream-data"));
    }

    #[test]
    fn an_empty_record_is_refused() {
        assert!(ReasoningTransport::from_continuation_key([6; 32])
            .from_anthropic_thinking(String::new(), None)
            .is_err());
    }

    #[test]
    fn original_empty_chat_reasoning_keeps_its_protocol_and_presence() {
        let transport = ReasoningTransport::from_continuation_key([6; 32]);
        let sealed = transport.from_chat_content(String::new()).unwrap();
        assert!(sealed.continuation.starts_with("asb-reasoning-v4."));
        let restored = transport.from_continuation(sealed.continuation).unwrap();
        assert_eq!(restored.protocol, UpstreamProtocol::ChatCompletions);
        assert!(restored.content.is_empty());
        assert!(restored.redacted.is_none());
    }

    #[test]
    fn only_this_gateway_format_is_recognized_as_a_continuation() {
        assert!(ReasoningTransport::is_continuation("asb-reasoning-v4.abc"));
        assert!(ReasoningTransport::is_continuation("asb-reasoning-v3.abc"));
        assert!(ReasoningTransport::is_continuation("asb-reasoning-v2.abc"));
        assert!(!ReasoningTransport::is_continuation("upstream-opaque-blob"));
    }

    #[test]
    fn a_stale_gateway_payload_is_rejected_rather_than_reinterpreted() {
        let transport = ReasoningTransport::from_continuation_key([7; 32]);
        for version in ["v2", "v3"] {
            let error = transport
                .from_continuation(format!("asb-reasoning-{version}.stale-payload"))
                .err()
                .expect("a stale payload must not be reopened");
            assert!(error.0.contains("当前仅接受 v4"), "{}", error.0);
        }
    }

    #[test]
    fn continuation_is_bound_to_its_loopback_route() {
        let first = ReasoningTransport::from_continuation_key([2; 32]);
        let second = ReasoningTransport::from_continuation_key([3; 32]);
        let reasoning = first
            .from_chat_content("private chain of thought".to_string())
            .expect("encrypt reasoning");

        assert!(second.from_continuation(reasoning.continuation).is_err());
    }
}
