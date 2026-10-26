//! Cross-request tool-call history for the HTTP Responses→Chat bridge.
//!
//! Codex sends follow-up turns as `previous_response_id` plus bare
//! `function_call_output` items, while Chat upstreams require the original
//! call message immediately before each tool result. The gateway therefore
//! indexes the Responses output it rendered for the client and restores the
//! referenced call items before conversion.
//!
//! Isolation: entries are bound to the client session that produced them, so
//! lookups never cross sessions. Deliberate route changes inside one session
//! keep restoring — the restored items are portable, plain tool calls.
//! Subagent flows that omit `previous_response_id` may fall back to a
//! `call_id` cached under exactly one response of the same session; an
//! ambiguous id restores nothing and the request fails with its existing
//! unpaired-tool-output error.

use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use tiny_http::Header;

const MAX_ENTRIES: usize = 512;
const CALL_TYPES: [&str; 3] = ["function_call", "custom_tool_call", "tool_search_call"];
const OUTPUT_TYPES: [&str; 3] = [
    "function_call_output",
    "custom_tool_call_output",
    "tool_search_output",
];
const MISSING_PREVIOUS: &str =
    "previous_response_id 无法解析为已缓存的响应；请提交完整 input 上下文";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct HistoryBinding {
    session: Option<String>,
}

impl HistoryBinding {
    /// The session that owns a conversation: `client_metadata.session_id`
    /// from the Codex request body, else the `session_id`/`thread_id` header.
    pub(crate) fn for_request(body: &[u8], incoming: Option<&[Header]>) -> HistoryBinding {
        let session = serde_json::from_slice::<Value>(body)
            .ok()
            .and_then(|value| {
                value
                    .pointer("/client_metadata/session_id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .map(str::to_string)
            })
            .or_else(|| {
                incoming?.iter().find_map(|header| {
                    let matches =
                        header.field.equiv("session_id") || header.field.equiv("thread_id");
                    matches
                        .then(|| header.value.as_str().trim().to_string())
                        .filter(|value| !value.is_empty())
                })
            });
        HistoryBinding { session }
    }
}

#[derive(Default)]
pub(crate) struct CodexToolHistory {
    inner: RwLock<HistoryInner>,
}

#[derive(Default)]
struct HistoryInner {
    entries: HashMap<(String, String), Entry>,
    order: VecDeque<(String, String)>,
}

struct Entry {
    calls: Vec<(String, Value)>,
}

impl CodexToolHistory {
    /// Indexes a gateway-rendered Responses object. Best effort: unparseable
    /// bodies or responses without call items record nothing.
    pub(crate) fn record_response(&self, binding: &HistoryBinding, rendered: &[u8]) {
        let Some(session) = &binding.session else {
            return;
        };
        let Ok(value) = serde_json::from_slice::<Value>(rendered) else {
            return;
        };
        if value.get("status").and_then(Value::as_str) != Some("completed") {
            return;
        }
        let Some(id) = value
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
        else {
            return;
        };
        let Some(output) = value.get("output").and_then(Value::as_array) else {
            return;
        };
        let mut seen = HashSet::new();
        let mut calls = Vec::new();
        for item in output {
            let Some(call_id) = call_id_of(item) else {
                continue;
            };
            if seen.insert(call_id.clone()) {
                calls.push((call_id, item.clone()));
            }
        }
        if calls.is_empty() {
            return;
        }
        let Ok(mut inner) = self.inner.write() else {
            return;
        };
        let key = (session.clone(), id);
        if !inner.entries.contains_key(&key) {
            inner.order.push_back(key.clone());
        }
        inner.entries.insert(key, Entry { calls });
        while inner.order.len() > MAX_ENTRIES {
            let Some(oldest) = inner.order.pop_front() else {
                break;
            };
            inner.entries.remove(&oldest);
        }
    }

    /// Restores missing call items and fills absent fields on present ones.
    /// A resolvable `previous_response_id` is consumed (removed) from the
    /// request; an unresolvable one is a loud error, mirroring the WebSocket
    /// continuation contract.
    pub(crate) fn enrich(
        &self,
        binding: &HistoryBinding,
        body: &mut Value,
    ) -> Result<usize, String> {
        let Some(map) = body.as_object_mut() else {
            return Ok(0);
        };
        let previous = map
            .get("previous_response_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string);
        let changed = {
            let inner = self
                .inner
                .read()
                .map_err(|_| "工具历史缓存不可用".to_string())?;
            if let Some(id) = previous.as_deref() {
                if inner.entry(id, binding).is_none() {
                    return Err(MISSING_PREVIOUS.into());
                }
            }
            let Some(input) = map.get_mut("input") else {
                return Ok(0);
            };
            let original_was_object = matches!(input, Value::Object(_));
            let mut items = match std::mem::take(input) {
                Value::Array(items) => items,
                Value::Object(object) => vec![Value::Object(object)],
                other => {
                    *input = other;
                    return Ok(0);
                }
            };
            let requested = requested_call_ids(&items);
            let lookup = Lookup {
                previous: previous.as_deref().and_then(|id| inner.entry(id, binding)),
                fallback: inner.unique_calls(binding, &requested),
            };
            let (restored, enriched) = restore_items(&lookup, &mut items)?;
            let unchanged_single =
                restored + enriched == 0 && original_was_object && items.len() == 1;
            *map.get_mut("input").expect("taken above") = if unchanged_single {
                items.into_iter().next().expect("single item")
            } else {
                Value::Array(items)
            };
            restored + enriched
        };
        if previous.is_some() {
            map.remove("previous_response_id");
        }
        Ok(changed)
    }
}

/// `call_id` of a completed call item (`function_call` and friends).
fn call_id_of(item: &Value) -> Option<String> {
    let kind = item.get("type").and_then(Value::as_str)?;
    if !CALL_TYPES.contains(&kind) {
        return None;
    }
    item.get("call_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

/// `call_id` of a tool-result item (`function_call_output` and friends).
fn output_call_id(item: &Value) -> Option<String> {
    let kind = item.get("type").and_then(Value::as_str)?;
    if !OUTPUT_TYPES.contains(&kind) {
        return None;
    }
    item.get("call_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

fn requested_call_ids(items: &[Value]) -> HashSet<String> {
    items
        .iter()
        .filter_map(|item| output_call_id(item).or_else(|| call_id_of(item)))
        .collect()
}

struct Lookup<'a> {
    previous: Option<&'a Entry>,
    fallback: Vec<&'a Entry>,
}

impl Lookup<'_> {
    fn call(&self, call_id: &str) -> Option<&Value> {
        if let Some(entry) = self.previous {
            if let Some(item) = entry.call(call_id) {
                return Some(item);
            }
        }
        self.fallback.iter().find_map(|entry| entry.call(call_id))
    }
}

impl Entry {
    fn call(&self, call_id: &str) -> Option<&Value> {
        self.calls
            .iter()
            .find(|(existing, _)| existing == call_id)
            .map(|(_, item)| item)
    }
}

impl HistoryInner {
    fn entry(&self, id: &str, binding: &HistoryBinding) -> Option<&Entry> {
        self.entries.get(&(binding.session.as_ref()?.clone(), id.to_string()))
    }

    /// Same-session entries that cache a requested `call_id` under exactly
    /// one response, in cache order.
    fn unique_calls(&self, binding: &HistoryBinding, requested: &HashSet<String>) -> Vec<&Entry> {
        let Some(session) = &binding.session else {
            return Vec::new();
        };
        let mut owners: HashMap<&str, usize> = HashMap::new();
        for ((owner, _), entry) in &self.entries {
            if owner != session {
                continue;
            }
            for (call_id, _) in &entry.calls {
                if requested.contains(call_id) {
                    *owners.entry(call_id.as_str()).or_default() += 1;
                }
            }
        }
        let unique: HashSet<&str> = owners
            .into_iter()
            .filter(|(_, count)| *count == 1)
            .map(|(call_id, _)| call_id)
            .collect();
        let mut selected = Vec::new();
        for key in &self.order {
            if &key.0 != session {
                continue;
            }
            let Some(entry) = self.entries.get(key) else {
                continue;
            };
            if entry
                .calls
                .iter()
                .any(|(call_id, _)| unique.contains(call_id.as_str()))
            {
                selected.push(entry);
            }
        }
        selected
    }
}

/// One pass over the request items: restores a cached call right before the
/// output that references it and fills absent fields on present calls.
fn restore_items(lookup: &Lookup<'_>, items: &mut Vec<Value>) -> Result<(usize, usize), String> {
    let mut seen: HashSet<String> = items.iter().filter_map(|item| call_id_of(item)).collect();
    let mut restored = 0;
    let mut enriched = 0;
    let mut output = Vec::with_capacity(items.len());
    for mut item in std::mem::take(items) {
        if let Some(call_id) = output_call_id(&item).filter(|call_id| !seen.contains(call_id)) {
            if let Some(cached) = lookup.call(&call_id).cloned() {
                seen.insert(call_id);
                output.push(cached);
                restored += 1;
            }
        }
        if let Some(call_id) = call_id_of(&item) {
            seen.insert(call_id.clone());
            if let Some(cached) = lookup.call(&call_id) {
                if enrich_call_item(&mut item, cached) {
                    enriched += 1;
                }
            }
        }
        output.push(item);
    }
    *items = output;
    Ok((restored, enriched))
}

fn enrich_call_item(item: &mut Value, cached: &Value) -> bool {
    let mut changed = false;
    for key in [
        "name",
        "namespace",
        "arguments",
        "input",
        "status",
        "execution",
    ] {
        let present = item
            .get(key)
            .is_some_and(|value| !value.is_null() && value.as_str() != Some(""));
        if present {
            continue;
        }
        let Some(value) = cached.get(key).filter(|value| !value.is_null()) else {
            continue;
        };
        if let Some(object) = item.as_object_mut() {
            object.insert(key.to_string(), value.clone());
            changed = true;
        }
    }
    changed
}

/// Records `response.completed` payloads from the client-bound Responses SSE
/// stream; every other byte passes through untouched.
pub(crate) struct StreamRecorder {
    history: Arc<CodexToolHistory>,
    binding: HistoryBinding,
    buffer: Vec<u8>,
}

impl StreamRecorder {
    pub(crate) fn new(history: Arc<CodexToolHistory>, binding: HistoryBinding) -> Self {
        Self {
            history,
            binding,
            buffer: Vec::new(),
        }
    }

    /// Indexes one non-streaming rendered Responses body directly.
    pub(crate) fn record_json(&mut self, rendered: &[u8]) {
        self.history.record_response(&self.binding, rendered);
    }

    pub(crate) fn feed(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);
        while let Some(position) = find_block_end(&self.buffer) {
            let block: Vec<u8> = self.buffer.drain(..position).collect();
            self.record_block(&block);
        }
        if self.buffer.len() > 4 * 1024 * 1024 {
            self.buffer.clear();
        }
    }

    fn record_block(&self, block: &[u8]) {
        let Ok(text) = std::str::from_utf8(block) else {
            return;
        };
        let data = text
            .lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .collect::<Vec<_>>()
            .join("\n");
        let Ok(value) = serde_json::from_str::<Value>(&data) else {
            return;
        };
        if value.get("type").and_then(Value::as_str) != Some("response.completed") {
            return;
        }
        let response = value.get("response").unwrap_or(&value);
        self.history
            .record_response(&self.binding, &response.to_string().into_bytes());
    }
}

fn find_block_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|position| position + 2)
}

#[cfg(test)]
mod tests;
