use super::{db_error, store, HistoryRoot, Preservation, Transcript};
use crate::session_manager::{SessionMeta, SessionSource};
use rusqlite::{params, Connection};
use std::collections::BTreeMap;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

pub(super) fn roots(connection: &Connection) -> Result<Vec<HistoryRoot>, String> {
    let mut statement = connection.prepare("SELECT path,automatic,issue FROM roots ORDER BY path").map_err(db_error)?;
    let values = statement.query_map([], |row| Ok((row.get::<_, String>(0)?,row.get::<_, bool>(1)?,row.get::<_, Option<String>>(2)?))).map_err(db_error)?;
    values.map(|value| value.map(|(path,automatic,issue)| HistoryRoot { available: Path::new(&path).join("projects").is_dir(),
        removable: !automatic, path, issue })
        .map_err(db_error)).collect()
}

pub(super) fn sources(connection: &Connection, database: &Path) -> Result<Vec<SessionSource>, String> {
    sources_observed(connection, database, &|| Ok(()))
}

pub(super) fn sources_observed(connection: &Connection, database: &Path, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<Vec<SessionSource>, String> {
    let mut statement = connection.prepare("SELECT id,origin,meta,saved_at,available,issue,revision,capture_issue FROM transcripts
        WHERE session_id NOT IN (SELECT id FROM deleted_sessions)
        AND (parent_id IS NULL OR parent_id NOT IN (SELECT id FROM deleted_sessions)) ORDER BY origin,generation").map_err(db_error)?;
    let mut rows = statement.query([]).map_err(db_error)?;
    let mut sources = BTreeMap::<String, SessionSource>::new();
    while let Some(row) = rows.next().map_err(db_error)? {
        checkpoint()?;
        let id: String = row.get(0).map_err(db_error)?;
        let origin = PathBuf::from(row.get::<_, String>(1).map_err(db_error)?);
        let mut meta: SessionMeta = serde_json::from_str(&row.get::<_, String>(2).map_err(db_error)?)
            .map_err(|error| format!("Claude 会话保留身份无效：{error}"))?;
        let issues = [row.get::<_, Option<String>>(5).map_err(db_error)?, row.get::<_, Option<String>>(7).map_err(db_error)?]
            .into_iter().flatten().collect::<Vec<_>>();
        let issue = (!issues.is_empty()).then(|| issues.join("；"));
        let preservation = Preservation { saved_at: row.get(3).map_err(db_error)?,
            source_available: row.get(4).map_err(db_error)?, needs_attention: issue.is_some() };
        meta.preservation = Some(preservation.clone());
        meta.resume_command = None;
        let transcript = Transcript { database: database.into(), id, origin: origin.clone(),
            revision: row.get(6).map_err(db_error)?, issue, parent: meta.parent_session_id.clone() };
        if let Some(source) = sources.get_mut(&meta.session_id) {
            merge(&mut source.meta, meta, preservation);
            source.paths.push(origin);
            source.retained.push(transcript);
        } else {
            sources.insert(meta.session_id.clone(), SessionSource { meta, paths: vec![origin],
                native_history: None, converted: None, retained: vec![transcript],
                index_root: database.parent().and_then(Path::parent).ok_or("保留库目录无效")?.into() });
        }
    }
    Ok(sources.into_values().collect())
}

fn merge(current: &mut SessionMeta, next: SessionMeta, preservation: Preservation) {
    if next.last_active_at > current.last_active_at {
        current.last_active_at = next.last_active_at;
        current.title = next.title;
    }
    if next.created_at.is_some() && (current.created_at.is_none() || next.created_at < current.created_at) {
        current.created_at = next.created_at;
        current.summary = next.summary;
    }
    if let Some(saved) = &mut current.preservation {
        saved.saved_at = saved.saved_at.clone().max(preservation.saved_at);
        saved.source_available |= preservation.source_available;
        saved.needs_attention |= preservation.needs_attention;
    }
}

pub(super) fn visit_observed(
    source: &Transcript, offset: u64, checkpoint: &dyn Fn() -> Result<(), String>,
    mut visitor: impl FnMut(usize, u64, serde_json::Value) -> Result<ControlFlow<()>, String>,
) -> Result<Vec<String>, String> {
    visit_bytes_observed(source, offset, checkpoint, |ordinal, position, raw| {
        if raw.iter().all(u8::is_ascii_whitespace) { return Ok(ControlFlow::Continue(())); }
        let value = serde_json::from_slice(raw).map_err(|error| format!("已保留的 Claude 记录无效：{error}"))?;
        visitor(ordinal, position, value)
    })
}

pub(super) fn visit_bytes(
    source: &Transcript, offset: u64,
    visitor: impl FnMut(usize, u64, &[u8]) -> Result<ControlFlow<()>, String>,
) -> Result<Vec<String>, String> {
    visit_bytes_observed(source, offset, &|| Ok(()), visitor)
}

fn visit_bytes_observed(
    source: &Transcript, offset: u64, checkpoint: &dyn Fn() -> Result<(), String>,
    mut visitor: impl FnMut(usize, u64, &[u8]) -> Result<ControlFlow<()>, String>,
) -> Result<Vec<String>, String> {
    checkpoint()?;
    let mut connection = store::read(&source.database)?;
    let transaction = connection.transaction().map_err(db_error)?;
    let revision: String = transaction.query_row("SELECT revision FROM transcripts WHERE id=?1", [&source.id], |row| row.get(0)).map_err(db_error)?;
    if revision != source.revision { return Err("已保留会话在读取期间变化，请刷新后重试".into()); }
    let mut statement = transaction.prepare("SELECT ordinal,position,raw FROM records WHERE source=?1 AND position>=?2 ORDER BY position")
        .map_err(db_error)?;
    let mut rows = statement.query(params![source.id, offset]).map_err(db_error)?;
    while let Some(row) = rows.next().map_err(db_error)? {
        checkpoint()?;
        let raw: Vec<u8> = row.get(2).map_err(db_error)?;
        if visitor(row.get(0).map_err(db_error)?, row.get(1).map_err(db_error)?, &raw)?.is_break() { break; }
    }
    Ok(Vec::new())
}
