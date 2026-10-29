use super::CodexRequestRecord;
use rusqlite::{params, Connection};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

static SCHEMA_GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());
const VERSION: i64 = 3;
const SCHEMA: &str = "
CREATE TABLE codex_requests (
 id TEXT PRIMARY KEY NOT NULL, at_ms INTEGER NOT NULL CHECK(at_ms >= 0),
 origin TEXT NOT NULL DEFAULT 'proxy' CHECK(origin IN ('proxy','session')), thread_id TEXT,
 profile_id TEXT, model TEXT, status INTEGER CHECK(status BETWEEN 100 AND 599),
 input_tokens INTEGER CHECK(input_tokens >= 0), output_tokens INTEGER CHECK(output_tokens >= 0),
 cache_read_tokens INTEGER CHECK(cache_read_tokens >= 0),
 cache_creation_tokens INTEGER CHECK(cache_creation_tokens >= 0),
 reasoning_tokens INTEGER CHECK(reasoning_tokens >= 0), cost_micros INTEGER CHECK(cost_micros >= 0),
 payload TEXT NOT NULL CHECK(json_valid(payload))
);
CREATE INDEX codex_requests_time ON codex_requests(at_ms DESC, id);
CREATE INDEX codex_requests_provider_time ON codex_requests(profile_id, at_ms DESC);
CREATE INDEX codex_requests_model_time ON codex_requests(model, at_ms DESC);
PRAGMA user_version = 3;";

/// Adds the current origin fields without retaining the abandoned replay cursor.
/// Existing payloads are rewritten once so the strict record contract keeps
/// rejecting rows that predate the origin discriminator.
fn migrate_v1_to_current(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "ALTER TABLE codex_requests ADD COLUMN origin TEXT NOT NULL DEFAULT 'proxy';
             ALTER TABLE codex_requests ADD COLUMN thread_id TEXT;
             ",
        )
        .map_err(db_error)?;
    let rows: Vec<(String, String)> = {
        let mut statement = connection
            .prepare("SELECT id,payload FROM codex_requests")
            .map_err(db_error)?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(db_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_error)?
    };
    for (id, payload) in rows {
        let mut value: serde_json::Value = serde_json::from_str(&payload)
            .map_err(|_| "Codex 账本 v1 载荷无效，未迁移".to_string())?;
        let Some(object) = value.as_object_mut() else {
            return Err("Codex 账本 v1 载荷无效，未迁移".into());
        };
        if object.contains_key("origin") {
            return Err("Codex 账本 v1 载荷已含来源字段，未迁移".into());
        }
        object.insert("origin".into(), "proxy".into());
        connection
            .execute(
                "UPDATE codex_requests SET payload=?1 WHERE id=?2",
                params![value.to_string(), id],
            )
            .map_err(db_error)?;
    }
    connection
        .pragma_update(None, "user_version", VERSION)
        .map_err(db_error)
}

#[derive(Clone)]
pub(crate) struct CodexRequestLedger {
    pub(super) path: PathBuf,
}
impl CodexRequestLedger {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            path: root.join("codex/requests.sqlite3"),
        }
    }
    fn open_write(&self) -> Result<Connection, String> {
        let _guard = SCHEMA_GATE.lock().map_err(|_| "Codex 账本初始化锁不可用")?;
        std::fs::create_dir_all(self.path.parent().ok_or("Codex 账本路径无效")?)
            .map_err(|error| format!("不能创建 Codex 账本目录：{error}"))?;
        let mut connection = Connection::open(&self.path).map_err(db_error)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(db_error)?;
        initialize(&mut connection)?;
        Ok(connection)
    }
    pub(super) fn open_read(&self) -> Result<Option<Connection>, String> {
        let _guard = SCHEMA_GATE.lock().map_err(|_| "Codex 账本初始化锁不可用")?;
        if !self.path.try_exists().map_err(|error| error.to_string())? {
            return Ok(None);
        }
        // Opened read-write so an existing ledger migrates on the first read;
        // the transaction gate keeps concurrent readers serialized.
        let mut connection = Connection::open(&self.path).map_err(db_error)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(db_error)?;
        initialize(&mut connection)?;
        Ok(Some(connection))
    }
    pub(crate) fn append(&self, record: &CodexRequestRecord) -> Result<(), String> {
        record.validate()?;
        let payload = serde_json::to_string(record).map_err(|error| error.to_string())?;
        let mut connection = self.open_write()?;
        let transaction = connection.transaction().map_err(db_error)?;
        let changed = transaction
            .execute(
                "INSERT INTO codex_requests
            (id, at_ms, origin, thread_id, profile_id, model, status, input_tokens, output_tokens,
             cache_read_tokens, cache_creation_tokens, reasoning_tokens, cost_micros, payload)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14) ON CONFLICT(id) DO NOTHING",
                params![
                    record.id,
                    record.at_ms,
                    record.origin.as_str(),
                    record.thread_id,
                    record.profile_id,
                    record.mapped_model,
                    record.status,
                    record.input_tokens,
                    record.output_tokens,
                    record.cache_read_tokens,
                    record.cache_creation_tokens,
                    record.reasoning_tokens,
                    record.cost_micros()?,
                    payload
                ],
            )
            .map_err(db_error)?;
        if changed == 0 {
            let existing: String = transaction
                .query_row(
                    "SELECT payload FROM codex_requests WHERE id=?1",
                    [&record.id],
                    |row| row.get(0),
                )
                .map_err(db_error)?;
            if existing != payload {
                return Err("Codex 请求标识已存在且内容不同，未覆盖原记录".into());
            }
        }
        transaction.commit().map_err(db_error)
    }
}
fn initialize(connection: &mut Connection) -> Result<(), String> {
    let transaction = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(db_error)?;
    let version: i64 = transaction
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(db_error)?;
    if version == 0 {
        let tables: i64 = transaction.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [], |row| row.get(0)).map_err(db_error)?;
        if tables != 0 {
            return Err("Codex 账本 schema 未知，未重建或覆盖数据库".into());
        }
        transaction.execute_batch(SCHEMA).map_err(db_error)?;
    } else if version == 1 {
        migrate_v1_to_current(&transaction)?;
    } else if version == 2 {
        transaction.execute_batch("DROP TABLE codex_session_sync; PRAGMA user_version=3;").map_err(db_error)?;
    } else if version != VERSION {
        return Err(format!("Codex 账本版本 {version} 不受支持，原数据库未更改"));
    }
    transaction.commit().map_err(db_error)
}
pub(super) fn db_error(error: rusqlite::Error) -> String {
    format!("Codex 请求账本数据库错误：{error}")
}

#[cfg(test)]
mod tests;
