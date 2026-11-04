use super::{parse, progress::Progress, store::error, TokenTotals, UsageEvent};
use crate::session_manager::read_index::key;
use asb_core::contracts::AppKind;
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Totals { tokens: TokenTotals, calls: u64 }

pub(super) struct Bucket {
    pub app: AppKind,
    pub model: Option<String>,
    pub day: Option<NaiveDate>,
    pub session: String,
    pub tokens: TokenTotals,
}

pub(super) fn adjust(connection: &Connection, scope: &str, event: &UsageEvent, add: bool) -> Result<(), String> {
    let app = key(&event.app)?;
    let model = key(&event.model)?;
    let day = event.at.as_deref().and_then(parse::local_date).map(|day| day.to_string()).unwrap_or_default();
    let raw: Option<String> = connection.prepare_cached("SELECT data FROM usage_buckets
        WHERE scope=?1 AND app=?2 AND model=?3 AND day=?4 AND session=?5")
        .map_err(error)?.query_row(params![scope,app,model,day,event.session], |row| row.get(0)).optional().map_err(error)?;
    let mut totals: Totals = raw.map(|raw| serde_json::from_str(&raw).map_err(error)).transpose()?.unwrap_or_default();
    if add {
        totals.tokens = totals.tokens.checked_add(event.tokens)?;
        totals.calls = totals.calls.checked_add(1).ok_or("用量调用数溢出")?;
    } else {
        totals.tokens = totals.tokens.checked_sub(event.tokens)?;
        totals.calls = totals.calls.checked_sub(1).ok_or("用量调用数不一致")?;
    }
    if totals.calls == 0 {
        connection.prepare_cached("DELETE FROM usage_buckets WHERE scope=?1 AND app=?2 AND model=?3 AND day=?4 AND session=?5")
            .map_err(error)?.execute(params![scope,app,model,day,event.session]).map_err(error)?;
    } else {
        connection.prepare_cached("INSERT INTO usage_buckets(scope,app,model,day,session,data) VALUES(?1,?2,?3,?4,?5,?6)
            ON CONFLICT(scope,app,model,day,session) DO UPDATE SET data=excluded.data")
            .map_err(error)?.execute(params![scope,app,model,day,event.session,key(&totals)?]).map_err(error)?;
    }
    Ok(())
}

pub(super) fn calendar(connection: &Connection, scope: &str, progress: &Progress<'_>) -> Result<(), String> {
    let zone = key(&(iana_time_zone::get_timezone().map_err(error)?, std::env::var("TZ").ok()))?;
    let saved: Option<String> = connection.query_row("SELECT zone FROM usage_calendar WHERE scope=?1", [scope], |row| row.get(0)).optional().map_err(error)?;
    if saved.as_deref() == Some(&zone) { return Ok(()); }
    let transaction = connection.unchecked_transaction().map_err(error)?;
    transaction.execute("DELETE FROM usage_buckets WHERE scope=?1", [scope]).map_err(error)?;
    let count: usize = transaction.query_row("SELECT count(*) FROM usage_calls WHERE scope=?1", [scope], |row| row.get(0)).map_err(error)?;
    progress.calls(count);
    {
        let mut statement = transaction.prepare("SELECT data FROM usage_calls WHERE scope=?1").map_err(error)?;
        let mut rows = statement.query([scope]).map_err(error)?;
        while let Some(row) = rows.next().map_err(error)? {
            let event = serde_json::from_str(&row.get::<_,String>(0).map_err(error)?).map_err(error)?;
            adjust(&transaction, scope, &event, true)?;
            progress.call_done();
        }
    }
    transaction.execute("INSERT INTO usage_calendar(scope,zone) VALUES(?1,?2) ON CONFLICT(scope) DO UPDATE SET zone=excluded.zone",
        params![scope,zone]).map_err(error)?;
    transaction.commit().map_err(error)?;
    Ok(())
}

pub(super) fn read(connection: &Connection, scope: &str) -> Result<Vec<Bucket>, String> {
    let mut statement = connection.prepare("SELECT app,model,day,session,data FROM usage_buckets WHERE scope=?1").map_err(error)?;
    let mut rows = statement.query([scope]).map_err(error)?;
    let mut values = Vec::new();
    while let Some(row) = rows.next().map_err(error)? {
        let day: String = row.get(2).map_err(error)?;
        let totals: Totals = serde_json::from_str(&row.get::<_,String>(4).map_err(error)?).map_err(error)?;
        values.push(Bucket { app: serde_json::from_str(&row.get::<_,String>(0).map_err(error)?).map_err(error)?,
            model: serde_json::from_str(&row.get::<_,String>(1).map_err(error)?).map_err(error)?,
            day: if day.is_empty() { None } else { Some(day.parse().map_err(error)?) },
            session: row.get(3).map_err(error)?, tokens: totals.tokens });
    }
    Ok(values)
}
