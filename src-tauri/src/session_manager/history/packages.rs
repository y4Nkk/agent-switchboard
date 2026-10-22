use super::{capture, db_error, store};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub(super) fn directory(root: &Path, id: &str) -> Result<PathBuf, String> {
    if uuid::Uuid::parse_str(id).is_err() { return Err("恢复文件身份无效".into()); }
    Ok(super::super::layout::directory(root).join("claude-resume").join(id))
}

pub(super) fn registered(connection: &Connection, root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut statement = connection.prepare("SELECT id FROM resume_packages ORDER BY id").map_err(db_error)?;
    let ids = statement.query_map([], |row| row.get::<_, String>(0)).map_err(db_error)?
        .collect::<Result<Vec<_>,_>>().map_err(db_error)?;
    let mut paths = Vec::new();
    for id in ids {
        let path = directory(root, &id)?;
        let temporary = path.with_file_name(format!(".{id}.tmp"));
        remove(root, &temporary)?;
        if path.try_exists().map_err(io_error)? { validate(root, &path)?; paths.push(capture::normalize(&path)?); }
        else {
            connection.execute("UPDATE transcripts SET available=0 WHERE root=?1", [capture::text(&capture::normalize(&path)?)?]).map_err(db_error)?;
            connection.execute("DELETE FROM resume_packages WHERE id=?1", [&id]).map_err(db_error)?;
        }
    }
    Ok(paths)
}

pub(super) fn delete(connection: &Connection, root: &Path, session: &str) -> Result<(), String> {
    let mut statement = connection.prepare("SELECT id FROM resume_packages WHERE session_id=?1").map_err(db_error)?;
    let ids = statement.query_map([session], |row| row.get::<_, String>(0)).map_err(db_error)?
        .collect::<Result<Vec<_>,_>>().map_err(db_error)?;
    for id in ids {
        let path = directory(root, &id)?;
        remove(root, &path.with_file_name(format!(".{id}.tmp")))?;
        remove(root, &path)?;
    }
    connection.execute("DELETE FROM resume_packages WHERE session_id=?1", [session]).map_err(db_error)?;
    Ok(())
}

fn validate(root: &Path, path: &Path) -> Result<(), String> {
    let allowed = super::super::layout::directory(root).join("claude-resume").canonicalize().map_err(io_error)?;
    let metadata = std::fs::symlink_metadata(path).map_err(io_error)?;
    if capture::is_link(&metadata) || !metadata.is_dir() || path.canonicalize().map_err(io_error)?.parent() != Some(allowed.as_path()) {
        return Err("恢复文件目录位置或类型已变化，未继续操作".into());
    }
    Ok(())
}

fn remove(root: &Path, path: &Path) -> Result<(), String> {
    if !path.try_exists().map_err(io_error)? { return Ok(()); }
    validate(root, path)?;
    std::fs::remove_dir_all(path).map_err(io_error)
}

fn io_error(error: impl std::fmt::Display) -> String { store::error(error) }
