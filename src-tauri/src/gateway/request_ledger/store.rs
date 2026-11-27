use super::{migration, ClaudeRequestRecord, MAX_ENTRIES, MAX_SPEND_DAYS};
use asb_core::contracts::decimal_micros;
use chrono::{DateTime, Datelike, Months, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{path::Path, time::Duration};

const APPLICATION_ID: i64 = 0x41534243;
const VERSION: i64 = 1;
const SCHEMA: &str = "
    CREATE TABLE requests (
        id INTEGER PRIMARY KEY,
        at_seconds INTEGER NOT NULL,
        at_nanos INTEGER NOT NULL,
        body TEXT NOT NULL
    ) STRICT;
    CREATE INDEX request_time ON requests (at_seconds DESC, at_nanos DESC, id DESC);
    CREATE TABLE daily_spend (
        day INTEGER NOT NULL,
        profile TEXT NOT NULL,
        micros BLOB NOT NULL CHECK (length(micros) = 8),
        PRIMARY KEY (day, profile)
    ) WITHOUT ROWID, STRICT;
    CREATE INDEX spend_profile_day ON daily_spend (profile, day);
";

pub(super) fn open(root: &Path) -> Result<Connection, String> {
    let mut connection = Connection::open(root.join("claude-request-ledger.sqlite3")).map_err(error)?;
    connection.busy_timeout(Duration::from_secs(5)).map_err(error)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(error)?;
    let owner: i64 = transaction.pragma_query_value(None, "application_id", |row| row.get(0)).map_err(error)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0)).map_err(error)?;
    match (owner, version) {
        (APPLICATION_ID, VERSION) => {}
        (0, 0) if !transaction.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema)", [], |row| row.get::<_, bool>(0)).map_err(error)? => {
            transaction.execute_batch(SCHEMA).map_err(error)?;
            migration::import(&transaction, root)?;
            transaction.pragma_update(None, "application_id", APPLICATION_ID).map_err(error)?;
            transaction.pragma_update(None, "user_version", VERSION).map_err(error)?;
        }
        _ => return Err("Claude 请求账本标识或版本不受支持，已保留原文件".into()),
    }
    transaction.commit().map_err(error)?;
    Ok(connection)
}

pub(super) fn append(connection: &mut Connection, record: &ClaudeRequestRecord, at: DateTime<Utc>) -> Result<(), String> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(error)?;
    insert_record(&transaction, record, &at)?;
    add_spend(&transaction, record, at.date_naive())?;
    transaction.prepare_cached(
        "DELETE FROM requests WHERE id IN (
            SELECT id FROM requests ORDER BY at_seconds DESC, at_nanos DESC, id DESC LIMIT -1 OFFSET ?1)"
    ).map_err(error)?.execute([MAX_ENTRIES]).map_err(error)?;
    transaction.commit().map_err(error)
}

pub(super) fn insert_record(connection: &Connection, record: &ClaudeRequestRecord, at: &DateTime<Utc>) -> Result<(), String> {
    let body = serde_json::to_string(record).map_err(|error| format!("Claude 请求记录序列化失败：{error}"))?;
    connection.prepare_cached("INSERT INTO requests (at_seconds, at_nanos, body) VALUES (?1, ?2, ?3)")
        .map_err(error)?.execute(params![at.timestamp(), at.timestamp_subsec_nanos(), body]).map_err(error)?;
    Ok(())
}

pub(super) fn add_spend(connection: &Connection, record: &ClaudeRequestRecord, date: NaiveDate) -> Result<(), String> {
    let (Some(profile), Some(cost)) = (&record.profile_id, &record.cost) else { return Ok(()); };
    let day = date.num_days_from_ce();
    let amount: Option<Vec<u8>> = connection.prepare_cached("SELECT micros FROM daily_spend WHERE day=?1 AND profile=?2")
        .map_err(error)?.query_row(params![day, profile], |row| row.get(0)).optional().map_err(error)?;
    let previous = amount.map(parse_amount).transpose()?.unwrap_or(0);
    let total = previous.checked_add(decimal_micros(&cost.total_usd)?)
        .ok_or("Claude 费用累计超出范围")?;
    let existing_day: bool = connection.prepare_cached("SELECT EXISTS(SELECT 1 FROM daily_spend WHERE day=?1)")
        .map_err(error)?.query_row([day], |row| row.get(0)).map_err(error)?;
    set_spend(connection, day, profile, total)?;
    if !existing_day {
        connection.prepare_cached("DELETE FROM daily_spend WHERE day < (
            SELECT day FROM daily_spend GROUP BY day ORDER BY day DESC LIMIT 1 OFFSET ?1)"
        ).map_err(error)?.execute([MAX_SPEND_DAYS - 1]).map_err(error)?;
    }
    Ok(())
}

pub(super) fn set_spend(connection: &Connection, day: i32, profile: &str, amount: u64) -> Result<(), String> {
    // SQLite INTEGER is signed, while the ledger preserves the full u64 range.
    connection.prepare_cached("INSERT INTO daily_spend (day, profile, micros) VALUES (?1, ?2, ?3)
        ON CONFLICT(day, profile) DO UPDATE SET micros=excluded.micros")
        .map_err(error)?.execute(params![day, profile, &amount.to_be_bytes()[..]]).map_err(error)?;
    Ok(())
}

pub(super) fn totals(connection: &Connection, profile: &str, today: NaiveDate) -> Result<(u64, u64), String> {
    let first = today.with_day(1).ok_or("Claude 费用月份无效")?;
    let next = first.checked_add_months(Months::new(1)).ok_or("Claude 费用月份无效")?;
    let mut statement = connection.prepare_cached(
        "SELECT day, micros FROM daily_spend WHERE profile=?1 AND day >= ?2 AND day < ?3"
    ).map_err(error)?;
    let mut rows = statement.query(params![profile, first.num_days_from_ce(), next.num_days_from_ce()]).map_err(error)?;
    let (mut daily, mut monthly) = (0u64, 0u64);
    while let Some(row) = rows.next().map_err(error)? {
        let day: i32 = row.get(0).map_err(error)?;
        let amount = parse_amount(row.get(1).map_err(error)?)?;
        if day == today.num_days_from_ce() { daily = amount; }
        monthly = monthly.saturating_add(amount);
    }
    Ok((daily, monthly))
}

fn parse_amount(value: Vec<u8>) -> Result<u64, String> {
    value.try_into().map(u64::from_be_bytes).map_err(|_| "Claude 账本的费用记录无效".into())
}

fn error(error: rusqlite::Error) -> String {
    format!("Claude 请求账本读写失败：{error}")
}
