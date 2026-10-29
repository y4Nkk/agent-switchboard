use super::{db_error, store, Connection, GATE};
use crate::session_manager::{read_index::{prefix, FileRevision}, SessionMeta};
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::params;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgePlan {
    pub cutoff: String,
    pub sessions: u64,
    pub bytes: u64,
}

pub(crate) fn purge_cutoff(value: &str) -> Result<DateTime<Utc>, String> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "请选择有效的清理日期")?;
    if date.to_string() != value { return Err("清理日期格式无效".into()); }
    if date >= Utc::now().date_naive() { return Err("清理日期必须是今天之前的日期".into()); }
    Ok(date.and_hms_opt(0, 0, 0).ok_or("清理日期无效")?.and_utc())
}

#[derive(Default)]
struct SessionLoad {
    activity: Option<DateTime<Utc>>,
    unknown_activity: bool,
    bytes: u64,
    transcripts: Vec<String>,
    members: BTreeSet<String>,
}

fn stale_sessions(
    connection: &Connection, cutoff: DateTime<Utc>, session: Option<&str>,
) -> Result<BTreeMap<String, SessionLoad>, String> {
    let mut statement = connection.prepare("SELECT id,session_id,parent_id,meta,byte_length
        FROM transcripts WHERE ?1 IS NULL OR session_id=?1 OR parent_id=?1").map_err(db_error)?;
    let mut rows = statement.query([session]).map_err(db_error)?;
    let mut sessions: BTreeMap<String, SessionLoad> = BTreeMap::new();
    while let Some(row) = rows.next().map_err(db_error)? {
        let id: String = row.get(0).map_err(db_error)?;
        let session_id: String = row.get(1).map_err(db_error)?;
        let parent_id: Option<String> = row.get(2).map_err(db_error)?;
        let metadata: String = row.get(3).map_err(db_error)?;
        let meta: SessionMeta = serde_json::from_str(&metadata)
            .map_err(|error| format!("会话活动信息无效，未清理：{error}"))?;
        let activity = meta.last_active_at.as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc));
        let top = parent_id.unwrap_or_else(|| session_id.clone());
        let entry = sessions.entry(top.clone()).or_default();
        entry.unknown_activity |= activity.is_none();
        entry.activity = entry.activity.max(activity);
        entry.bytes = entry.bytes.saturating_add(row.get::<_, i64>(4).map_err(db_error)?.max(0) as u64);
        entry.transcripts.push(id);
        entry.members.extend([top, session_id]);
    }
    // Unknown activity is protected; a capture timestamp is never a substitute.
    sessions.retain(|_, load| !load.unknown_activity && load.activity.is_some_and(|at| at < cutoff));
    Ok(sessions)
}

fn exclusive_asset_bytes(connection: &Connection, sessions: &BTreeMap<String, SessionLoad>) -> Result<u64, String> {
    let retained: BTreeSet<&str> = sessions.values()
        .flat_map(|load| load.transcripts.iter().map(String::as_str)).collect();
    let mut owners: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut statement = connection.prepare("SELECT asset,source FROM attachments").map_err(db_error)?;
    let mut rows = statement.query([]).map_err(db_error)?;
    while let Some(row) = rows.next().map_err(db_error)? {
        owners.entry(row.get(0).map_err(db_error)?).or_default().push(row.get(1).map_err(db_error)?);
    }
    let mut statement = connection.prepare("SELECT id,size FROM assets").map_err(db_error)?;
    let mut rows = statement.query([]).map_err(db_error)?;
    let mut bytes = 0_u64;
    while let Some(row) = rows.next().map_err(db_error)? {
        let id: String = row.get(0).map_err(db_error)?;
        let reclaimable = owners.get(&id)
            .is_none_or(|sources| sources.iter().all(|source| retained.contains(source.as_str())));
        if reclaimable { bytes = bytes.saturating_add(row.get::<_, i64>(1).map_err(db_error)?.max(0) as u64); }
    }
    Ok(bytes)
}

pub(crate) fn preview_purge(root: &Path, cutoff: &str) -> Result<PurgePlan, String> {
    let date = purge_cutoff(cutoff)?;
    let _guard = GATE.lock().map_err(|_| "Claude 会话保留锁不可用")?;
    let connection = store::open(root)?;
    let sessions = stale_sessions(&connection, date, None)?;
    let bytes = sessions.values().map(|load| load.bytes).fold(0_u64, u64::saturating_add)
        .saturating_add(exclusive_asset_bytes(&connection, &sessions)?);
    Ok(PurgePlan { cutoff: cutoff.into(), sessions: sessions.len() as u64, bytes })
}

pub(crate) fn stale_session_ids(root: &Path, cutoff: DateTime<Utc>) -> Result<Vec<String>, String> {
    let _guard = GATE.lock().map_err(|_| "Claude 会话保留锁不可用")?;
    let connection = store::open(root)?;
    Ok(stale_sessions(&connection, cutoff, None)?.into_keys().collect())
}

pub(super) fn validate_deletion(connection: &Connection, ids: &[String], cutoff: DateTime<Utc>) -> Result<(), String> {
    let main = ids.first().ok_or("待清理会话身份缺失")?;
    let selected = stale_sessions(connection, cutoff, Some(main))?;
    if ids.iter().any(|id| !selected.values().any(|load| load.members.contains(id))) {
        return Err("会话最后活动时间已变化或未知，不再符合清理日期，未删除".into());
    }
    let mut statement = connection.prepare("SELECT t.origin,t.byte_length,t.revision FROM transcripts t
        WHERE (t.session_id=?1 OR t.parent_id=?1) AND t.generation=(
            SELECT MAX(newer.generation) FROM transcripts newer WHERE newer.origin=t.origin)").map_err(db_error)?;
    let rows = statement.query_map(params![main], |row| Ok((
        PathBuf::from(row.get::<_, String>(0)?), row.get::<_, u64>(1)?, row.get::<_, String>(2)?,
    ))).map_err(db_error)?;
    let mut origins = BTreeSet::new();
    for row in rows {
        let (path, length, digest) = row.map_err(db_error)?;
        origins.insert(path.clone());
        verify_original(&path, length, &digest)?;
    }
    verify_subagents(&origins)
}

fn verify_original(path: &Path, length: u64, expected: &str) -> Result<(), String> {
    if !path.try_exists().map_err(|error| error.to_string())? { return Ok(()); }
    let before = FileRevision::read(path)?;
    if before.bytes != length { return Err(format!("会话来源已更新，未删除：{}", path.display())); }
    prefix::verify(path, &before, length, expected, &|| Ok(()))?;
    if FileRevision::read(path)? != before {
        return Err(format!("会话来源在清理检查期间变化，未删除：{}", path.display()));
    }
    Ok(())
}

fn verify_subagents(origins: &BTreeSet<PathBuf>) -> Result<(), String> {
    for origin in origins {
        let directory = origin.with_extension("").join("subagents");
        if !directory.try_exists().map_err(|error| error.to_string())? { continue; }
        for entry in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.extension().is_some_and(|extension| extension == "jsonl") && !origins.contains(&path) {
                return Err(format!("会话新增了尚未捕获的子会话，未删除：{}", path.display()));
            }
        }
    }
    Ok(())
}

pub(crate) fn reclaim_space(root: &Path) -> Result<(), String> {
    let _guard = GATE.lock().map_err(|_| "Claude 会话保留锁不可用")?;
    let connection = store::open(root)?;
    connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;").map_err(db_error)
}
