use super::parse::{self, ClaudeCandidate};
use crate::codex_metering::TokenUsageState;
use crate::session_manager::{read_index::{key, FileRevision, ReadIndex}, records};
use crate::session_manager::read_index::prefix::{self, DigestReader, extend_hash};
use asb_core::contracts::AppKind;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::BufReader;
use std::ops::ControlFlow;
use std::path::Path;
mod saved;
mod compressed;

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
    let before = FileRevision::read(path)?;
    let result = read_snapshot(index, path, app, session);
    if matches!(&result, Err(prefix::SnapshotError::Changed(_)))
        || result.is_err() && FileRevision::read(path).is_ok_and(|current| current != before) {
        return read_snapshot(index, path, app, session).map_err(String::from);
    }
    result.map_err(String::from)
}

fn read_snapshot(index: &ReadIndex, path: &Path, app: AppKind, session: &str) -> Result<FileUsage, prefix::SnapshotError> {
    let revision = FileRevision::read(path)?;
    let cache_key = key(&(app, session, path))?;
    let encoded = revision.encoded()?;
    if let Some(cached) = index.get("usage-file", &cache_key, &encoded)? { return Ok(cached); }
    let previous = index.previous::<FileUsage>("usage-file", &cache_key)?;
    let (next, cacheable) = capture(path, app, session, revision, previous)?;
    verify(path, &next)?;
    if cacheable { index.put("usage-file", &cache_key, &encoded, &next)?; }
    Ok(next)
}

fn capture(path: &Path, app: AppKind, session: &str, revision: FileRevision,
    previous: Option<FileUsage>) -> Result<(FileUsage, bool), String> {
    if crate::session_manager::record_file::compressed(path) {
        return compressed::capture(path, app, session, revision);
    }
    let (mut next, mut hash) = resume(path, app, revision.clone(), previous)?;
    let start = next.offset;
    let mut reader = DigestReader::open(path, start, revision.bytes.saturating_sub(start), &mut hash)?;
    let scan = records::scan_reader(&path.display().to_string(), start, next.ordinal, BufReader::new(&mut reader), &|| Ok(()), |ordinal, _, value| {
        if let Some(issue) = next.state.observe(&value, session)? {
            next.issues.push(format!("{} 第 {ordinal} 条: {issue}", path.display()));
        }
        Ok(ControlFlow::Continue(()))
    });
    let consumed = reader.bytes;
    let cacheable = scan.is_ok();
    match scan {
        Ok(scan) => {
            next.issues.extend(scan.issues);
            next.offset = scan.next_offset;
            next.ordinal = scan.ordinal;
            next.appendable = scan.appendable;
        }
        Err(error) => { next.issues.push(error); next.appendable = false; next.offset = start + consumed; }
    }
    next.prefix = format!("{:x}", hash.finalize());
    next.revision = revision;
    Ok((next, cacheable))
}

fn verify(path: &Path, snapshot: &FileUsage) -> Result<(), prefix::SnapshotError> {
    let compressed = crate::session_manager::record_file::compressed(path);
    let length = if compressed { snapshot.revision.bytes } else { snapshot.offset };
    prefix::verify(path, &snapshot.revision, length, &snapshot.prefix, &|| Ok(()))?;
    if compressed && FileRevision::read(path)? != snapshot.revision {
        return Err(prefix::SnapshotError::Changed(format!("归档在用量读取期间变化：{}", path.display())));
    }
    Ok(())
}

fn resume(path: &Path, app: AppKind, revision: FileRevision, previous: Option<FileUsage>) -> Result<(FileUsage, Sha256), String> {
    if let Some(previous) = previous.filter(|previous| previous.appendable
        && previous.revision.created == revision.created && previous.offset <= revision.bytes) {
        let mut hash = Sha256::new();
        extend_hash(path, 0, previous.offset, &mut hash, &|| Ok(()))?;
        if format!("{:x}", hash.clone().finalize()) == previous.prefix { return Ok((previous, hash)); }
    }
    Ok((FileUsage { revision, offset: 0, ordinal: 0, prefix: String::new(), appendable: false,
        state: UsageState::new(app), issues: Vec::new() }, Sha256::new()))
}

#[cfg(test)]
mod tests;
