use super::{buckets, ingest::Observation, parse, store::error, TokenTotals, UsageEvent};
use crate::codex_metering::merge_token_event;
use crate::session_manager::read_index::key;
use asb_core::contracts::AppKind;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;

pub(super) fn update(connection: &Connection, scope: &str, app: AppKind, identity: &str) -> Result<(), String> {
    let app_key = key(&app)?;
    let old: Option<String> = connection.prepare_cached("SELECT data FROM usage_calls WHERE scope=?1 AND app=?2 AND identity=?3")
        .map_err(error)?.query_row(params![scope,app_key,identity], |row| row.get(0)).optional().map_err(error)?;
    let old: Option<UsageEvent> = old.map(|raw| serde_json::from_str(&raw).map_err(error)).transpose()?;
    let (next, issues) = select(connection, scope, &app_key, identity)?;
    if old != next {
        if let Some(old) = &old { buckets::adjust(connection, scope, old, false)?; }
        if let Some(next) = &next { buckets::adjust(connection, scope, next, true)?; }
    }
    match next {
        Some(next) => {
            connection.prepare_cached("INSERT INTO usage_calls(scope,app,identity,data,issues) VALUES(?1,?2,?3,?4,?5)
                ON CONFLICT(scope,app,identity) DO UPDATE SET data=excluded.data,issues=excluded.issues")
                .map_err(error)?.execute(params![scope,app_key,identity,key(&next)?,key(&issues)?]).map_err(error)?;
        }
        None => { connection.prepare_cached("DELETE FROM usage_calls WHERE scope=?1 AND app=?2 AND identity=?3")
            .map_err(error)?.execute(params![scope,app_key,identity]).map_err(error)?; }
    }
    Ok(())
}

fn select(connection: &Connection, scope: &str, app: &str, identity: &str) -> Result<(Option<UsageEvent>, Vec<String>), String> {
    let mut statement = connection.prepare_cached("SELECT o.data,m.model FROM usage_observations o
        JOIN usage_sources s ON s.scope=o.scope AND s.id=o.source
        LEFT JOIN usage_models m ON m.scope=o.scope AND m.source=o.source AND m.turn=o.turn
        WHERE o.scope=?1 AND o.app=?2 AND o.identity=?3 ORDER BY s.session,s.ordering,o.ordinal").map_err(error)?;
    let mut rows = statement.query(params![scope,app,identity]).map_err(error)?;
    let mut codex = HashMap::new();
    let mut claude = HashMap::new();
    let mut issues = Vec::new();
    while let Some(row) = rows.next().map_err(error)? {
        let observation: Observation = serde_json::from_str(&row.get::<_,String>(0).map_err(error)?).map_err(error)?;
        match observation {
            Observation::Codex { mut event, .. } => {
                event.model = row.get(1).map_err(error)?;
                if let Err(issue) = merge_token_event(&mut codex, event) { issues.push(issue); }
            }
            Observation::Claude(candidate) => parse::merge(&mut claude, candidate),
        }
    }
    let event = codex.into_values().next().map(|event| {
        let tokens = event.tokens;
        UsageEvent { app: AppKind::Codex, identity: event.identity, session: event.owner,
            model: event.model, at: event.at, tokens: TokenTotals { input: tokens.input - tokens.cached_input - tokens.cache_write_input,
                cache_read: tokens.cached_input, cache_creation: tokens.cache_write_input,
                output: tokens.output, total: tokens.total().expect("validated receipt") } }
    }).or_else(|| claude.into_values().next().map(|candidate| candidate.event));
    Ok((event, issues))
}
