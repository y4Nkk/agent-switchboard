use super::{parser, read_index::ReadIndex, records, timeline::{Location, Position}};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::path::Path;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordPosition {
    offset: u64,
    id: String,
    digest: String,
    occurrence: usize,
    at: Option<String>,
    order_at: Option<String>,
    projected: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FilePositions { entries: Vec<RecordPosition>, issues: Vec<String> }

pub(super) fn positions(index: &ReadIndex, paths: &[records::Source], protocol: bool, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<(Vec<Position>, Vec<String>), String> {
    let mut entries = Vec::new();
    let mut issues = Vec::new();
    let mut seen = HashMap::<String, usize>::new();
    for path in paths {
        checkpoint()?;
        let result = indexed(index, path, checkpoint);
        checkpoint()?;
        let file = match result {
            Ok(file) => file,
            Err(error) => { issues.push(error); continue; }
        };
        issues.extend(file.issues);
        for entry in file.entries {
            checkpoint()?;
            if !protocol || entry.projected.is_none() {
                let key = if entry.id.starts_with("claude-") { &entry.id } else { &entry.digest };
                if seen.get(key).is_some_and(|count| *count >= entry.occurrence) { continue; }
                seen.insert(key.clone(), entry.occurrence);
            }
            let id = if protocol { entry.projected.clone().unwrap_or_else(|| format!("protocol-{}-{}", entry.digest, entry.occurrence)) }
                else { entry.id };
            entries.push(Position { id, at: entry.at, order_at: entry.order_at,
                projected: protocol && entry.projected.is_some(),
                location: match path {
                    records::Source::File(path) => Location::Jsonl { path: path.clone(), offset: entry.offset, digest: entry.digest },
                    records::Source::Saved(source) => Location::Saved { source: source.clone(), offset: entry.offset, digest: entry.digest },
                } });
        }
    }
    if !protocol && paths.len() > 1 { entries.sort_by(|left, right| left.order_at.cmp(&right.order_at)); }
    Ok((entries, issues))
}

fn indexed(index: &ReadIndex, source: &records::Source, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<FilePositions, String> {
    match source {
        records::Source::File(path) => index.file("raw-messages", path, || scan(source, checkpoint)),
        records::Source::Saved(saved) => {
            if let Some(value) = index.get("saved-messages", &saved.id, &saved.revision)? { return Ok(value); }
            let value = scan(source, checkpoint)?;
            index.put("saved-messages", &saved.id, &saved.revision, &value)?;
            Ok(value)
        }
    }
}

fn scan(path: &records::Source, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<FilePositions, String> {
    let mut entries = Vec::new();
    let mut occurrences = HashMap::<String, usize>::new();
    let mut prefix = Sha256::new();
    let mut order_at = None;
    let issues = path.visit_observed(0, checkpoint, |_, offset, value| {
        let encoded = value.to_string();
        prefix.update(encoded.as_bytes());
        prefix.update(b"\n");
        let Some(message) = parser::message_from(&value) else { return Ok(ControlFlow::Continue(())); };
        let digest = format!("{:x}", Sha256::digest(encoded.as_bytes()));
        let occurrence = occurrences.entry(digest.clone()).or_default();
        *occurrence += 1;
        if message.at.is_some() { order_at = message.at.clone(); }
        let projected = if value.get("type").and_then(serde_json::Value::as_str) == Some("event_msg")
            && value.pointer("/payload/type").and_then(serde_json::Value::as_str) == Some("item_completed") {
            value.pointer("/payload/item").map(parser::native_identity)
        } else { None };
        entries.push(RecordPosition { offset, id: parser::claude_record_id(&value)
                .unwrap_or_else(|| format!("raw-{:x}-{}", prefix.clone().finalize(), *occurrence)),
            digest, occurrence: *occurrence, at: message.at, order_at: order_at.clone(), projected });
        Ok(ControlFlow::Continue(()))
    })?;
    Ok(FilePositions { entries, issues })
}

pub(super) fn value(path: &Path, offset: u64, digest: &str) -> Result<serde_json::Value, String> {
    let mut found = None;
    records::visit_records(path, offset, |_, _, value| { found = Some(value); Ok(ControlFlow::Break(())) })?;
    let value = found.ok_or("原会话记录已经改变，请重新打开会话")?;
    if parser::record_identity(&value) != digest { return Err("原会话记录已经改变，请重新打开会话".into()); }
    Ok(value)
}
