use super::{SourceUsage, UsageState};
use crate::session_manager::{history::{self, Transcript}, read_index::{key, ReadIndex}};
use asb_core::contracts::AppKind;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ops::ControlFlow;

#[derive(Serialize, Deserialize)]
struct SavedUsage { offset: u64, prefix: String, appendable: bool, usage: SourceUsage }

pub(super) fn read(index: &ReadIndex, source: &Transcript, app: AppKind, session: &str) -> Result<SourceUsage, String> {
    let cache_key = key(&(app, session, &source.database, &source.id))?;
    if let Some(value) = index.get::<SavedUsage>("usage-saved-events", &cache_key, &source.revision)? { return Ok(value.usage); }
    let previous = index.previous::<SavedUsage>("usage-saved-events", &cache_key)?;
    let (mut value, mut hash) = resume(source, app, previous)?;
    let warnings = history::visit_record_bytes(source, value.offset, |ordinal, position, raw| {
        if position != value.offset { return Err("已保留会话的追加位置不连续".into()); }
        if !raw.iter().all(u8::is_ascii_whitespace) {
            let record = serde_json::from_slice(raw).map_err(|error| format!("已保留会话记录无效：{error}"))?;
            if let Some(issue) = value.usage.state.observe(&record, session)? {
                value.usage.issues.push(format!("已保留会话 {session} 第 {ordinal} 条：{issue}"));
            }
        }
        hash.update(raw);
        value.offset += raw.len() as u64;
        value.appendable = raw.last() == Some(&b'\n');
        Ok(ControlFlow::Continue(()))
    })?;
    value.usage.issues.extend(warnings);
    value.prefix = format!("{:x}", hash.finalize());
    index.put("usage-saved-events", &cache_key, &source.revision, &value)?;
    Ok(value.usage)
}

fn resume(source: &Transcript, app: AppKind, previous: Option<SavedUsage>) -> Result<(SavedUsage, Sha256), String> {
    if let Some(previous) = previous.filter(|previous| previous.appendable) {
        let mut hash = Sha256::new();
        let mut end = 0;
        history::visit_record_bytes(source, 0, |_, position, raw| {
            if position >= previous.offset { return Ok(ControlFlow::Break(())); }
            if position != end { return Err("已保留会话的记录位置不连续".into()); }
            hash.update(raw);
            end += raw.len() as u64;
            Ok(ControlFlow::Continue(()))
        })?;
        if end == previous.offset && format!("{:x}", hash.clone().finalize()) == previous.prefix { return Ok((previous, hash)); }
    }
    Ok((SavedUsage { offset: 0, prefix: String::new(), appendable: false,
        usage: SourceUsage { state: UsageState::new(app), issues: Vec::new() } }, Sha256::new()))
}
