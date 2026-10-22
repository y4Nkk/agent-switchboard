use super::{attachments, db_error, deleted, issues, reading, Connection};
use crate::session_manager::{conversion, images, parser, records, sources, SessionIssue};
use asb_core::AppKind;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{File, Metadata};
use std::io::{BufRead, BufReader, Read};
use std::path::{Component, Path, PathBuf};
pub(super) use crate::session_manager::sources::is_link;

pub(super) fn remember_root(connection: &Connection, directory: &Path, root: &Path, automatic: bool) -> Result<(), String> {
    if !directory.is_absolute() || directory.components().any(|part| matches!(part, Component::ParentDir)) {
        return Err("Claude 会话来源必须是无上级跳转的绝对目录".into());
    }
    let directory = normalize(directory)?;
    let state = normalize(root)?;
    if directory.starts_with(&state) || state.starts_with(&directory) {
        return Err("Claude 会话来源不能包含应用状态目录".into());
    }
    connection.execute("INSERT INTO roots(path,automatic) VALUES(?1,?2)
        ON CONFLICT(path) DO UPDATE SET automatic=max(automatic,excluded.automatic)", params![text(&directory)?,automatic]).map_err(db_error)?;
    Ok(())
}

pub(super) fn synchronize(connection: &mut Connection, root: &Path, initial: &[PathBuf]) -> Result<Vec<SessionIssue>, String> {
    let mut messages = Vec::new();
    let mut workflows = Vec::new();
    connection.execute("UPDATE roots SET automatic=0", []).map_err(db_error)?;
    for directory in initial {
        if let Err(error) = remember_root(connection, directory, root, true) { messages.push(error); }
    }
    let mut directories: Vec<_> = reading::roots(connection)?.into_iter().map(|directory| PathBuf::from(directory.path)).collect();
    directories.extend(super::packages::registered(connection, root)?);
    for directory in directories {
        let directory = text(&directory)?;
        let start = messages.len();
        connection.execute("UPDATE transcripts SET available=0 WHERE root=?1", [directory]).map_err(db_error)?;
        let collected = sources::collect(&Path::new(directory).join("projects"));
        messages.extend(collected.issues);
        for path in collected.paths {
            if conversion::is_workflow(&path) {
                workflows.push(conversion::pending_issue(&path).unwrap_or_else(|message| SessionIssue::new(AppKind::Claude, message)));
                continue;
            }
            match capture(connection, directory, &path) {
                Ok(warnings) => messages.extend(warnings),
                Err(error) => {
                    let message = format!("{}：{error}", path.display());
                    connection.execute("UPDATE transcripts SET capture_issue=?1,available=1 WHERE origin=?2
                        AND generation=(SELECT max(generation) FROM transcripts WHERE origin=?2)",
                        params![message, text(&path)?]).map_err(db_error)?;
                    messages.push(message);
                }
            }
        }
        let issue = (messages.len() > start).then(|| messages[start..].join("；"));
        connection.execute("UPDATE roots SET issue=?1 WHERE path=?2", params![issue,directory]).map_err(db_error)?;
    }
    let mut result = issues(messages);
    result.extend(workflows);
    Ok(result)
}

pub(super) fn initial_roots() -> Result<Vec<PathBuf>, String> {
    let current = crate::local_state::LocalState::user_config_path(AppKind::Claude)?;
    Ok(vec![crate::local_state::user_home_dir()?.join(".claude"),
        current.parent().ok_or("Claude 会话配置目录无效")?.into()])
}

struct Previous { id: String, generation: u64, session_id: String, revision: String, length: u64,
    stamp: String, asset_stamp: String, issue: Option<String>, retry: bool }

fn previous(connection: &Connection, origin: &str) -> Result<Option<Previous>, String> {
    connection.query_row("SELECT id,generation,session_id,revision,byte_length,stamp,asset_stamp,issue,capture_issue IS NOT NULL FROM transcripts
        WHERE origin=?1 ORDER BY generation DESC LIMIT 1", [origin],
        |row| Ok(Previous { id: row.get(0)?, generation: row.get(1)?, session_id: row.get(2)?, revision: row.get(3)?,
            length: row.get(4)?, stamp: row.get(5)?, asset_stamp: row.get(6)?, issue: row.get(7)?, retry: row.get(8)? }))
        .optional().map_err(db_error)
}

fn capture(connection: &mut Connection, root: &str, path: &Path) -> Result<Vec<String>, String> {
    let origin = text(path)?;
    let previous = previous(connection, origin)?;
    let id = previous.as_ref().map(|old| old.id.clone()).unwrap_or_else(|| asb_switch::sha256_hex(&format!("{origin}:0")));
    let before = stamp(&path.metadata().map_err(db_error_path)?)?;
    let inventory = attachments::inventory(connection, &id, path)?;
    if previous.as_ref().is_some_and(|old| old.stamp == before && old.asset_stamp == inventory.stamp && !old.retry) {
        connection.execute("UPDATE transcripts SET available=1 WHERE id=?1", [&id]).map_err(db_error)?;
        return Ok(previous.and_then(|old| old.issue).into_iter().collect());
    }
    let (mut meta, warnings) = parser::parse_session(AppKind::Claude, path).map_err(db_error_path)?;
    if !warnings.is_empty() { return Err(warnings.join("；")); }
    let native_id = meta.session_id.clone();
    let relative = path.strip_prefix(Path::new(root).join("projects")).map_err(db_error_path)?;
    if relative.components().count() == 4 && path.parent().and_then(Path::file_name).is_some_and(|name| name == "subagents") {
        meta.parent_session_id = Some(native_id);
        meta.session_id = format!("{}-{}", meta.parent_session_id.as_deref().unwrap_or(""), path.file_stem().and_then(|name| name.to_str()).ok_or("子代理文件名无效")?);
    }
    if !parser::valid_session_id(&meta.session_id) { return Err("Claude 会话身份无效".into()); }
    if deleted(connection, &meta.session_id, meta.parent_session_id.as_deref())? { return Ok(Vec::new()); }
    if previous.as_ref().is_some_and(|old| old.session_id != meta.session_id) {
        return Err("原路径的会话身份已改变；已保留原会话，未覆盖".into());
    }
    let generation = generation(path, previous.as_ref(), inventory.rewritten)?;
    let id = asb_switch::sha256_hex(&format!("{origin}:{generation}"));
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
    let metadata = serde_json::to_string(&meta).map_err(|error| error.to_string())?;
    transaction.execute("INSERT OR IGNORE INTO transcripts
        (id,origin,root,generation,session_id,parent_id,meta,revision,byte_length,stamp,asset_stamp,saved_at,available)
        VALUES(?1,?2,?3,?4,?5,?6,?7,'',0,'','','',1)",
        params![id,origin,root,generation,meta.session_id,meta.parent_session_id,metadata]).map_err(db_error)?;
    if let Some(old) = previous.as_ref().filter(|old| old.id != id) {
        transaction.execute("INSERT INTO attachments SELECT ?1,reference,relative_path,stamp,asset FROM attachments WHERE source=?2", params![id,old.id]).map_err(db_error)?;
        transaction.execute("INSERT INTO image_references SELECT ?1,reference,relative_path FROM image_references WHERE source=?2", params![id,old.id]).map_err(db_error)?;
    }
    let (revision, length, references) = save_records(&transaction, &id, path, previous.as_ref().filter(|old| old.id == id))?;
    transaction.execute("UPDATE transcripts SET meta=?1 WHERE id=?2", params![metadata,id]).map_err(db_error)?;
    let (warnings, asset_stamp) = attachments::save(&transaction, &id, path, &inventory, &references)?;
    if before != stamp(&path.metadata().map_err(db_error_path)?)? || digest(path, None)? != revision {
        return Err("会话在保存期间发生变化；保留上次完整记录，稍后重试".into());
    }
    let inventory = attachments::inventory(&transaction, &id, path)?;
    if inventory.stamp != asset_stamp { return Err("会话附件集合在保存期间变化，稍后重试".into()); }
    let issue = (!warnings.is_empty()).then(|| warnings.join("；"));
    transaction.execute("UPDATE transcripts SET capture_issue=NULL WHERE origin=?1", [origin]).map_err(db_error)?;
    transaction.execute("UPDATE transcripts SET meta=?1,session_id=?2,parent_id=?3,revision=?4,
        byte_length=?5,stamp=?6,asset_stamp=?7,saved_at=?8,available=1,issue=?9 WHERE id=?10",
        params![metadata,meta.session_id,meta.parent_session_id,revision,length,before,inventory.stamp,chrono::Utc::now().to_rfc3339(),issue,id]).map_err(db_error)?;
    super::store::remove_unused_assets(&transaction)?;
    transaction.commit().map_err(db_error)?;
    Ok(warnings)
}

fn generation(path: &Path, previous: Option<&Previous>, rewritten_assets: bool) -> Result<u64, String> {
    let Some(old) = previous else { return Ok(0); };
    if !rewritten_assets && digest(path, Some(old.length))? == old.revision { return Ok(old.generation); }
    old.generation.checked_add(1).ok_or("会话版本超出保存范围".into())
}

fn save_records(connection: &Connection, id: &str, path: &Path, previous: Option<&Previous>) -> Result<(String, u64, BTreeSet<String>), String> {
    let append_at = match previous {
        Some(old) if digest(path, Some(old.length))? == old.revision => old.length,
        Some(_) => return Err("原记录在版本确认后再次改写，已保留旧版本，稍后重试".into()),
        None => 0,
    };
    let mut reader = BufReader::new(File::open(path).map_err(db_error_path)?);
    let mut line = Vec::new();
    let mut hash = Sha256::new();
    let mut position = 0u64;
    let mut ordinal = 0usize;
    let mut references = BTreeSet::new();
    loop {
        line.clear();
        let count = (&mut reader).take(records::MAX_RECORD_BYTES + 1).read_until(b'\n', &mut line).map_err(db_error_path)?;
        if count == 0 { break; }
        if count as u64 > records::MAX_RECORD_BYTES { return Err("单条会话记录超过 32 MiB，未发布部分快照".into()); }
        ordinal += 1;
        if !line.iter().all(u8::is_ascii_whitespace) {
            let value = serde_json::from_slice(&line).map_err(|error| format!("第 {ordinal} 条记录尚未完整或无效：{error}"))?;
            references.extend(images::references(&value).into_iter().filter(|reference| !reference.starts_with("data:")));
        }
        if position >= append_at {
            connection.execute("INSERT INTO records(source,ordinal,position,raw) VALUES(?1,?2,?3,?4)",
                params![id,ordinal,position,line]).map_err(db_error)?;
        } else if position + count as u64 > append_at {
            let changed = connection.execute("UPDATE records SET raw=?1 WHERE source=?2 AND position=?3", params![line,id,position]).map_err(db_error)?;
            if changed != 1 { return Err("已保存末条记录的位置不一致，未继续追加".into()); }
        }
        hash.update(&line);
        position += count as u64;
    }
    if position < append_at { return Err("会话在保存期间被截断，未替换已保留记录".into()); }
    Ok((format!("{:x}", hash.finalize()), position, references))
}

pub(super) fn digest(path: &Path, limit: Option<u64>) -> Result<String, String> {
    let mut file = File::open(path).map_err(db_error_path)?.take(limit.unwrap_or(u64::MAX));
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65_536];
    loop {
        let count = file.read(&mut buffer).map_err(db_error_path)?;
        if count == 0 { break; }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub(super) fn stamp(meta: &Metadata) -> Result<String, String> {
    let time = meta.modified().map_err(db_error_path)?.duration_since(std::time::UNIX_EPOCH).map_err(db_error_path)?;
    Ok(format!("{}:{}:{}", meta.len(), time.as_secs(), time.subsec_nanos()))
}

pub(super) fn text(path: &Path) -> Result<&str, String> { path.to_str().ok_or("Claude 来源路径不能无损保存为 UTF-8".into()) }
fn db_error_path(error: impl std::fmt::Display) -> String { error.to_string() }

pub(super) fn normalize(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() { return Err("Claude 来源必须是绝对目录".into()); }
    let path = if path.try_exists().map_err(db_error_path)? { path.canonicalize().map_err(db_error_path)? } else { path.components().collect() };
    #[cfg(windows)]
    {
        let value = text(&path)?;
        if let Some(value) = value.strip_prefix(r"\\?\UNC\") { return Ok(PathBuf::from(format!(r"\\{value}"))); }
        if let Some(value) = value.strip_prefix(r"\\?\") { return Ok(PathBuf::from(value)); }
    }
    Ok(path)
}
