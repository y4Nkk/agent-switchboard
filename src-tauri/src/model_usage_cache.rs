//! All calendar ranges share one derived snapshot and the same indexed call facts.
use crate::local_state::LocalState;
use crate::session_manager::{read_index::{key, FileRevision, ReadIndex}, SessionIssue, SessionSource};
use asb_core::contracts::{ModelUsageFreshness, ModelUsageRead, ModelUsageReport, ModelUsageRequest};
use chrono::{DateTime, Duration, Local, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

pub(crate) const MODEL_USAGE_REFRESH_INTERVAL: Duration = Duration::minutes(5);
static REFRESH_GATE: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot { cached_at: String, reports: Vec<ModelUsageReport> }

pub(crate) fn get_or_refresh(state: &LocalState, request: ModelUsageRequest) -> Result<ModelUsageRead, String> {
    let _guard = REFRESH_GATE.lock().map_err(|_| "本地用量索引刷新锁不可用")?;
    let index = ReadIndex::open(state.root())?;
    let roots = crate::session_manager::codex_session_roots()?;
    let native = crate::local_state::codex_paths::sqlite_root(&crate::session_manager::codex_root()?);
    let cache_key = key(&(roots, native))?;
    let (mut sources, mut issues) = crate::session_manager::scan_usage_sources(state.root())?;
    sources.sort_by(|left, right| (left.meta.app, &left.meta.session_id).cmp(&(right.meta.app, &right.meta.session_id)));
    issues.retain(|issue| issue.conversion_id.is_none());
    issues.sort_by(|left, right| (left.app, &left.message).cmp(&(right.app, &right.message)));
    let revision = source_revision(state.root(), &sources, &issues)?;
    if !request.force_refresh {
        if let Some(snapshot) = index.get::<Snapshot>("usage-reports", &cache_key, &revision)? {
            if DateTime::parse_from_rfc3339(&refresh_after(&snapshot.cached_at)?).map_err(|error| error.to_string())?
                .with_timezone(&Utc) > Utc::now() {
                return read(snapshot, request, ModelUsageFreshness::Cached);
            }
        }
    }
    let reports = crate::model_usage::reports(state.root(), sources.clone(), issues.clone(), Local::now())?;
    if source_revision(state.root(), &sources, &issues)? != revision { return Err("会话来源在汇总期间发生变化，请刷新重试".into()); }
    let snapshot = Snapshot { cached_at: Utc::now().to_rfc3339(), reports };
    index.put("usage-reports", &cache_key, &revision, &snapshot)?;
    read(snapshot, request, ModelUsageFreshness::Fresh)
}

fn source_revision(root: &std::path::Path, sources: &[SessionSource], issues: &[SessionIssue]) -> Result<String, String> {
    let mut files = Vec::new();
    for source in sources {
        let revisions: Vec<_> = source.paths.iter().filter(|_| source.retained.is_empty()).map(|path| (path, FileRevision::read(path))).collect();
        files.push((&source.meta, revisions, &source.retained));
    }
    let retained = crate::session_manager::history::revision(root);
    let encoded = key(&(Local::now().format("%Y-%m-%d%:z").to_string(), files, issues, retained))?;
    Ok(asb_switch::sha256_hex(&encoded))
}

fn read(snapshot: Snapshot, request: ModelUsageRequest, freshness: ModelUsageFreshness) -> Result<ModelUsageRead, String> {
    use asb_core::contracts::ModelUsageRange::{Today, Last7Days, Last30Days, All};
    if !snapshot.reports.iter().map(|report| report.range).eq([Today, Last7Days, Last30Days, All]) {
        return Err("本地用量索引的日期范围集合无效".into());
    }
    let refresh_after = refresh_after(&snapshot.cached_at)?;
    let report = snapshot.reports.into_iter().find(|report| report.range == request.range)
        .ok_or("本地用量索引缺少当前日期范围")?;
    Ok(ModelUsageRead { report, freshness, refresh_after })
}

fn refresh_after(cached_at: &str) -> Result<String, String> {
    let cached_at = DateTime::parse_from_rfc3339(cached_at).map_err(|error| error.to_string())?.with_timezone(&Utc);
    let expires = cached_at.checked_add_signed(MODEL_USAGE_REFRESH_INTERVAL).ok_or("用量索引刷新时间无效")?;
    let midnight = cached_at.with_timezone(&Local).date_naive().succ_opt()
        .and_then(|date| date.and_hms_opt(0, 0, 0)).and_then(|date| date.and_local_timezone(Local).earliest())
        .ok_or("无法确定本机日期边界")?.with_timezone(&Utc);
    Ok(expires.min(midnight).to_rfc3339())
}

#[cfg(test)]
mod tests;
