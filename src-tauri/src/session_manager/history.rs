mod store;
mod capture;
mod attachments;
mod reading;
mod resume;
mod worker;
mod deletion;
mod packages;
pub(crate) use worker::{spawn, flush_on_exit};

use super::{SessionIssue, SessionMeta, SessionSource};
use asb_core::AppKind;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static GATE: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Transcript {
    pub database: PathBuf,
    pub id: String,
    pub origin: PathBuf,
    pub revision: String,
    pub issue: Option<String>,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preservation {
    pub saved_at: String,
    pub source_available: bool,
    pub needs_attention: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRoot {
    pub path: String,
    pub available: bool,
    pub removable: bool,
    pub issue: Option<String>,
}

pub(crate) fn initialize(root: &Path) -> Result<(), String> {
    let _guard = GATE.lock().map_err(|_| "Claude 会话保留锁不可用")?;
    store::open(root).map(|_| ())
}

pub(super) fn scan(root: &Path) -> Result<(Vec<SessionSource>, Vec<SessionIssue>), String> {
    let _guard = GATE.lock().map_err(|_| "Claude 会话保留锁不可用")?;
    let mut connection = store::open(root)?;
    let issues = match capture::initial_roots().and_then(|initial| capture::synchronize(&mut connection, root, &initial)) {
        Ok(issues) => issues,
        Err(error) => issues(vec![error]),
    };
    Ok((reading::sources(&connection, &store::path(root))?, issues))
}

pub(crate) fn preserve_before_switch(root: &Path) -> Result<Vec<asb_core::contracts::LocalizedMessage>, String> {
    let _guard = GATE.lock().map_err(|_| "Claude 会话保留锁不可用")?;
    let mut connection = store::open(root)?;
    let issues = capture::synchronize(&mut connection, root, &capture::initial_roots()?)?;
    Ok(issues.into_iter().filter(|issue| issue.conversion_id.is_none()).map(|issue| asb_core::contracts::LocalizedMessage::new(
        "sessions.preservation.issue", serde_json::json!({"detail":issue.message}),
        format!("部分 Claude 会话尚未完整保留：{}", issue.message),
    )).collect())
}

pub(crate) fn roots(root: &Path) -> Result<Vec<HistoryRoot>, String> {
    let _guard = GATE.lock().map_err(|_| "Claude 会话保留锁不可用")?;
    let connection = store::open(root)?;
    reading::roots(&connection)
}

pub(super) fn workflow_roots(root: &Path) -> Result<Vec<PathBuf>, String> {
    let _guard = GATE.lock().map_err(|_| "Claude 会话保留锁不可用")?;
    let connection = store::open(root)?;
    let mut roots: Vec<_> = reading::roots(&connection)?.into_iter().map(|source| PathBuf::from(source.path)).collect();
    roots.extend(packages::registered(&connection, root)?);
    Ok(roots.into_iter().map(|path| path.join("projects")).collect())
}

pub(crate) fn forget_root(root: &Path, path: &str) -> Result<Vec<HistoryRoot>, String> {
    let _guard = GATE.lock().map_err(|_| "Claude 会话保留锁不可用")?;
    let mut connection = store::open(root)?;
    let roots = reading::roots(&connection)?;
    let selected = roots.iter().find(|source| source.path == path).ok_or("会话来源已经变化，请刷新后重试")?;
    if !selected.removable { return Err("当前和默认 Claude 来源始终接入，不能停用".into()); }
    let transaction = connection.transaction().map_err(db_error)?;
    transaction.execute("DELETE FROM roots WHERE path=?1", [path]).map_err(db_error)?;
    transaction.execute("UPDATE transcripts SET available=0 WHERE root=?1", [path]).map_err(db_error)?;
    transaction.commit().map_err(db_error)?;
    reading::roots(&connection)
}

pub(crate) fn revision(root: &Path) -> Result<String, String> {
    use sha2::Digest;
    let connection = store::read(&store::path(root))?;
    let mut statement = connection.prepare("SELECT id,revision FROM transcripts ORDER BY id").map_err(db_error)?;
    let mut rows = statement.query([]).map_err(db_error)?;
    let mut hash = sha2::Sha256::new();
    while let Some(row) = rows.next().map_err(db_error)? {
        hash.update(row.get::<_, String>(0).map_err(db_error)?.as_bytes());
        hash.update(row.get::<_, String>(1).map_err(db_error)?.as_bytes());
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub(crate) fn add_root(root: &Path, directory: &Path) -> Result<Vec<HistoryRoot>, String> {
    let directory = directory.canonicalize().map_err(|error| format!("无法接入 Claude 会话目录：{error}"))?;
    if !directory.join("projects").is_dir() { return Err("请选择包含 projects 子目录的 Claude 配置目录".into()); }
    let _guard = GATE.lock().map_err(|_| "Claude 会话保留锁不可用")?;
    let mut connection = store::open(root)?;
    capture::remember_root(&connection, &directory, root, false)?;
    capture::synchronize(&mut connection, root, &capture::initial_roots()?)?;
    reading::roots(&connection)
}

pub(super) fn delete(root: &Path, ids: &[String]) -> Result<(), String> {
    let _guard = GATE.lock().map_err(|_| "Claude 会话保留锁不可用")?;
    deletion::delete(root, ids)
}

pub(super) fn prepare_resume(root: &Path, source: &SessionSource) -> Result<PathBuf, String> {
    let _guard = GATE.lock().map_err(|_| "Claude 会话保留锁不可用")?;
    resume::prepare(root, source)
}

pub(super) fn resolve_images(sources: &[Transcript], references: &[String]) -> (Vec<String>, Vec<String>) {
    attachments::resolve_images(sources, references)
}

pub(super) fn bind_images(source: &Transcript, references: Vec<String>) -> Vec<String> {
    attachments::bind_images(source, references)
}

pub(crate) fn visit_records(
    source: &Transcript, offset: u64,
    visitor: impl FnMut(usize, u64, serde_json::Value) -> Result<std::ops::ControlFlow<()>, String>,
) -> Result<Vec<String>, String> { reading::visit(source, offset, visitor) }

pub(crate) fn visit_record_bytes(
    source: &Transcript, offset: u64,
    visitor: impl FnMut(usize, u64, &[u8]) -> Result<std::ops::ControlFlow<()>, String>,
) -> Result<Vec<String>, String> { reading::visit_bytes(source, offset, visitor) }

fn session_id(meta: &SessionMeta) -> &str {
    meta.parent_session_id.as_deref().unwrap_or(&meta.session_id)
}

fn db_error(error: rusqlite::Error) -> String { store::error(error) }

fn issues(messages: Vec<String>) -> Vec<SessionIssue> {
    messages.into_iter().map(|message| SessionIssue::new(AppKind::Claude, message)).collect()
}

fn deleted(connection: &Connection, id: &str, parent: Option<&str>) -> Result<bool, String> {
    connection.query_row("SELECT EXISTS(SELECT 1 FROM deleted_sessions WHERE id=?1 OR id=?2)",
        rusqlite::params![id, parent], |row| row.get(0)).map_err(db_error)
}

#[cfg(test)]
mod tests;
