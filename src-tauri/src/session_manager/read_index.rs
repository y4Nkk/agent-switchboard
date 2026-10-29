use super::layout;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};
pub(crate) mod prefix;

static SCHEMA_GATE: Mutex<()> = Mutex::new(());
const SCHEMA: &str = "CREATE TABLE entries (
 kind TEXT NOT NULL, cache_key TEXT NOT NULL, revision TEXT NOT NULL, data TEXT NOT NULL,
 PRIMARY KEY(kind,cache_key)
) STRICT";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FileRevision {
    pub bytes: u64,
    pub modified: String,
    pub created: Option<String>,
}

impl FileRevision {
    pub(crate) fn read(path: &Path) -> Result<Self, String> {
        let metadata = fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(format!("来源必须是普通文件：{}", path.display()));
        }
        let timestamp = |value: std::time::SystemTime| value.duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos().to_string()).map_err(|error| error.to_string());
        Ok(Self { bytes: metadata.len(), modified: timestamp(metadata.modified().map_err(|error| error.to_string())?)?,
            created: metadata.created().ok().map(timestamp).transpose()? })
    }

    pub(crate) fn encoded(&self) -> Result<String, String> { key(self) }
}

pub(crate) fn key(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

pub(crate) struct ReadIndex { connection: Connection }

impl ReadIndex {
    pub(crate) fn open(root: &Path) -> Result<Self, String> {
        Self::open_observed(root, &|| Ok(()))
    }

    pub(super) fn open_observed(root: &Path, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<Self, String> {
        checkpoint()?;
        let directory = layout::directory(root);
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        if directory.is_symlink() { return Err("会话读取索引目录不能是符号链接".into()); }
        let path = directory.join("read-index.sqlite3");
        if path.is_symlink() { return Err("会话读取索引不能是符号链接".into()); }
        let _guard = super::read_observer::lock(&SCHEMA_GATE, checkpoint)?;
        let connection = Connection::open(&path).map_err(|error| format!("会话读取索引 {} 不可用：{error}", path.display()))?;
        connection.busy_timeout(Duration::from_secs(5)).map_err(error)?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0)).map_err(error)?;
        if version == 0 {
            let count: i64 = connection.query_row("SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'", [], |row| row.get(0)).map_err(error)?;
            if count != 0 { return Err("会话读取索引模式未知，已保留原件".into()); }
            connection.execute_batch(&format!("BEGIN IMMEDIATE; {SCHEMA}; PRAGMA user_version=2; COMMIT;")).map_err(error)?;
        } else if version != 1 && version != 2 { return Err("会话读取索引版本不受支持，已保留原件".into()); }
        let schema: Option<String> = connection.query_row("SELECT sql FROM sqlite_master WHERE name='entries' AND type='table'", [], |row| row.get(0)).optional().map_err(error)?;
        let objects: i64 = connection.query_row("SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'", [], |row| row.get(0)).map_err(error)?;
        if schema.as_deref() != Some(SCHEMA) || objects != 1 { return Err(format!("会话读取索引模式无效，已保留原件：{}", path.display())); }
        if version == 1 {
            connection.execute_batch("BEGIN IMMEDIATE; DELETE FROM entries; PRAGMA user_version=2; COMMIT;").map_err(error)?;
            log::info!("已重建会话读取派生索引：{}", path.display());
        }
        let mode: String = connection.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0)).map_err(error)?;
        if !mode.eq_ignore_ascii_case("wal") { return Err("会话读取索引未能启用并发读写".into()); }
        Ok(Self { connection })
    }

    pub(crate) fn get<T: DeserializeOwned>(&self, kind: &str, cache_key: &str, revision: &str) -> Result<Option<T>, String> {
        let raw: Option<String> = self.connection.query_row(
            "SELECT data FROM entries WHERE kind=?1 AND cache_key=?2 AND revision=?3",
            params![kind, cache_key, revision], |row| row.get(0)).optional().map_err(error)?;
        self.decode(kind, cache_key, raw)
    }

    pub(crate) fn previous<T: DeserializeOwned>(&self, kind: &str, cache_key: &str) -> Result<Option<T>, String> {
        let raw: Option<String> = self.connection.query_row("SELECT data FROM entries WHERE kind=?1 AND cache_key=?2",
            params![kind, cache_key], |row| row.get(0)).optional().map_err(error)?;
        self.decode(kind, cache_key, raw)
    }

    fn decode<T: DeserializeOwned>(&self, kind: &str, cache_key: &str, raw: Option<String>) -> Result<Option<T>, String> {
        let Some(raw) = raw else { return Ok(None); };
        match serde_json::from_str(&raw) {
            Ok(value) => Ok(Some(value)),
            Err(cause) => {
                log::warn!("重建无效会话派生项 {kind}: {cause}");
                self.connection.execute("DELETE FROM entries WHERE kind=?1 AND cache_key=?2", params![kind, cache_key]).map_err(error)?;
                Ok(None)
            }
        }
    }

    pub(crate) fn put(&self, kind: &str, cache_key: &str, revision: &str, value: &impl Serialize) -> Result<(), String> {
        self.connection.execute("INSERT INTO entries(kind,cache_key,revision,data) VALUES(?1,?2,?3,?4) \
            ON CONFLICT(kind,cache_key) DO UPDATE SET revision=excluded.revision,data=excluded.data",
            params![kind, cache_key, revision, key(value)?]).map_err(error)?;
        Ok(())
    }

    pub(crate) fn file<T: Serialize + DeserializeOwned>(&self, kind: &str, path: &Path,
        mut build: impl FnMut() -> Result<T, String>) -> Result<T, String> {
        let before = FileRevision::read(path)?;
        let result = self.file_snapshot(kind, path, &mut build);
        if result.is_err() && FileRevision::read(path).is_ok_and(|current| current != before) {
            return self.file_snapshot(kind, path, &mut build);
        }
        result
    }

    fn file_snapshot<T: Serialize + DeserializeOwned>(&self, kind: &str, path: &Path,
        build: &mut impl FnMut() -> Result<T, String>) -> Result<T, String> {
        let revision = FileRevision::read(path)?;
        let encoded = revision.encoded()?;
        let cache_key = key(&path)?;
        if let Some(value) = self.get(kind, &cache_key, &encoded)? { return Ok(value); }
        let value = build()?;
        if FileRevision::read(path)? != revision { return Err(format!("来源在读取期间发生变化，请重试：{}", path.display())); }
        self.put(kind, &cache_key, &encoded, &value)?;
        Ok(value)
    }

    pub(crate) fn file_prefix<T: Serialize + DeserializeOwned>(&self, kind: &str, path: &Path, checkpoint: &dyn Fn() -> Result<(), String>,
        mut build: impl FnMut(&mut dyn std::io::BufRead) -> Result<T, String>) -> Result<T, String> {
        let before = FileRevision::read(path)?;
        let result = self.prefix_snapshot(kind, path, checkpoint, &mut build);
        checkpoint()?;
        if result.is_err() && FileRevision::read(path).is_ok_and(|current| current != before) {
            return self.prefix_snapshot(kind, path, checkpoint, &mut build);
        }
        result
    }

    fn prefix_snapshot<T: Serialize + DeserializeOwned>(&self, kind: &str, path: &Path, checkpoint: &dyn Fn() -> Result<(), String>,
        build: &mut impl FnMut(&mut dyn std::io::BufRead) -> Result<T, String>) -> Result<T, String> {
        use sha2::{Digest, Sha256};
        let revision = FileRevision::read(path)?;
        let encoded = revision.encoded()?;
        let cache_key = key(&path)?;
        if let Some(value) = self.get(kind, &cache_key, &encoded)? { return Ok(value); }
        let mut hash = Sha256::new();
        let mut reader = prefix::DigestReader::open(path, 0, revision.bytes, &mut hash)?;
        let value = {
            let decoded = super::record_file::decode(path, &mut reader)?;
            build(&mut std::io::BufReader::new(decoded))?
        };
        let length = reader.bytes;
        drop(reader);
        prefix::verify(path, &revision, length, &format!("{:x}", hash.finalize()), checkpoint)?;
        if super::record_file::compressed(path) && FileRevision::read(path)? != revision {
            return Err(format!("归档在读取期间变化，稍后重新读取：{}", path.display()));
        }
        self.put(kind, &cache_key, &encoded, &value)?;
        Ok(value)
    }
}

fn error(error: rusqlite::Error) -> String { format!("会话读取索引不可用：{error}") }

#[cfg(test)]
mod tests;
