use rusqlite::{Connection, OptionalExtension};

const ENTRIES: &str = "CREATE TABLE entries (
 kind TEXT NOT NULL, cache_key TEXT NOT NULL, revision TEXT NOT NULL, data TEXT NOT NULL,
 PRIMARY KEY(kind,cache_key)
) STRICT";

const USAGE: &[(&str, &str)] = &[
    ("usage_calendar", "CREATE TABLE usage_calendar (scope TEXT PRIMARY KEY NOT NULL,zone TEXT NOT NULL) STRICT"),
    ("usage_sources", "CREATE TABLE usage_sources (
 scope TEXT NOT NULL,id TEXT NOT NULL,app TEXT NOT NULL,session TEXT NOT NULL,ordering INTEGER NOT NULL,
 revision TEXT NOT NULL,cursor TEXT NOT NULL,issues TEXT NOT NULL,PRIMARY KEY(scope,id)
) STRICT"),
    ("usage_observations", "CREATE TABLE usage_observations (
 scope TEXT NOT NULL,source TEXT NOT NULL,ordinal INTEGER NOT NULL,app TEXT NOT NULL,
 identity TEXT NOT NULL,turn TEXT,data TEXT NOT NULL,PRIMARY KEY(scope,source,ordinal)
) STRICT"),
    ("usage_models", "CREATE TABLE usage_models (
 scope TEXT NOT NULL,source TEXT NOT NULL,turn TEXT NOT NULL,model TEXT,PRIMARY KEY(scope,source,turn)
) STRICT"),
    ("usage_calls", "CREATE TABLE usage_calls (
 scope TEXT NOT NULL,app TEXT NOT NULL,identity TEXT NOT NULL,data TEXT NOT NULL,
 issues TEXT NOT NULL,PRIMARY KEY(scope,app,identity)
) STRICT"),
    ("usage_buckets", "CREATE TABLE usage_buckets (
 scope TEXT NOT NULL,app TEXT NOT NULL,model TEXT NOT NULL,day TEXT NOT NULL,session TEXT NOT NULL,
 data TEXT NOT NULL,PRIMARY KEY(scope,app,model,day,session)
) STRICT"),
    ("usage_snapshot", "CREATE TABLE usage_snapshot (
 scope TEXT PRIMARY KEY NOT NULL,cached_at TEXT NOT NULL,reports TEXT NOT NULL
) STRICT"),
    ("usage_observation_call", "CREATE INDEX usage_observation_call ON usage_observations(scope,app,identity)"),
    ("usage_observation_turn", "CREATE INDEX usage_observation_turn ON usage_observations(scope,source,turn)"),
    ("usage_call_issues", "CREATE INDEX usage_call_issues ON usage_calls(scope) WHERE issues<>'[]'"),
];

pub(super) fn initialize(connection: &Connection) -> Result<(), String> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0)).map_err(error)?;
    if !(0..=3).contains(&version) { return Err("会话读取索引版本不受支持，已保留原件".into()); }
    let objects: i64 = connection.query_row("SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'", [], |row| row.get(0)).map_err(error)?;
    if version == 0 && objects != 0 { return Err("会话读取索引模式未知，已保留原件".into()); }
    if version > 0 { validate_object(connection, "entries", ENTRIES)?; }
    if version < 3 {
        if version > 0 && objects != 1 { return Err("会话读取索引旧模式无效，已保留原件".into()); }
        let transaction = connection.unchecked_transaction().map_err(error)?;
        if version == 0 { transaction.execute_batch(ENTRIES).map_err(error)?; }
        if version == 1 { transaction.execute("DELETE FROM entries", []).map_err(error)?; }
        for (_, sql) in USAGE { transaction.execute_batch(sql).map_err(error)?; }
        if version == 2 { transfer_snapshot(&transaction)?; }
        transaction.execute("DELETE FROM entries WHERE kind IN ('usage-file','usage-saved-events','usage-reports')", []).map_err(error)?;
        transaction.pragma_update(None, "user_version", 3).map_err(error)?;
        transaction.commit().map_err(error)?;
    }
    for (name, sql) in USAGE { validate_object(connection, name, sql)?; }
    let objects: usize = connection.query_row("SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'", [], |row| row.get(0)).map_err(error)?;
    if objects != USAGE.len() + 1 { return Err("会话读取索引存在未知结构，已保留原件".into()); }
    Ok(())
}

fn transfer_snapshot(connection: &Connection) -> Result<(), String> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Snapshot { cached_at: String, reports: Vec<asb_core::contracts::ModelUsageReport> }
    let mut statement = connection.prepare("SELECT cache_key,data FROM entries WHERE kind='usage-reports'").map_err(error)?;
    let mut rows = statement.query([]).map_err(error)?;
    while let Some(row) = rows.next().map_err(error)? {
        let raw: String = row.get(1).map_err(error)?;
        let Ok(snapshot) = serde_json::from_str::<Snapshot>(&raw) else { continue; };
        let old_key: String = row.get(0).map_err(error)?;
        let Ok((roots, Ok(native))) = serde_json::from_str::<(Vec<std::path::PathBuf>,Result<std::path::PathBuf,String>)>(&old_key) else { continue; };
        let scope = super::key(&(roots,native))?;
        use asb_core::contracts::ModelUsageRange::{Today,Last7Days,Last30Days,All};
        if !snapshot.reports.iter().map(|report| report.range).eq([Today,Last7Days,Last30Days,All])
            || chrono::DateTime::parse_from_rfc3339(&snapshot.cached_at).is_err() { continue; }
        connection.execute("INSERT INTO usage_snapshot(scope,cached_at,reports) VALUES(?1,?2,?3)",
            rusqlite::params![scope,snapshot.cached_at,
                serde_json::to_string(&snapshot.reports).map_err(|error| error.to_string())?]).map_err(error)?;
    }
    Ok(())
}

fn validate_object(connection: &Connection, name: &str, expected: &str) -> Result<(), String> {
    let actual: Option<String> = connection.query_row("SELECT sql FROM sqlite_master WHERE name=?1", [name], |row| row.get(0)).optional().map_err(error)?;
    if actual.as_deref() != Some(expected) { return Err(format!("会话读取索引结构 {name} 无效，已保留原件")); }
    Ok(())
}

fn error(error: rusqlite::Error) -> String { format!("会话读取索引升级失败：{error}") }
