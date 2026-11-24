use super::{connect, index_path, schema, DATABASE_NAME, SCHEMA_GATE};
use crate::session_manager::sources::is_link;
use rusqlite::{ErrorCode, OpenFlags};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub(crate) fn storage_bytes(root: &Path) -> Result<u64, String> {
    let (database, sidecars) = paths(root)?;
    std::iter::once(&database).chain(&sidecars).try_fold(0_u64, |total, path| {
        Ok(total.saturating_add(file_bytes(path)?.unwrap_or(0)))
    })
}

pub(crate) fn clear(root: &Path) -> Result<u64, String> {
    // All normal connections enter WAL under this gate. Leaving WAL also
    // requires SQLite's exclusive lock, including against idle connections.
    let _guard = SCHEMA_GATE.lock().map_err(|_| "会话索引锁不可用")?;
    let (database, sidecars) = paths(root)?;
    let extra = sidecars.iter().map(|path| file_bytes(path)).collect::<Result<Vec<_>, _>>()?;
    let Some(bytes) = file_bytes(&database)? else {
        if extra.iter().any(Option::is_some) {
            return Err("会话索引主文件缺失，已保留索引日志文件".into());
        }
        return Ok(0);
    };
    let bytes = extra.into_iter().flatten().fold(bytes, u64::saturating_add);

    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection = connect(&database, flags)?;
    schema::validate(&connection)?;
    let mode: String = connection.pragma_update_and_check(None, "journal_mode", "DELETE", |row| row.get(0))
        .map_err(|error| match error.sqlite_error_code() {
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => {
                format!("会话与用量索引正在使用，请等待当前任务完成后重试：{error}")
            }
            _ => super::error(error),
        })?;
    if !mode.eq_ignore_ascii_case("delete") {
        return Err("会话与用量索引未能退出 WAL 模式，请稍后重试".into());
    }
    connection.close().map_err(|(_, error)| super::error(error))?;

    for path in &sidecars {
        if file_bytes(path)?.is_some() {
            return Err(format!("索引日志文件仍在使用，请稍后重试：{}", path.display()));
        }
    }
    fs::remove_file(&database).map_err(|error| format!("无法清理会话与用量索引：{error}"))?;
    Ok(bytes)
}

fn paths(root: &Path) -> Result<(PathBuf, [PathBuf; 3]), String> {
    let database = index_path(root);
    let directory = database.parent().ok_or("会话索引目录无效")?;
    metadata(root, fs::Metadata::is_dir)?;
    metadata(directory, fs::Metadata::is_dir)?;
    let sidecars = ["-wal", "-shm", "-journal"].map(|suffix| directory.join(format!("{DATABASE_NAME}{suffix}")));
    Ok((database, sidecars))
}

fn file_bytes(path: &Path) -> Result<Option<u64>, String> {
    Ok(metadata(path, fs::Metadata::is_file)?.map(|metadata| metadata.len()))
}

fn metadata(path: &Path, expected: fn(&fs::Metadata) -> bool) -> Result<Option<fs::Metadata>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if expected(&metadata) && !is_link(&metadata) => Ok(Some(metadata)),
        Ok(_) => Err(format!("会话索引路径类型无效：{}", path.display())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("无法检查会话索引路径 {}：{error}", path.display())),
    }
}
