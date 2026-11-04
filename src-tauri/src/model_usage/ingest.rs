use super::{parse, progress::Progress};
use crate::codex_metering::{observe_token, CodexTokenEvent, TokenChange};
use crate::session_manager::{history, read_index::FileRevision, records};
use asb_core::contracts::{AppKind, ModelUsagePhase};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Cursor {
    pub revision: Option<FileRevision>,
    pub offset: u64,
    pub ordinal: usize,
    pub prefix: String,
    pub appendable: bool,
    pub seen_meta: bool,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) enum Observation {
    Codex { event: CodexTokenEvent, turn: String },
    Claude(parse::ClaudeCandidate),
}

impl Observation {
    pub(super) fn identity(&self) -> &str {
        match self { Self::Codex { event, .. } => &event.identity, Self::Claude(value) => &value.event.identity }
    }
    pub(super) fn turn(&self) -> Option<&str> {
        match self { Self::Codex { turn, .. } => Some(turn), Self::Claude(_) => None }
    }
}

pub(super) struct Delta {
    pub cursor: Cursor,
    pub reset: bool,
    pub observations: Vec<(usize, Observation)>,
    pub models: BTreeMap<String, Option<String>>,
}

impl Delta {
    pub(super) fn new(previous: Option<Cursor>) -> Self {
        Self { reset: previous.is_none(), cursor: previous.unwrap_or_default(),
            observations: Vec::new(), models: BTreeMap::new() }
    }

    pub(super) fn observe(&mut self, ordinal: usize, app: AppKind, session: &str, value: &serde_json::Value) -> Result<(), String> {
        match app {
            AppKind::Codex => match observe_token(value, session, &mut self.cursor.seen_meta)? {
                Some(TokenChange::Model { turn, model }) => { self.models.insert(turn, model); }
                Some(TokenChange::Receipt { turn, event }) => self.observations.push((ordinal, Observation::Codex { turn, event })),
                Some(TokenChange::Issue(issue)) => self.cursor.issues.push(format!("第 {ordinal} 条: {issue}")),
                None => {}
            },
            AppKind::Claude => {
                if value.get("type").and_then(serde_json::Value::as_str) != Some("assistant") { return Ok(()); }
                let Some(message) = value.get("message").filter(|message| message.get("usage").is_some()) else { return Ok(()); };
                match parse::candidate(message, value, session) {
                    Ok(value) => self.observations.push((ordinal, Observation::Claude(value))),
                    Err(issue) => self.cursor.issues.push(format!("第 {ordinal} 条: {issue}")),
                }
            }
        }
        Ok(())
    }
}

pub(super) fn read(source: &records::Source, app: AppKind, session: &str,
    previous: Option<Cursor>, progress: &Progress<'_>) -> Result<Delta, String> {
    match source {
        records::Source::File(path) => super::stream::read(path, app, session, previous, progress),
        records::Source::Saved(source) => saved(source, app, session, previous, progress),
    }
}

fn saved(source: &history::Transcript, app: AppKind, session: &str,
    previous: Option<Cursor>, progress: &Progress<'_>) -> Result<Delta, String> {
    let mut delta = Delta::new(previous.filter(|cursor| cursor.appendable));
    let start = delta.cursor.offset;
    history::visit_committed_usage(source, start,
        |total| progress.bytes(ModelUsagePhase::Reading, total), |ordinal, position, raw| {
            if position != delta.cursor.offset { return Err("已保留会话的追加位置不连续".into()); }
            if !raw.iter().all(u8::is_ascii_whitespace) {
                let value = serde_json::from_slice(raw).map_err(|error| format!("已保留会话记录无效：{error}"))?;
                delta.observe(ordinal, app, session, &value)?;
            }
            delta.cursor.offset += raw.len() as u64;
            delta.cursor.ordinal = ordinal;
            delta.cursor.appendable = raw.last() == Some(&b'\n');
            progress.advance_bytes(raw.len() as u64);
            Ok(())
        })?;
    Ok(delta)
}
