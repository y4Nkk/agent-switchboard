//! Snapshot reads never discover sources or wait for a refresh task.
mod task;
pub(crate) use task::ModelUsageManager;
use crate::local_state::LocalState;
use crate::session_manager::read_index::{key, ReadIndex};
use asb_core::contracts::{ModelUsageFreshness, ModelUsageProgress, ModelUsageRead, ModelUsageReport};
use chrono::{DateTime, Duration, Local, Utc};
use rusqlite::{params, OptionalExtension};
use std::path::Path;

const MODEL_USAGE_REFRESH_INTERVAL: Duration = Duration::minutes(5);

fn cache_key() -> Result<String, String> {
    let roots = crate::session_manager::codex_session_roots()?;
    let native = crate::local_state::codex_paths::sqlite_root(&crate::session_manager::codex_root()?)?;
    key(&(roots, native))
}

pub(crate) fn read_cached(state: &LocalState) -> Result<Option<ModelUsageRead>, String> {
    let index = ReadIndex::open(state.root())?;
    read_snapshot(&index, &cache_key()?)
}

fn read_snapshot(index: &ReadIndex, scope: &str) -> Result<Option<ModelUsageRead>, String> {
    let row: Option<(String,String)> = index.connection().query_row(
        "SELECT cached_at,reports FROM usage_snapshot WHERE scope=?1", [scope],
        |row| Ok((row.get(0)?,row.get(1)?))).optional().map_err(|error| error.to_string())?;
    row.map(|(at, raw)| {
        let reports = serde_json::from_str(&raw).map_err(|error| format!("用量汇总无效：{error}"))?;
        reading(reports, &at, ModelUsageFreshness::Cached, MODEL_USAGE_REFRESH_INTERVAL)
    }).transpose()
}

fn rebuild(root: &Path, scope: &str, publish: &dyn Fn(ModelUsageProgress)) -> Result<ModelUsageRead, String> {
    let progress = crate::model_usage::progress::Progress::new(publish);
    let index = ReadIndex::open(root)?;
    let manifest = crate::session_manager::usage_sources::discover(root, &index)?;
    let batch = crate::model_usage::refresh(&index, scope, manifest, Local::now, &progress)?;
    let cached_at = Utc::now().to_rfc3339();
    let transaction = index.connection().unchecked_transaction().map_err(|error| error.to_string())?;
    transaction.execute("INSERT INTO usage_snapshot(scope,cached_at,reports)
        VALUES(?1,?2,?3) ON CONFLICT(scope) DO UPDATE SET
        cached_at=excluded.cached_at,reports=excluded.reports",
        params![scope,cached_at,key(&batch.reports)?]).map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    crate::session_manager::diagnostics::report(root, "本地消耗统计",
        batch.reports.iter().take(1).flat_map(|report| report.issues.iter())
            .map(|issue| format!("{}: {}", issue.app.label(), issue.message)));
    reading(batch.reports, &cached_at, ModelUsageFreshness::Fresh,
        if batch.refresh_soon { Duration::seconds(10) } else { MODEL_USAGE_REFRESH_INTERVAL })
}

fn reading(reports: Vec<ModelUsageReport>, cached_at: &str, freshness: ModelUsageFreshness,
    interval: Duration) -> Result<ModelUsageRead, String> {
    use asb_core::contracts::ModelUsageRange::{Today,Last7Days,Last30Days,All};
    if !reports.iter().map(|report| report.range).eq([Today,Last7Days,Last30Days,All]) {
        return Err("本地用量索引的日期范围集合无效".into());
    }
    Ok(ModelUsageRead { reports, freshness, refresh_after: refresh_deadline(cached_at, interval)? })
}

fn refresh_deadline(cached_at: &str, interval: Duration) -> Result<String, String> {
    let cached_at = DateTime::parse_from_rfc3339(cached_at).map_err(|error| error.to_string())?.with_timezone(&Utc);
    let expires = cached_at.checked_add_signed(interval).ok_or("用量索引刷新时间无效")?;
    let midnight = cached_at.with_timezone(&Local).date_naive().succ_opt()
        .and_then(|date| date.and_hms_opt(0,0,0)).and_then(|date| date.and_local_timezone(Local).earliest())
        .ok_or("无法确定本机日期边界")?.with_timezone(&Utc);
    Ok(expires.min(midnight).to_rfc3339())
}

#[cfg(test)]
mod tests;
