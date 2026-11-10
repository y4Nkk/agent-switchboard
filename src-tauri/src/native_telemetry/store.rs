use super::{MeasurementScope, RequestUsageRow};
use asb_core::contracts::AppKind;
use rusqlite::{params, Connection, OpenFlags};
use std::{path::Path, time::Duration};

fn path(root: &Path) -> std::path::PathBuf { root.join("state/request-performance.sqlite3") }

fn configure(connection: &Connection) -> Result<(), String> {
    connection.busy_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())
}

fn current(connection: &Connection) -> Result<(), String> {
    let version: u32 = connection.pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let owner: u32 = connection.pragma_query_value(None, "application_id", |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if version != 1 || owner != 0x41534250 {
        return Err(format!("原生性能库标识或版本 {version} 不受支持，已保留原文件"));
    }
    Ok(())
}

pub(super) fn initialize(root: &Path) -> Result<(), String> {
    let file = path(root);
    std::fs::create_dir_all(file.parent().ok_or("性能库目录无效")?).map_err(|e| e.to_string())?;
    let connection = Connection::open(file).map_err(|e| e.to_string())?;
    configure(&connection)?;
    let version: u32 = connection.pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let owner: u32 = connection.pragma_query_value(None, "application_id", |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let objects: u64 = connection.query_row(
        "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'", [], |r| r.get(0),
    ).map_err(|e| e.to_string())?;
    if version != 0 || objects != 0 || owner != 0 { return current(&connection); }
    connection.execute_batch("PRAGMA journal_mode=WAL;
        BEGIN IMMEDIATE;
        CREATE TABLE observations (
            id TEXT PRIMARY KEY NOT NULL,
            app TEXT NOT NULL CHECK (app IN ('codex', 'claude')),
            scope TEXT NOT NULL CHECK (scope IN ('generation', 'transport')),
            at_ms INTEGER NOT NULL CHECK (at_ms > 0),
            body TEXT NOT NULL
        );
        CREATE INDEX observations_time ON observations (scope, at_ms DESC, id DESC);
        CREATE INDEX observations_client_time ON observations (app, scope, at_ms DESC, id DESC);
        PRAGMA application_id=0x41534250;
        PRAGMA user_version=1;
        COMMIT;").map_err(|e| e.to_string())
}

pub(super) fn append(root: &Path, records: &[RequestUsageRow]) -> Result<usize, String> {
    let mut connection = Connection::open_with_flags(path(root), OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| e.to_string())?;
    configure(&connection)?;
    current(&connection)?;
    let transaction = connection.transaction().map_err(|e| e.to_string())?;
    let mut inserted = 0;
    for row in records {
        row.validate()?;
        let body = serde_json::to_string(row).map_err(|e| e.to_string())?;
        let at = i64::try_from(row.at_ms).map_err(|_| "原生性能记录时间超出范围")?;
        let count = transaction.execute(
            "INSERT INTO observations (id, app, scope, at_ms, body) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO NOTHING", params![row.id, app_key(row.app), row.scope.key(), at, body],
        ).map_err(|e| e.to_string())?;
        if count == 0 {
            let previous: String = transaction.query_row("SELECT body FROM observations WHERE id=?1",
                [&row.id], |r| r.get(0)).map_err(|e| e.to_string())?;
            if previous != body { return Err(format!("原生性能身份 {} 包含不同内容，已拒绝覆盖", row.id)); }
        }
        inserted += count;
    }
    transaction.commit().map_err(|e| e.to_string())?;
    Ok(inserted)
}

pub(crate) fn visit(
    root: &Path, app: Option<AppKind>, scope: MeasurementScope, start: u64, end: u64,
    mut consume: impl FnMut(RequestUsageRow),
) -> Result<(), String> {
    let connection = Connection::open_with_flags(path(root), OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    configure(&connection)?;
    current(&connection)?;
    let mut statement = connection.prepare(
        "SELECT id, app, scope, at_ms, body FROM observations WHERE scope=?1 AND at_ms BETWEEN ?2 AND ?3
         AND (?4 IS NULL OR app=?4) ORDER BY at_ms DESC, id DESC",
    ).map_err(|e| e.to_string())?;
    let mut cursor = statement.query(params![scope.key(), start, end, app.map(app_key)])
        .map_err(|e| e.to_string())?;
    while let Some(record) = cursor.next().map_err(|e| e.to_string())? {
        let body: String = record.get(4).map_err(|e| e.to_string())?;
        let row: RequestUsageRow = serde_json::from_str(&body).map_err(|_| "原生性能库包含无效记录，已保留原件")?;
        row.validate()?;
        let id: String = record.get(0).map_err(|e| e.to_string())?;
        let app: String = record.get(1).map_err(|e| e.to_string())?;
        let scope: String = record.get(2).map_err(|e| e.to_string())?;
        let at: u64 = record.get(3).map_err(|e| e.to_string())?;
        if row.id != id || app_key(row.app) != app || row.scope.key() != scope || row.at_ms != at {
            return Err("原生性能库记录与索引不一致，已保留原件".into());
        }
        consume(row);
    }
    Ok(())
}

fn app_key(app: AppKind) -> &'static str {
    match app { AppKind::Codex => "codex", AppKind::Claude => "claude" }
}
