use super::{Error, Result, Scope, TARGET};
use rusqlite::{params, Connection, DatabaseName, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Row { pub id: String, pub provider: String, pub rollout: String, pub archived: bool }

pub(super) fn open(path: &Path, write: bool) -> Result<Connection> {
    let flags = if write { OpenFlags::SQLITE_OPEN_READ_WRITE } else { OpenFlags::SQLITE_OPEN_READ_ONLY };
    let connection = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    connection.busy_timeout(Duration::from_secs(2))?;
    Ok(connection)
}

pub(super) fn rows(scope: &Scope) -> Result<Option<Vec<Row>>> {
    if !scope.database().try_exists()? { return Ok(None); }
    read(&open(&scope.database(), false)?).map(Some)
}

pub(super) fn read(connection: &Connection) -> Result<Vec<Row>> {
    let mut query = connection.prepare("SELECT id,model_provider,rollout_path,archived FROM threads ORDER BY id")?;
    let rows = query.query_map([], |row| Ok(Row { id: row.get(0)?, provider: row.get(1)?,
        rollout: row.get(2)?, archived: row.get(3)? }))?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub(super) fn backup(scope: &Scope, directory: &Path) -> Result<Option<String>> {
    if !scope.database().try_exists()? { return Ok(None); }
    let path = directory.join("state_5.sqlite");
    open(&scope.database(), false)?.backup(DatabaseName::Main, &path, None)?;
    std::fs::File::options().write(true).open(&path)?.sync_all()?;
    Ok(Some(super::transcript::digest(&path)?))
}

pub(super) fn apply(connection: &Connection, before: &[Row]) -> Result<()> {
    for row in before.iter().filter(|row| row.provider != TARGET) {
        let changed = connection.execute("UPDATE threads SET model_provider=?1 WHERE id=?2 AND model_provider COLLATE BINARY=?3",
            params![TARGET, row.id, row.provider])?;
        if changed != 1 { return Err(Error::Changed); }
    }
    Ok(())
}

pub(super) fn check_restore(connection: &Connection, before: &[Row]) -> Result<()> {
    for row in before.iter().filter(|row| row.provider != TARGET) {
        let current: Option<(String, String)> = connection.query_row(
            "SELECT model_provider,rollout_path FROM threads WHERE id=?1", [&row.id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        if current.as_ref().is_none_or(|(provider, rollout)| rollout != &row.rollout || (provider != &row.provider && provider != TARGET)) {
            return Err(Error::Conflict(format!("Thread {}: provider or transcript registration changed", row.id)));
        }
    }
    Ok(())
}

pub(super) fn restore(connection: &Connection, before: &[Row]) -> Result<()> {
    check_restore(connection, before)?;
    for row in before.iter().filter(|row| row.provider != TARGET) {
        connection.execute("UPDATE threads SET model_provider=?1 WHERE id=?2", params![row.provider, row.id])?;
    }
    Ok(())
}
