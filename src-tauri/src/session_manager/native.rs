use super::{parser, SessionMeta};
use asb_core::contracts::AppKind;
use chrono::{TimeZone, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(super) struct NativeThread {
    pub meta: SessionMeta,
    pub rollout: PathBuf,
    pub history: PathBuf,
}

pub(super) fn open(path: &Path) -> Result<Connection, String> {
    let connection = Connection::open_with_flags(path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|error| format!("无法读取 Codex 原生历史 {}: {error}", path.display()))?;
    connection.busy_timeout(Duration::from_secs(2)).map_err(|error| error.to_string())?;
    connection.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH, (super::records::MAX_RECORD_BYTES + 65_536) as i32);
    Ok(connection)
}

pub(super) fn threads_matching(root: &Path, session: Option<&str>) -> Result<(Vec<NativeThread>, Vec<String>), String> {
    let database = crate::local_state::codex_paths::sqlite_root(root)?.join("state_5.sqlite");
    if !database.try_exists().map_err(|error| error.to_string())? { return Ok((Vec::new(), Vec::new())); }
    let connection = open(&database)?;
    let mut columns = connection.prepare("PRAGMA table_info(threads)").map_err(|error| error.to_string())?;
    let has_name = columns.query_map([], |row| row.get::<_, String>(1)).map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?.iter().any(|name| name == "name");
    let title_expression = if has_name { "COALESCE(NULLIF(name,''),title)" } else { "title" };
    let predicate = if session.is_some() { "WHERE id=?1" } else { "" };
    let query = format!("SELECT id,rollout_path,COALESCE(created_at_ms,created_at*1000),COALESCE(updated_at_ms,updated_at*1000),{title_expression},cwd,first_user_message,source FROM threads {predicate} ORDER BY created_at,id");
    let mut statement = connection.prepare(
        &query).map_err(|error| error.to_string())?;
    let history = database.with_file_name("thread_history_1.sqlite");
    let rows = statement.query_map(rusqlite::params_from_iter(session), |row| {
        let id: String = row.get(0)?;
        let title: String = row.get(4)?;
        let first: String = row.get::<_, Option<String>>(6)?.unwrap_or_default();
        let source: String = row.get(7)?;
        let parent = serde_json::from_str::<Value>(&source).ok()
            .and_then(|value| parser::parent_from(&serde_json::json!({"source":value})));
        let meta = SessionMeta { app: AppKind::Codex,
            resume_command: Some(super::resume::resume_command(AppKind::Codex, &id)), session_id: id,
            title: parser::clamp_title(&title), summary: parser::clamp_text(&parser::prompt(&first).unwrap_or_else(|| title.clone()), 180),
            project_dir: Some(row.get(5)?), created_at: at(row.get(2)?), last_active_at: at(row.get(3)?),
            alias: None, pinned: false, tags: Vec::new(), parent_session_id: parent, can_delete: false, preservation: None };
        Ok(NativeThread { meta, rollout: PathBuf::from(row.get::<_, String>(1)?), history: history.clone() })
    }).map_err(|error| error.to_string())?;
    let mut threads = Vec::new();
    let mut issues = Vec::new();
    for row in rows {
        match row {
            Ok(thread) => {
                threads.push(thread);
            }
            Err(error) => issues.push(format!("一条原生会话元信息不可读: {error}")),
        }
    }
    Ok((threads, issues))
}

fn at(milliseconds: i64) -> Option<String> {
    Utc.timestamp_millis_opt(milliseconds).single().map(|at| at.to_rfc3339())
}
