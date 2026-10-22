use super::{Location, Position};
use crate::session_manager::{parser, read_index::{key, ReadIndex}, records};
use chrono::{TimeZone, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

#[cfg(test)]
thread_local! { static SOURCE_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[cfg(test)]
pub(super) fn source_read_count() -> usize { SOURCE_READS.with(|count| count.get()) }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct NativeRow {
    turn: String,
    item: String,
    created: i64,
    version: i64,
    realtime: bool,
    ordinal: i64,
}

#[derive(Serialize, Deserialize)]
struct CachedEntry { row: NativeRow, position: Option<Position>, issue: Option<String> }

pub(super) fn positions(index: &ReadIndex, connection: &Connection, history: &Path, thread: &str) -> Result<(Vec<Position>, Vec<String>), String> {
    let cache_key = key(&(history, thread))?;
    let previous = index.previous::<Vec<CachedEntry>>("native-messages", &cache_key)?.unwrap_or_default();
    let mut previous: HashMap<_, _> = previous.into_iter().map(|entry|
        ((entry.row.turn.clone(), entry.row.item.clone(), entry.row.realtime), entry)).collect();
    let mut next = Vec::new();
    let mut changed = false;
    for row in rows(connection, thread)? {
        let cached = previous.remove(&(row.turn.clone(), row.item.clone(), row.realtime));
        let entry = match cached {
            Some(entry) if !row.realtime && entry.row == row => entry,
            _ => {
                changed = true;
                match position(connection, thread, &row) {
                    Ok(position) => CachedEntry { row, position, issue: None },
                    Err(error) => CachedEntry { row, position: None, issue: Some(format!("会话 {thread} 的一条原生消息不可读: {error}")) },
                }
            }
        };
        next.push(entry);
    }
    if changed || !previous.is_empty() { index.put("native-messages", &cache_key, "current", &next)?; }
    let mut positions = Vec::new();
    let mut issues = Vec::new();
    for entry in next {
        if let Some(position) = entry.position { positions.push(position); }
        if let Some(issue) = entry.issue { issues.push(issue); }
    }
    Ok((positions, issues))
}

fn rows(connection: &Connection, thread: &str) -> Result<Vec<NativeRow>, String> {
    let mut statement = connection.prepare(
        "SELECT turn_id,item_id,created_at_ms,updated_at_ordinal,0,rollout_ordinal FROM thread_items WHERE thread_id=?1 \
         UNION ALL SELECT '',r.item_id,r.created_at_ms,0,1,r.rollout_ordinal FROM thread_realtime_items r \
         WHERE r.thread_id=?1 AND NOT EXISTS (SELECT 1 FROM thread_items i WHERE i.thread_id=r.thread_id AND i.item_id=r.item_id) \
         ORDER BY 6,3,2").map_err(|error| error.to_string())?;
    let rows = statement.query_map([thread], |row| Ok(NativeRow { turn: row.get(0)?, item: row.get(1)?,
        created: row.get(2)?, version: row.get(3)?, realtime: row.get(4)?, ordinal: row.get(5)? })).map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
}

fn position(connection: &Connection, thread: &str, row: &NativeRow) -> Result<Option<Position>, String> {
    let value = value(connection, thread, &row.turn, &row.item, row.realtime)?;
    if parser::message_from(&value).is_none() { return Ok(None); }
    let at = Utc.timestamp_millis_opt(row.created).single().map(|at| at.to_rfc3339());
    Ok(Some(Position { id: parser::native_identity(&value), at: at.clone(), order_at: at, projected: true,
        location: Location::Native { turn: row.turn.clone(), item: row.item.clone(), realtime: row.realtime } }))
}

pub(super) fn value(connection: &Connection, thread: &str, turn: &str, item: &str, realtime: bool) -> Result<Value, String> {
    #[cfg(test)]
    SOURCE_READS.with(|count| count.set(count.get() + 1));
    let sql = if realtime {
        "SELECT CASE WHEN length(CAST(item_json AS BLOB))<=?4 THEN item_json END FROM thread_realtime_items WHERE thread_id=?1 AND item_id=?3"
    } else {
        "SELECT CASE WHEN length(CAST(item_json AS BLOB))<=?4 THEN item_json END FROM thread_items WHERE thread_id=?1 AND turn_id=?2 AND item_id=?3"
    };
    let raw: Option<String> = connection.query_row(sql, params![thread, turn, item, records::MAX_RECORD_BYTES], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    let raw = raw.ok_or("原生消息超过 32 MiB 单条读取上限")?;
    serde_json::from_str(&raw).map_err(|error| error.to_string())
}
