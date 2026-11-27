use super::{store, validate_record, validate_text, ClaudeRequestRecord, MAX_ENTRIES, MAX_SPEND_DAYS};
use chrono::{Datelike, NaiveDate, Utc};
use rusqlite::Connection;
use serde::Deserialize;
use std::{collections::BTreeMap, fs, io::ErrorKind, path::Path};

type DailySpend = BTreeMap<String, BTreeMap<String, u64>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct V1 {
    version: u8,
    #[serde(default)]
    entries: Vec<ClaudeRequestRecord>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct V2 {
    version: u8,
    #[serde(default)]
    entries: Vec<ClaudeRequestRecord>,
    spend_by_day_utc: DailySpend,
}

pub(super) fn import(connection: &Connection, root: &Path) -> Result<(), String> {
    let bytes = match fs::read(root.join("claude-request-ledger.json")) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("旧 Claude 请求账本不可读：{error}")),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(invalid)?;
    let (entries, spend) = match value.get("version").and_then(serde_json::Value::as_u64) {
        Some(1) => {
            let ledger: V1 = serde_json::from_value(value).map_err(invalid)?;
            debug_assert_eq!(ledger.version, 1);
            (ledger.entries, None)
        }
        Some(2) => {
            let ledger: V2 = serde_json::from_value(value).map_err(invalid)?;
            debug_assert_eq!(ledger.version, 2);
            (ledger.entries, Some(ledger.spend_by_day_utc))
        }
        _ => return Err("旧 Claude 请求账本版本不受支持，已保留原文件".into()),
    };
    if entries.len() > MAX_ENTRIES { return Err("旧 Claude 请求账本条目数量超过限制".into()); }
    for record in entries {
        let at = validate_record(&record)?.with_timezone(&Utc);
        store::insert_record(connection, &record, &at)?;
        if spend.is_none() { store::add_spend(connection, &record, at.date_naive())?; }
    }
    if let Some(spend) = spend { import_spend(connection, spend)?; }
    Ok(())
}

fn import_spend(connection: &Connection, spend: DailySpend) -> Result<(), String> {
    if spend.len() > MAX_SPEND_DAYS { return Err("旧 Claude 费用累计天数超出范围".into()); }
    for (day_text, profiles) in spend {
        let date = NaiveDate::parse_from_str(&day_text, "%Y-%m-%d")
            .map_err(|_| "旧 Claude 费用累计日期无效")?;
        if date.to_string() != day_text { return Err("旧 Claude 费用累计日期格式不明确".into()); }
        for (profile, amount) in profiles {
            validate_text(&profile)?;
            store::set_spend(connection, date.num_days_from_ce(), &profile, amount)?;
        }
    }
    Ok(())
}

fn invalid(error: serde_json::Error) -> String {
    format!("旧 Claude 请求账本格式无效，已保留原文件：{error}")
}
