use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::{collections::HashSet, path::{Path, PathBuf}, time::Duration};

const SCHEMA: &str = "CREATE TABLE archived_sessions(session_id TEXT PRIMARY KEY NOT NULL) STRICT";
static SCHEMA_GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn path(root: &Path) -> PathBuf { super::super::layout::directory(root).join("archive.sqlite3") }
fn error(error: impl std::fmt::Display) -> String { format!("会话归档库操作失败：{error}") }

fn open(root: &Path, write: bool) -> Result<Connection, String> {
    let _guard = SCHEMA_GATE.lock().map_err(|_| "会话归档库初始化锁不可用")?;
    let path = path(root);
    let existed = path.try_exists().map_err(error)?;
    if write { std::fs::create_dir_all(path.parent().ok_or("归档目录无效")?).map_err(error)?; }
    let flags = if write { OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE } else { OpenFlags::SQLITE_OPEN_READ_ONLY };
    let connection = Connection::open_with_flags(&path, flags).map_err(error)?;
    connection.busy_timeout(Duration::from_secs(5)).map_err(error)?;
    if !existed && write {
        connection.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA}; PRAGMA application_id=1095975491; PRAGMA user_version=1; COMMIT;")).map_err(error)?;
    }
    let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0)).map_err(error)?;
    let identity: u32 = connection.pragma_query_value(None, "application_id", |row| row.get(0)).map_err(error)?;
    let tables: u32 = connection.query_row("SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'", [], |row| row.get(0)).map_err(error)?;
    let schema: Option<String> = connection.query_row("SELECT sql FROM sqlite_master WHERE name='archived_sessions'", [], |row| row.get(0)).optional().map_err(error)?;
    if version != 1 || identity != 1095975491 || tables != 1 || schema.as_deref() != Some(SCHEMA) {
        return Err("会话归档库结构无效；已保留原库，未重置数据".into());
    }
    Ok(connection)
}

pub(super) fn load(root: &Path) -> Result<HashSet<String>, String> {
    if !path(root).try_exists().map_err(error)? { return Ok(HashSet::new()); }
    let connection = open(root, false)?;
    let mut statement = connection.prepare("SELECT session_id FROM archived_sessions ORDER BY session_id").map_err(error)?;
    let ids = statement.query_map([], |row| row.get::<_, String>(0)).map_err(error)?
        .collect::<Result<HashSet<_>, _>>().map_err(error)?;
    if ids.iter().any(|id| !super::super::parser::valid_session_id(id)) { return Err("会话归档身份无效".into()); }
    Ok(ids)
}

pub(super) fn set(root: &Path, id: &str, archived: bool) -> Result<(), String> {
    if !super::super::parser::valid_session_id(id) { return Err("会话归档身份无效".into()); }
    if !archived { return remove(root, &[id.to_string()]); }
    open(root, true)?.execute("INSERT OR IGNORE INTO archived_sessions(session_id) VALUES(?1)", [id]).map_err(error)?;
    Ok(())
}

pub(super) fn remove(root: &Path, ids: &[String]) -> Result<(), String> {
    if !path(root).try_exists().map_err(error)? { return Ok(()); }
    let mut connection = open(root, true)?;
    let transaction = connection.transaction().map_err(error)?;
    for id in ids { transaction.execute("DELETE FROM archived_sessions WHERE session_id=?1", [id]).map_err(error)?; }
    transaction.commit().map_err(error)
}
