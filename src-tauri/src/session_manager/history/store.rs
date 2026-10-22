use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(super) const SCHEMA: &str = "
CREATE TABLE roots(path TEXT PRIMARY KEY NOT NULL, automatic INTEGER NOT NULL DEFAULT 0 CHECK(automatic IN (0,1)), issue TEXT) STRICT;
CREATE TABLE deleted_sessions(id TEXT PRIMARY KEY NOT NULL) STRICT;
CREATE TABLE resume_packages(id TEXT PRIMARY KEY NOT NULL, session_id TEXT NOT NULL) STRICT;
CREATE TABLE transcripts(
    id TEXT PRIMARY KEY NOT NULL, origin TEXT NOT NULL, root TEXT NOT NULL,
    generation INTEGER NOT NULL CHECK(generation>=0),
    session_id TEXT NOT NULL, parent_id TEXT, meta TEXT NOT NULL CHECK(json_valid(meta)),
    revision TEXT NOT NULL, byte_length INTEGER NOT NULL CHECK(byte_length>=0),
    stamp TEXT NOT NULL, asset_stamp TEXT NOT NULL, saved_at TEXT NOT NULL,
    available INTEGER NOT NULL CHECK(available IN (0,1)), issue TEXT, capture_issue TEXT,
    UNIQUE(origin,generation)
) STRICT;
CREATE TABLE records(
    source TEXT NOT NULL REFERENCES transcripts(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK(ordinal>0), position INTEGER NOT NULL CHECK(position>=0),
    raw BLOB NOT NULL CHECK(length(raw)<=33554432),
    PRIMARY KEY(source,position), UNIQUE(source,ordinal)
) STRICT;
CREATE TABLE assets(id TEXT PRIMARY KEY NOT NULL, size INTEGER NOT NULL CHECK(size>=0)) STRICT;
CREATE TABLE asset_chunks(
    asset TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL, data BLOB NOT NULL CHECK(length(data)<=1048576),
    PRIMARY KEY(asset,ordinal)
) STRICT;
CREATE TABLE attachments(
    source TEXT NOT NULL REFERENCES transcripts(id) ON DELETE CASCADE,
    reference TEXT NOT NULL, relative_path TEXT NOT NULL, stamp TEXT NOT NULL,
    asset TEXT NOT NULL REFERENCES assets(id), PRIMARY KEY(source,reference)
) STRICT;
CREATE TABLE image_references(
    source TEXT NOT NULL REFERENCES transcripts(id) ON DELETE CASCADE,
    reference TEXT NOT NULL, relative_path TEXT NOT NULL, PRIMARY KEY(source,reference)
) STRICT;
CREATE INDEX transcript_sessions ON transcripts(session_id,parent_id);
";

pub(super) fn path(root: &Path) -> PathBuf {
    super::super::layout::directory(root).join("claude.sqlite3")
}

pub(super) fn error(error: impl std::fmt::Display) -> String {
    format!("Claude 会话保留库操作失败：{error}")
}

fn configure(connection: &Connection) -> Result<(), String> {
    connection.busy_timeout(Duration::from_secs(10)).map_err(error)?;
    connection.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH, 33_619_968);
    connection.execute_batch("PRAGMA foreign_keys=ON").map_err(error)
}

pub(super) fn open(root: &Path) -> Result<Connection, String> {
    let path = path(root);
    let existed = path.try_exists().map_err(error)?;
    std::fs::create_dir_all(path.parent().ok_or("Claude 会话保留目录无效")?).map_err(error)?;
    validate_path(&path)?;
    let mut connection = Connection::open(&path).map_err(error)?;
    configure(&connection)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(error)?;
    let version: u32 = transaction.pragma_query_value(None, "user_version", |row| row.get(0)).map_err(error)?;
    let tables: u32 = transaction.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'", [], |row| row.get(0),
    ).map_err(error)?;
    if !existed && version == 0 && tables == 0 {
        transaction.execute_batch(SCHEMA).map_err(error)?;
        transaction.pragma_update(None, "application_id", 0x41534248).map_err(error)?;
        transaction.pragma_update(None, "user_version", 1).map_err(error)?;
    } else {
        validate(&transaction)?;
    }
    transaction.commit().map_err(error)?;
    let mode: String = connection.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0)).map_err(error)?;
    if !mode.eq_ignore_ascii_case("wal") { return Err("Claude 会话保留库无法启用 WAL，未继续保存".into()); }
    connection.execute_batch("PRAGMA synchronous=FULL").map_err(error)?;
    Ok(connection)
}

pub(super) fn read(path: &Path) -> Result<Connection, String> {
    validate_path(path)?;
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
    configure(&connection)?;
    validate(&connection)?;
    Ok(connection)
}

fn validate_path(path: &Path) -> Result<(), String> {
    for target in [path.parent().ok_or("Claude 会话保留目录无效")?, path] {
        match std::fs::symlink_metadata(target) {
            Ok(metadata) if super::capture::is_link(&metadata) => return Err("Claude 会话保留库及其目录不能是链接".into()),
            Err(issue) if issue.kind() == std::io::ErrorKind::NotFound && target == path => {}
            Err(issue) => return Err(error(issue)),
            _ => {}
        }
    }
    Ok(())
}

fn validate(connection: &Connection) -> Result<(), String> {
    let version: u32 = connection.pragma_query_value(None, "user_version", |row| row.get(0)).map_err(error)?;
    let identity: u32 = connection.pragma_query_value(None, "application_id", |row| row.get(0)).map_err(error)?;
    let schema: Option<String> = connection.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='records'", [], |row| row.get(0),
    ).optional().map_err(error)?;
    if version != 1 || identity != 0x41534248 || schema.is_none() {
        return Err("Claude 会话保留库结构无效；已保留原库，未重置数据".into());
    }
    let expected: std::collections::BTreeSet<_> = SCHEMA.split(';').map(str::trim)
        .filter(|entry| entry.starts_with("CREATE TABLE ")).map(str::to_string).collect();
    let mut tables = connection.prepare("SELECT sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'").map_err(error)?;
    let actual = tables.query_map([], |row| row.get::<_, String>(0)).map_err(error)?
        .collect::<Result<std::collections::BTreeSet<_>,_>>().map_err(error)?;
    if actual != expected { return Err("Claude 会话保留库表结构与当前契约不一致；原库已保留".into()); }
    connection.prepare("SELECT t.id,t.origin,t.root,t.session_id,t.parent_id,t.meta,t.revision,
        t.byte_length,t.stamp,t.asset_stamp,t.saved_at,t.available,t.issue,r.position,r.ordinal,r.raw,
        a.relative_path,a.reference,c.data,d.id FROM transcripts t
        LEFT JOIN records r ON r.source=t.id LEFT JOIN attachments a ON a.source=t.id
        LEFT JOIN asset_chunks c ON c.asset=a.asset LEFT JOIN deleted_sessions d ON d.id=t.session_id LIMIT 0")
        .map_err(error)?;
    Ok(())
}

pub(super) fn remove_unused_assets(connection: &Connection) -> Result<(), String> {
    connection.execute("DELETE FROM assets WHERE id NOT IN (SELECT asset FROM attachments)", []).map_err(error)?;
    Ok(())
}
