use super::{ingest::{Cursor, Delta}, progress::Progress, resolve};
use crate::session_manager::read_index::key;
use asb_core::contracts::AppKind;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{BTreeSet, HashSet};

pub(super) struct Source {
    pub id: String,
    pub app: AppKind,
    pub session: String,
    pub order: usize,
}

pub(super) fn error(error: impl std::fmt::Display) -> String { format!("用量读取索引不可用：{error}") }

pub(super) fn previous(connection: &Connection, scope: &str, id: &str) -> Result<Option<(String, Cursor)>, String> {
    let row: Option<(String, String)> = connection.prepare_cached(
        "SELECT revision,cursor FROM usage_sources WHERE scope=?1 AND id=?2").map_err(error)?.query_row(params![scope,id],
        |row| Ok((row.get(0)?, row.get(1)?))).optional().map_err(error)?;
    let Some((revision, raw)) = row else { return Ok(None); };
    match serde_json::from_str(&raw) {
        Ok(cursor) => Ok(Some((revision,cursor))),
        Err(cause) => {
            log::warn!("用量来源游标损坏，将重新验证该来源：{cause}");
            Ok(None)
        }
    }
}

pub(super) fn apply(connection: &Connection, scope: &str, source: &Source, revision: &str,
    delta: Delta, progress: &Progress<'_>) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(error)?;
    let mut affected = BTreeSet::new();
    if delta.reset {
        affected.extend(identities(&transaction, scope, &source.id)?);
        clear_source(&transaction, scope, &source.id)?;
    }
    transaction.execute("INSERT INTO usage_sources(scope,id,app,session,ordering,revision,cursor,issues)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(scope,id) DO UPDATE SET
        ordering=excluded.ordering,revision=excluded.revision,cursor=excluded.cursor,issues=excluded.issues",
        params![scope,source.id,key(&source.app)?,source.session,source.order,revision,key(&delta.cursor)?,key(&delta.cursor.issues)?]).map_err(error)?;
    update_models(&transaction, scope, source, &delta, &mut affected)?;
    for (ordinal, observation) in delta.observations {
        affected.insert(observation.identity().to_string());
        transaction.prepare_cached("INSERT INTO usage_observations(scope,source,ordinal,app,identity,turn,data)
            VALUES(?1,?2,?3,?4,?5,?6,?7)").map_err(error)?.execute(params![scope,source.id,ordinal,key(&source.app)?,
            observation.identity(),observation.turn(),key(&observation)?]).map_err(error)?;
    }
    progress.calls(affected.len());
    for identity in affected {
        resolve::update(&transaction, scope, source.app, &identity)?;
        progress.call_done();
    }
    transaction.commit().map_err(error)
}

fn update_models(connection: &Connection, scope: &str, source: &Source, delta: &Delta,
    affected: &mut BTreeSet<String>) -> Result<(), String> {
    for (turn, model) in &delta.models {
        let mut statement = connection.prepare_cached("SELECT DISTINCT identity FROM usage_observations WHERE scope=?1 AND source=?2 AND turn=?3").map_err(error)?;
        let rows = statement.query_map(params![scope,source.id,turn], |row| row.get::<_,String>(0)).map_err(error)?;
        for row in rows { affected.insert(row.map_err(error)?); }
        connection.prepare_cached("INSERT INTO usage_models(scope,source,turn,model) VALUES(?1,?2,?3,?4)
            ON CONFLICT(scope,source,turn) DO UPDATE SET model=excluded.model").map_err(error)?.execute(params![scope,source.id,turn,model]).map_err(error)?;
    }
    Ok(())
}

fn identities(connection: &Connection, scope: &str, id: &str) -> Result<Vec<String>, String> {
    let mut statement = connection.prepare("SELECT DISTINCT identity FROM usage_observations WHERE scope=?1 AND source=?2").map_err(error)?;
    let rows = statement.query_map(params![scope,id], |row| row.get(0)).map_err(error)?;
    rows.collect::<Result<_,_>>().map_err(error)
}

fn clear_source(connection: &Connection, scope: &str, id: &str) -> Result<(), String> {
    connection.execute("DELETE FROM usage_observations WHERE scope=?1 AND source=?2", params![scope,id]).map_err(error)?;
    connection.execute("DELETE FROM usage_models WHERE scope=?1 AND source=?2", params![scope,id]).map_err(error)?;
    Ok(())
}

pub(super) fn reconcile(connection: &Connection, scope: &str, sources: &[Source], complete: bool,
    progress: &Progress<'_>) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(error)?;
    for source in sources {
        transaction.execute("UPDATE usage_sources SET ordering=?3 WHERE scope=?1 AND id=?2 AND ordering<>?3",
            params![scope,source.id,source.order]).map_err(error)?;
    }
    if complete {
        let present: HashSet<_> = sources.iter().map(|source| source.id.as_str()).collect();
        let mut statement = transaction.prepare("SELECT id,app FROM usage_sources WHERE scope=?1").map_err(error)?;
        let rows = statement.query_map([scope], |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).map_err(error)?;
        let removed = rows.collect::<Result<Vec<_>,_>>().map_err(error)?;
        drop(statement);
        for (id, app) in removed.into_iter().filter(|(id,_)| !present.contains(id.as_str())) {
            let affected = identities(&transaction, scope, &id)?;
            clear_source(&transaction, scope, &id)?;
            transaction.execute("DELETE FROM usage_sources WHERE scope=?1 AND id=?2", params![scope,id]).map_err(error)?;
            progress.calls(affected.len());
            for identity in affected {
                resolve::update(&transaction, scope, serde_json::from_str(&app).map_err(error)?, &identity)?;
                progress.call_done();
            }
        }
    }
    transaction.commit().map_err(error)
}

pub(super) fn issues(connection: &Connection, scope: &str) -> Result<Vec<asb_core::contracts::ModelUsageIssue>, String> {
    let mut statement = connection.prepare("SELECT app,issues FROM usage_sources WHERE scope=?1 AND issues<>'[]'
        UNION ALL SELECT app,issues FROM usage_calls WHERE scope=?1 AND issues<>'[]'").map_err(error)?;
    let mut rows = statement.query([scope]).map_err(error)?;
    let mut issues = Vec::new();
    while let Some(row) = rows.next().map_err(error)? {
        let app = serde_json::from_str(&row.get::<_,String>(0).map_err(error)?).map_err(error)?;
        let messages: Vec<String> = serde_json::from_str(&row.get::<_,String>(1).map_err(error)?).map_err(error)?;
        issues.extend(messages.into_iter().map(|message| asb_core::contracts::ModelUsageIssue { app, message }));
    }
    Ok(issues)
}
