//! One owner for provider usage parsing and the three protocol usage shapes.
//!
//! The three protocols report the same three counters under different names,
//! and real providers add their own cache counters on top of the documented
//! set. A usage object carries metering only, never request semantics, so an
//! unrecognised vendor counter is preserved where it has a canonical slot and
//! otherwise ignored. Rejecting a finished generation over a metering extra
//! would fail a request the user already paid for.

use super::{TransformError, Usage};
use asb_core::contracts::UpstreamProtocol;
use serde_json::{json, Map, Value};

impl Usage {
    /// Stream usage is cumulative but may be reported in partial snapshots.
    pub(crate) fn merge_from(&mut self, update: &Self) {
        let totals_changed = update.input_tokens.is_some() || update.output_tokens.is_some();
        self.input_tokens = update.input_tokens.or(self.input_tokens);
        self.output_tokens = update.output_tokens.or(self.output_tokens);
        self.total_tokens = update.total_tokens.or_else(|| {
            if totals_changed {
                None
            } else {
                self.total_tokens
            }
        });
        self.cached_tokens = update.cached_tokens.or(self.cached_tokens);
        self.cache_creation_tokens = update.cache_creation_tokens.or(self.cache_creation_tokens);
        self.reasoning_tokens = update.reasoning_tokens.or(self.reasoning_tokens);
    }
}

pub(crate) fn parse(
    protocol: UpstreamProtocol,
    value: Option<&Value>,
) -> Result<Usage, TransformError> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(Usage::default());
    };
    let map = value
        .as_object()
        .ok_or_else(|| TransformError(format!("{} 必须是对象", context(protocol))))?;
    Ok(match protocol {
        UpstreamProtocol::Responses => parse_responses(map),
        UpstreamProtocol::ChatCompletions => parse_chat(map),
        UpstreamProtocol::AnthropicMessages => parse_anthropic(map),
        UpstreamProtocol::GeminiGenerateContent => parse_gemini(map),
    })
}

/// The Responses usage shape Codex reads back. Detail objects are emitted only
/// when the upstream actually reported them; an unknown counter is not turned
/// into a fabricated zero.
pub(crate) fn responses_json(usage: &Usage) -> Value {
    let mut value = totals(usage, "input_tokens", "output_tokens", "total_tokens");
    let object = value.as_object_mut().expect("usage object");
    if let Some(cached) = usage.cached_tokens {
        object.insert(
            "input_tokens_details".to_string(),
            json!({ "cached_tokens": cached }),
        );
    }
    if let Some(reasoning) = usage.reasoning_tokens {
        object.insert(
            "output_tokens_details".to_string(),
            json!({ "reasoning_tokens": reasoning }),
        );
    }
    value
}

pub(crate) fn chat_json(usage: &Usage) -> Value {
    let mut value = totals(usage, "prompt_tokens", "completion_tokens", "total_tokens");
    let object = value.as_object_mut().expect("usage object");
    if let Some(cached) = usage.cached_tokens {
        object.insert(
            "prompt_tokens_details".to_string(),
            json!({ "cached_tokens": cached }),
        );
    }
    if let Some(reasoning) = usage.reasoning_tokens {
        object.insert(
            "completion_tokens_details".to_string(),
            json!({ "reasoning_tokens": reasoning }),
        );
    }
    value
}

pub(crate) fn anthropic_json(usage: &Usage) -> Value {
    let mut value = anthropic_message_start_json(usage);
    value.as_object_mut().expect("usage object").insert(
        "output_tokens".to_string(),
        json!(usage.output_tokens.unwrap_or(0)),
    );
    value
}

/// Anthropic reports the input side on `message_start` and the output side on
/// `message_delta`, so a streaming bridge must be able to emit each half.
pub(crate) fn anthropic_message_start_json(usage: &Usage) -> Value {
    let mut value =
        json!({ "input_tokens": anthropic_fresh_input(usage).unwrap_or(0), "output_tokens": 0 });
    if let Some(cached) = usage.cached_tokens {
        value
            .as_object_mut()
            .expect("usage object")
            .insert("cache_read_input_tokens".to_string(), json!(cached));
    }
    if let Some(created) = usage.cache_creation_tokens {
        value["cache_creation_input_tokens"] = json!(created);
    }
    value
}

pub(crate) fn anthropic_message_delta_json(usage: &Usage) -> Value {
    let mut value = json!({ "output_tokens": usage.output_tokens.unwrap_or(0) });
    if let Some(input) = anthropic_fresh_input(usage) {
        value["input_tokens"] = json!(input);
    }
    if let Some(cached) = usage.cached_tokens {
        value["cache_read_input_tokens"] = json!(cached);
    }
    if let Some(created) = usage.cache_creation_tokens {
        value["cache_creation_input_tokens"] = json!(created);
    }
    value
}

fn anthropic_fresh_input(usage: &Usage) -> Option<u64> {
    usage.input_tokens.map(|input| {
        input
            .saturating_sub(usage.cached_tokens.unwrap_or(0))
            .saturating_sub(usage.cache_creation_tokens.unwrap_or(0))
    })
}

fn totals(usage: &Usage, input: &str, output: &str, total: &str) -> Value {
    let input_tokens = usage.input_tokens.unwrap_or(0);
    let output_tokens = usage.output_tokens.unwrap_or(0);
    json!({
        input: input_tokens,
        output: output_tokens,
        total: usage.total_tokens.unwrap_or(input_tokens.saturating_add(output_tokens)),
    })
}

fn parse_responses(map: &Map<String, Value>) -> Usage {
    Usage {
        input_tokens: token(map, "input_tokens"),
        output_tokens: token(map, "output_tokens"),
        total_tokens: token(map, "total_tokens"),
        cached_tokens: pointer(map, "/input_tokens_details/cached_tokens"),
        cache_creation_tokens: None,
        reasoning_tokens: pointer(map, "/output_tokens_details/reasoning_tokens"),
    }
}

/// Chat Completions cache reporting is the least standardised part of the
/// ecosystem, so the documented detail object wins and each vendor's own
/// counter is a fallback behind it. DeepSeek reports `prompt_cache_hit_tokens`
/// and some relays only mirror it into the detail object.
fn parse_chat(map: &Map<String, Value>) -> Usage {
    Usage {
        input_tokens: token(map, "prompt_tokens"),
        output_tokens: token(map, "completion_tokens"),
        total_tokens: token(map, "total_tokens"),
        cached_tokens: pointer(map, "/prompt_tokens_details/cached_tokens")
            .or_else(|| pointer(map, "/input_tokens_details/cached_tokens"))
            .or_else(|| token(map, "prompt_cache_hit_tokens"))
            .or_else(|| token(map, "cache_read_input_tokens")),
        reasoning_tokens: pointer(map, "/completion_tokens_details/reasoning_tokens"),
        cache_creation_tokens: None,
    }
}

fn parse_anthropic(map: &Map<String, Value>) -> Usage {
    let cached = token(map, "cache_read_input_tokens");
    let created = token(map, "cache_creation_input_tokens");
    let input = token(map, "input_tokens").map(|input| {
        input
            .saturating_add(cached.unwrap_or(0))
            .saturating_add(created.unwrap_or(0))
    });
    let output = token(map, "output_tokens");
    Usage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: input
            .zip(output)
            .map(|(input, output)| input.saturating_add(output)),
        cached_tokens: cached,
        cache_creation_tokens: created,
        reasoning_tokens: None,
    }
}

fn token(map: &Map<String, Value>, key: &str) -> Option<u64> {
    map.get(key).and_then(Value::as_u64)
}

fn pointer(map: &Map<String, Value>, path: &str) -> Option<u64> {
    Value::Object(map.clone())
        .pointer(path)
        .and_then(Value::as_u64)
}

fn context(protocol: UpstreamProtocol) -> &'static str {
    match protocol {
        UpstreamProtocol::Responses => "Responses usage",
        UpstreamProtocol::ChatCompletions => "Chat usage",
        UpstreamProtocol::AnthropicMessages => "Anthropic usage",
        UpstreamProtocol::GeminiGenerateContent => "Gemini usageMetadata",
    }
}

/// Metadata, keepalives and an empty assistant role are not generated tokens.
pub(super) fn frame_has_token(
    protocol: asb_core::contracts::UpstreamProtocol,
    value: &serde_json::Value,
) -> bool {
    let nonempty = |value: Option<&serde_json::Value>| {
        value
            .and_then(serde_json::Value::as_str)
            .is_some_and(|s| !s.is_empty())
    };
    match protocol {
        asb_core::contracts::UpstreamProtocol::GeminiGenerateContent => value
            .pointer("/candidates/0/content/parts")
            .and_then(Value::as_array)
            .is_some_and(|parts| {
                parts
                    .iter()
                    .any(|part| nonempty(part.get("text"))
                        || nonempty(part.pointer("/functionCall/name"))
                        || part.pointer("/functionCall/args").and_then(Value::as_object)
                            .is_some_and(|args| !args.is_empty()))
            }),
        asb_core::contracts::UpstreamProtocol::AnthropicMessages => {
            ["/delta/text", "/delta/thinking", "/delta/partial_json",
                "/content_block/text", "/content_block/thinking", "/content_block/name"]
                .iter()
                .any(|path| nonempty(value.pointer(path)))
        }
        asb_core::contracts::UpstreamProtocol::Responses => value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|kind| {
                matches!(
                    kind,
                    "response.output_text.delta"
                        | "response.reasoning_text.delta"
                        | "response.reasoning_summary_text.delta"
                        | "response.function_call_arguments.delta"
                ) && nonempty(value.get("delta"))
            }),
        asb_core::contracts::UpstreamProtocol::ChatCompletions => value
            .get("choices")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|choices| {
                choices.iter().any(|choice| {
                    [
                        "/delta/content",
                        "/delta/reasoning_content",
                        "/delta/reasoning",
                    ]
                    .iter()
                    .any(|path| nonempty(choice.pointer(path)))
                        || choice
                            .pointer("/delta/tool_calls")
                            .and_then(serde_json::Value::as_array)
                            .is_some_and(|tools| {
                                tools
                                    .iter()
                                    .any(|tool| nonempty(tool.pointer("/function/arguments"))
                                        || nonempty(tool.pointer("/function/name")))
                            })
                })
            }),
    }
}

fn parse_gemini(map: &Map<String, Value>) -> Usage {
    let number = |key: &str| map.get(key).and_then(Value::as_u64);
    let input = number("promptTokenCount");
    let visible = number("candidatesTokenCount");
    let reasoning = number("thoughtsTokenCount");
    // Google counts thinking separately from candidates; canonical output and
    // Claude output_tokens include both, while cache reads are already in input.
    let output = visible.map(|v| v.saturating_add(reasoning.unwrap_or(0)));
    Usage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: number("totalTokenCount")
            .or_else(|| input.zip(output).map(|(i, o)| i.saturating_add(o))),
        cached_tokens: number("cachedContentTokenCount"),
        cache_creation_tokens: None,
        reasoning_tokens: reasoning,
    }
}
