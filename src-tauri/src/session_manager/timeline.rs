mod native_index;

use super::{jsonl, native, parser, read_index::ReadIndex, records, SessionMessage, SessionSource};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Mutex;

static MESSAGE_INDEX_GATE: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) enum Location {
    Native { turn: String, item: String, realtime: bool },
    Jsonl { path: PathBuf, offset: u64, digest: String },
    Converted { path: PathBuf, offset: u64, digest: String },
    Saved { source: super::history::Transcript, offset: u64, digest: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Position {
    pub id: String,
    pub at: Option<String>,
    pub order_at: Option<String>,
    pub projected: bool,
    pub location: Location,
}

pub(super) struct MessageIndex {
    pub entries: Vec<Position>,
    pub issues: Vec<String>,
    connection: Option<Connection>,
    thread: String,
    readers: jsonl::Readers,
}

pub(super) fn index(source: &SessionSource) -> Result<MessageIndex, String> {
    index_observed(source, &|| Ok(()))
}

pub(super) fn index_observed(source: &SessionSource, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<MessageIndex, String> {
    let _guard = super::read_observer::lock(&MESSAGE_INDEX_GATE, checkpoint)?;
    let index = ReadIndex::open_observed(&source.index_root, checkpoint)?;
    if let Some(path) = &source.converted {
        let (entries, issues) = index.file("converted-messages", path, || converted_positions(path, checkpoint))?;
        return Ok(MessageIndex { entries, issues, connection: None, thread: source.meta.session_id.clone(), readers: Default::default() });
    }
    let mut result = MessageIndex { entries: Vec::new(), issues: Vec::new(), connection: None, thread: source.meta.session_id.clone(), readers: Default::default() };
    if let Some(history) = &source.native_history {
        let snapshot = native::open(history).and_then(|connection| {
            connection.execute_batch("BEGIN DEFERRED").map_err(|error| error.to_string())?;
            let (entries, issues) = native_index::positions_observed(&index, &connection, history, &source.meta.session_id, checkpoint)?;
            Ok((connection, entries, issues))
        });
        match snapshot {
            Ok((connection, entries, issues)) => { result.connection = Some(connection); result.entries = entries; result.issues.extend(issues); }
            Err(error) => result.issues.push(error),
        }
    }
    checkpoint()?;
    let protocol = !result.entries.is_empty();
    let (raw, issues) = jsonl::positions(&index, &source.record_sources(), protocol, checkpoint)?;
    result.issues.extend(issues);
    result.issues.extend(source.retained.iter().filter_map(|source| source.issue.clone()));
    let mut identities: HashSet<_> = result.entries.iter().map(|entry| entry.id.clone()).collect();
    result.entries.extend(raw.into_iter().filter(|entry| !entry.projected || identities.insert(entry.id.clone())));
    if protocol { result.entries.sort_by(|left, right| left.at.cmp(&right.at)); }
    if result.entries.is_empty() && source.paths.is_empty() {
        result.issues.push("仅保留会话身份，原始记录和原生消息正文均不可用".into());
    }
    Ok(result)
}

fn converted_positions(path: &std::path::Path, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<(Vec<Position>, Vec<String>), String> {
    let mut entries = Vec::new();
    let scan = records::scan_records_observed(path, 0, 0, checkpoint, |ordinal, offset, value| {
        if ordinal == 1 { return Ok(ControlFlow::Continue(())); }
        let digest = parser::record_identity(&value);
        let message: SessionMessage = serde_json::from_value(value).map_err(|error| error.to_string())?;
        entries.push(Position { id: message.id, at: message.at.clone(), order_at: message.at, projected: false,
            location: Location::Converted { path: path.into(), offset, digest } });
        Ok(ControlFlow::Continue(()))
    })?;
    Ok((entries, scan.issues))
}

impl MessageIndex {
    pub(super) fn load(&self, entry: &Position) -> Result<SessionMessage, String> {
        self.load_observed(entry, &|| Ok(()))
    }

    fn load_observed(&self, entry: &Position, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<SessionMessage, String> {
        checkpoint()?;
        let mut message = match &entry.location {
            Location::Saved { source, offset, digest } => {
                let value = records::Source::Saved(source.clone()).load(*offset)?;
                if parser::record_identity(&value) != *digest { return Err("已保留会话在读取期间改变，请重新打开".into()); }
                let mut message = parser::message_from(&value).ok_or("已保留消息内容不可读")?;
                message.images = super::history::bind_images(source, message.images);
                message
            }
            Location::Jsonl { path, offset, digest } => parser::message_from(&self.readers.value(path, *offset, digest, checkpoint)?)
                .ok_or("原消息内容已不可读")?,
            Location::Converted { path, offset, digest } => serde_json::from_value(self.readers.value(path, *offset, digest, checkpoint)?)
                .map_err(|error| error.to_string())?,
            Location::Native { turn, item, realtime } => {
                let connection = self.connection.as_ref().ok_or("缺少原生消息读取事务")?;
                let value = native_index::value(connection, &self.thread, turn, item, *realtime)?;
                if parser::native_identity(&value) != entry.id { return Err("原生消息已经改变，请重新打开会话".into()); }
                parser::message_from(&value).ok_or("原消息内容已不可读")?
            }
        };
        message.id = entry.id.clone();
        message.at = entry.at.clone();
        Ok(message)
    }

    pub(super) fn load_batch<T>(&self, entries: &[Position], mut summarize: impl FnMut(SessionMessage) -> T)
        -> Vec<Result<T, String>> {
        let mut order: Vec<_> = entries.iter().enumerate().collect();
        order.sort_by_key(|(ordinal, entry)| match &entry.location {
            Location::Jsonl { path, offset, .. } if super::record_file::compressed(path) =>
                (Some(path.clone()), *offset, 0),
            _ => (None, 0, *ordinal),
        });
        let mut results = std::collections::BTreeMap::new();
        for (ordinal, entry) in order { results.insert(ordinal, self.load(entry).map(&mut summarize)); }
        results.into_values().collect()
    }

    pub(super) fn find(&self, id: &str) -> Result<SessionMessage, String> {
        let entry = self.entries.iter().find(|entry| entry.id == id)
            .ok_or_else(|| if self.issues.is_empty() { "原消息已改变或不存在；请重新打开会话".into() } else { self.issues.join("；") })?;
        self.load(entry)
    }

    pub(super) fn visit(&self, visitor: impl FnMut(SessionMessage) -> Result<ControlFlow<()>, String>) -> Result<Vec<String>, String> {
        self.visit_observed(|_, _| Ok(()), &|| Ok(()), visitor)
    }

    pub(super) fn visit_observed(&self, mut progress: impl FnMut(usize, usize) -> Result<(), String>,
        checkpoint: &dyn Fn() -> Result<(), String>,
        mut visitor: impl FnMut(SessionMessage) -> Result<ControlFlow<()>, String>) -> Result<Vec<String>, String> {
        let mut issues = self.issues.clone();
        progress(0, self.entries.len())?;
        for (completed, entry) in self.entries.iter().enumerate() {
            checkpoint()?;
            let stop = match self.load_observed(entry, checkpoint) {
                Ok(message) => visitor(message)?.is_break(),
                Err(error) => { issues.push(error); false },
            };
            progress(completed + 1, self.entries.len())?;
            if stop { break; }
        }
        let mut seen = HashSet::new();
        issues.retain(|issue| seen.insert(issue.clone()));
        Ok(issues)
    }
}

#[cfg(test)]
mod tests;
