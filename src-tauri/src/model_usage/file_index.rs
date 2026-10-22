use super::parse::{self, ClaudeCandidate};
use crate::codex_metering::TokenUsageState;
use crate::session_manager::{read_index::{key, FileRevision, ReadIndex}, records};
use asb_core::contracts::AppKind;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::ops::ControlFlow;
use std::path::Path;
mod saved;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) enum UsageState {
    Codex(TokenUsageState),
    Claude(HashMap<String, ClaudeCandidate>),
}

impl UsageState {
    fn new(app: AppKind) -> Self {
        match app { AppKind::Codex => Self::Codex(TokenUsageState::default()), AppKind::Claude => Self::Claude(HashMap::new()) }
    }

    fn observe(&mut self, value: &serde_json::Value, session: &str) -> Result<Option<String>, String> {
        match self {
            Self::Codex(state) => state.observe(value, session),
            Self::Claude(state) => Ok(parse::observe(value, session, state).err()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FileUsage {
    revision: FileRevision,
    offset: u64,
    ordinal: usize,
    prefix: String,
    appendable: bool,
    pub state: UsageState,
    pub issues: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct SourceUsage {
    pub state: UsageState,
    pub issues: Vec<String>,
}

pub(super) fn read_source(index: &ReadIndex, source: &records::Source, app: AppKind, session: &str) -> Result<SourceUsage, String> {
    match source {
        records::Source::File(path) => {
            let value = read(index, path, app, session)?;
            Ok(SourceUsage { state: value.state, issues: value.issues })
        }
        records::Source::Saved(source) => saved::read(index, source, app, session),
    }
}

pub(super) fn read(index: &ReadIndex, path: &Path, app: AppKind, session: &str) -> Result<FileUsage, String> {
    let revision = FileRevision::read(path)?;
    let cache_key = key(&(app, session, path))?;
    let encoded = revision.encoded()?;
    if let Some(cached) = index.get("usage-file", &cache_key, &encoded)? { return Ok(cached); }
    let previous = index.previous::<FileUsage>("usage-file", &cache_key)?;
    let (mut next, mut hash) = resume(path, app, revision.clone(), previous)?;
    let start = next.offset;
    let scan = records::scan_records(path, start, next.ordinal, |ordinal, _, value| {
        if let Some(issue) = next.state.observe(&value, session)? {
            next.issues.push(format!("{} 第 {ordinal} 条: {issue}", path.display()));
        }
        Ok(ControlFlow::Continue(()))
    });
    if FileRevision::read(path)? != revision { return Err(format!("用量来源在读取期间发生变化，请重试：{}", path.display())); }
    match scan {
        Ok(scan) => {
            next.issues.extend(scan.issues);
            next.offset = scan.next_offset;
            next.ordinal = scan.ordinal;
            next.appendable = scan.appendable;
            extend_hash(path, start, next.offset.saturating_sub(start), &mut hash)?;
            next.prefix = format!("{:x}", hash.finalize());
            next.revision = revision.clone();
            if FileRevision::read(path)? != revision { return Err(format!("用量来源在读取期间发生变化，请重试：{}", path.display())); }
            index.put("usage-file", &cache_key, &encoded, &next)?;
        }
        Err(error) => { next.issues.push(error); next.appendable = false; }
    }
    Ok(next)
}

fn resume(path: &Path, app: AppKind, revision: FileRevision, previous: Option<FileUsage>) -> Result<(FileUsage, Sha256), String> {
    if let Some(previous) = previous.filter(|previous| previous.appendable
        && previous.revision.created == revision.created && previous.offset <= revision.bytes) {
        let mut hash = Sha256::new();
        extend_hash(path, 0, previous.offset, &mut hash)?;
        if format!("{:x}", hash.clone().finalize()) == previous.prefix { return Ok((previous, hash)); }
    }
    Ok((FileUsage { revision, offset: 0, ordinal: 0, prefix: String::new(), appendable: false,
        state: UsageState::new(app), issues: Vec::new() }, Sha256::new()))
}

fn extend_hash(path: &Path, start: u64, mut length: u64, hash: &mut Sha256) -> Result<(), String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    file.seek(SeekFrom::Start(start)).map_err(|error| error.to_string())?;
    let mut buffer = [0; 64 * 1024];
    while length > 0 {
        let size = length.min(buffer.len() as u64) as usize;
        file.read_exact(&mut buffer[..size]).map_err(|error| error.to_string())?;
        hash.update(&buffer[..size]);
        length -= size as u64;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
