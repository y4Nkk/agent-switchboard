use super::{capture, db_error, store};
use crate::session_manager::parser;
use rusqlite::{params, Connection, TransactionBehavior};
use std::collections::BTreeSet;
use crate::session_manager::read_index::{prefix, FileRevision};
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
pub(super) fn verify_sources(connection: &Connection, ids: &[String]) -> Result<(), String> {
    for main in ids {
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
        verify_subagents(&origins)?;
    }
    Ok(())
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
