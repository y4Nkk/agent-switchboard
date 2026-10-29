use super::{capture, db_error, store};
use crate::session_manager::parser;
use rusqlite::{Connection, TransactionBehavior};
use std::path::{Path, PathBuf};

pub(super) fn delete(connection: &mut Connection, root: &Path, ids: &[String]) -> Result<(), String> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
    for id in ids {
        let originals = originals(&transaction, id)?;
        for (path, directory, native_id) in originals { remove_original(&path, &directory, &native_id)?; }
        super::packages::delete(&transaction, root, id)?;
    }
    for id in ids {
        transaction.execute("INSERT OR IGNORE INTO deleted_sessions(id) VALUES(?1)", [id]).map_err(db_error)?;
        transaction.execute("DELETE FROM transcripts WHERE session_id=?1 OR parent_id=?1", [id]).map_err(db_error)?;
    }
    store::remove_unused_assets(&transaction)?;
    transaction.commit().map_err(db_error)
}

fn originals(connection: &Connection, id: &str) -> Result<Vec<(PathBuf,PathBuf,String)>, String> {
    let mut statement = connection.prepare("SELECT origin,root,meta FROM transcripts WHERE session_id=?1 OR parent_id=?1 ORDER BY length(origin)")
        .map_err(db_error)?;
    let mut rows = statement.query([id]).map_err(db_error)?;
    let mut originals = Vec::new();
    while let Some(row) = rows.next().map_err(db_error)? {
        let meta: crate::session_manager::SessionMeta = serde_json::from_str(&row.get::<_, String>(2).map_err(db_error)?)
            .map_err(|error| error.to_string())?;
        originals.push((PathBuf::from(row.get::<_, String>(0).map_err(db_error)?),
            PathBuf::from(row.get::<_, String>(1).map_err(db_error)?), super::session_id(&meta).to_string()));
    }
    Ok(originals)
}

fn remove_original(path: &Path, root: &Path, expected: &str) -> Result<(), String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    if capture::is_link(&metadata) || !metadata.is_file() { return Err(format!("会话来源类型已变化，未删除：{}", path.display())); }
    let resolved = path.canonicalize().map_err(|error| error.to_string())?;
    let allowed = root.join("projects").canonicalize().map_err(|error| error.to_string())?;
    if !resolved.starts_with(&allowed) { return Err("会话来源已跨出原目录，未删除".into()); }
    let identity = parser::session_identity(&resolved).map_err(|error| error.to_string())?;
    if identity.is_none_or(|identity| identity.id != expected) { return Err("会话来源身份已变化，未删除".into()); }
    let sidecar = resolved.with_extension("");
    if sidecar.try_exists().map_err(|error| error.to_string())? {
        let metadata = std::fs::symlink_metadata(&sidecar).map_err(|error| error.to_string())?;
        if capture::is_link(&metadata) || !sidecar.canonicalize().map_err(|error| error.to_string())?.starts_with(&allowed) {
            return Err("会话附属目录已变化，未删除".into());
        }
        if metadata.is_dir() { std::fs::remove_dir_all(&sidecar).map_err(|error| error.to_string())?; }
    }
    std::fs::remove_file(&resolved).map_err(|error| format!("无法删除会话 {}：{error}", capture::text(&resolved).unwrap_or("<非 UTF-8 路径>")))
}
