use serde_json::Value;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TokenCounters {
    pub input: u64,
    pub cached_input: u64,
    pub cache_write_input: u64,
    pub output: u64,
    pub reasoning_output: u64,
}
impl TokenCounters {
    fn from(value: &Value) -> Result<Self, String> {
        let count = |key: &str| value.get(key).and_then(Value::as_u64)
            .ok_or_else(|| format!("真实调用记录缺少有效的 {key}"));
        let counters = Self { input: count("input_tokens")?, cached_input: count("cached_input_tokens")?,
            cache_write_input: count("cache_write_input_tokens")?, output: count("output_tokens")?,
            reasoning_output: count("reasoning_output_tokens")? };
        if counters.total() != Some(count("total_tokens")?)
            || counters.cached_input.checked_add(counters.cache_write_input).is_none_or(|cache| cache > counters.input)
            || counters.reasoning_output > counters.output {
            return Err("真实调用记录的 token 分项与总数不一致".into());
        }
        Ok(counters)
    }

    pub(crate) fn total(self) -> Option<u64> { self.input.checked_add(self.output) }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CodexTokenEvent {
    pub identity: String,
    pub owner: String,
    pub model: Option<String>,
    pub at: Option<String>,
    pub tokens: TokenCounters,
}

struct Receipt {
    event: CodexTokenEvent,
    turn: String,
}

fn receipt(payload: &Value, at: Option<String>) -> Result<Receipt, String> {
    let id = |key: &str| payload.get(key).and_then(Value::as_str).filter(|id| !id.trim().is_empty())
        .ok_or_else(|| format!("真实调用记录缺少 {key}"));
    let owner = id("thread_id")?;
    let session = id("session_id")?;
    let response = id("response_id")?;
    let turn = id("turn_id")?.to_string();
    id("root_turn_id")?;
    Ok(Receipt { turn, event: CodexTokenEvent {
        identity: serde_json::json!([owner, session, response]).to_string(), owner: owner.to_string(),
        model: None, at, tokens: TokenCounters::from(&payload["usage"])? } })
}

/// Request identity is shared by raw rollouts, fork copies and compaction checkpoints.
pub(crate) fn merge_token_event(
    events: &mut HashMap<String, CodexTokenEvent>, event: CodexTokenEvent,
) -> Result<(), String> {
    use std::collections::hash_map::Entry;
    match events.entry(event.identity.clone()) {
        Entry::Vacant(entry) => { entry.insert(event); }
        Entry::Occupied(mut entry) => {
            let previous = entry.get_mut();
            if previous.tokens != event.tokens {
                return Err(format!("同一 Codex 调用存在冲突的用量记录: {}", event.identity));
            }
            if previous.at.as_ref().zip(event.at.as_ref()).is_some_and(|(a, b)| a != b)
                || previous.model.as_ref().zip(event.model.as_ref()).is_some_and(|(a, b)| a != b) {
                return Err(format!("同一 Codex 调用存在冲突的日期或模型: {}", event.identity));
            }
            if previous.at.is_none() { previous.at = event.at; }
            if previous.model.is_none() { previous.model = event.model; }
        }
    }
    Ok(())
}

pub(crate) fn visit_token_usage(
    path: &Path, session_id: &str,
    mut visitor: impl FnMut(CodexTokenEvent) -> Result<(), String>,
) -> Result<Vec<String>, String> {
    let mut state = TokenUsageState::default();
    let mut issues = Vec::new();
    let read_issues = crate::session_manager::records::visit_records(path, 0, |ordinal, _, value| {
        if let Some(issue) = state.observe(&value, session_id)? {
            issues.push(format!("{} 第 {ordinal} 条: {issue}", path.display()));
        }
        Ok(ControlFlow::Continue(()))
    });
    match read_issues { Ok(warnings) => issues.extend(warnings), Err(error) => issues.push(error) }
    for event in state.events() { visitor(event)?; }
    Ok(issues)
}

#[derive(Default)]
struct TokenUsageState {
    seen_meta: bool,
    models: HashMap<String, Option<String>>,
    receipts: Vec<Receipt>,
}

pub(crate) enum TokenChange {
    Model { turn: String, model: Option<String> },
    Receipt { turn: String, event: CodexTokenEvent },
    Issue(String),
}

pub(crate) fn observe_token(value: &Value, session_id: &str, seen_meta: &mut bool) -> Result<Option<TokenChange>, String> {
    let payload = &value["payload"];
    match value.get("type").and_then(Value::as_str) {
        Some("session_meta") if !*seen_meta => {
            if payload.get("id").and_then(Value::as_str) != Some(session_id) {
                return Err("会话文件身份已改变，未读取另一会话的用量".into());
            }
            *seen_meta = true;
        }
        Some("turn_context") => {
            if let Some(turn) = payload.get("turn_id").and_then(Value::as_str) {
                return Ok(Some(TokenChange::Model { turn: turn.into(), model: payload.get("model")
                    .and_then(Value::as_str).filter(|model| !model.trim().is_empty()).map(str::to_string) }));
            }
        }
        Some("token_usage_record" | "compacted") => {
            let (payload, at) = if value["type"] == "compacted" {
                let Some(record) = payload.get("latest_token_usage_record").filter(|record| !record.is_null()) else { return Ok(None); };
                (record, None)
            } else { (payload, crate::session_manager::parser::timestamp_from(value)) };
            return Ok(Some(match if *seen_meta { receipt(payload, at) } else { Err("真实调用记录出现于会话身份确认之前".into()) } {
                Ok(receipt) => TokenChange::Receipt { turn: receipt.turn, event: receipt.event },
                Err(error) => TokenChange::Issue(error),
            }));
        }
        _ => {}
    }
    Ok(None)
}

impl TokenUsageState {
    fn observe(&mut self, value: &Value, session_id: &str) -> Result<Option<String>, String> {
        match observe_token(value, session_id, &mut self.seen_meta)? {
            Some(TokenChange::Model { turn, model }) => { self.models.insert(turn, model); }
            Some(TokenChange::Receipt { turn, event }) => self.receipts.push(Receipt { turn, event }),
            Some(TokenChange::Issue(issue)) => return Ok(Some(issue)),
            None => {}
        }
        Ok(None)
    }

    fn events(&self) -> impl Iterator<Item = CodexTokenEvent> + '_ {
        self.receipts.iter().map(|record| {
            let mut event = record.event.clone();
            event.model = self.models.get(&record.turn).cloned().flatten();
            event
        })
    }
}

pub(crate) struct SessionTokenSummary {
    pub input: u64,
    pub cached_input: u64,
    pub output: u64,
    pub reasoning_output: Option<u64>,
    pub total: Option<u64>,
    pub model: Option<String>,
}
