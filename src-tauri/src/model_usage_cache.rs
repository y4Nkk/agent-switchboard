//! Persisted, credential-free snapshots of local session token totals.
//!
//! This cache owns the one refresh lifetime for the model-usage page. It is
//! deliberately separate from provider and official-quota history: its values
//! come only from local Codex and Claude Code session records.

use crate::local_state::LocalState;
use asb_core::contracts::{
    ModelUsageFreshness, ModelUsageRange, ModelUsageRead, ModelUsageReport, ModelUsageRequest,
};
use chrono::{DateTime, Duration, Local, Utc};

pub(crate) const MODEL_USAGE_REFRESH_INTERVAL: Duration = Duration::minutes(5);

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ModelUsageCache {
    source_roots_revision: String,
    entries: Vec<CachedModelUsage>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CachedModelUsage {
    range: ModelUsageRange,
    cached_at: String,
    report: ModelUsageReport,
}

impl ModelUsageCache {
    fn cached(&self, range: ModelUsageRange) -> Option<&CachedModelUsage> {
        self.entries.iter().find(|entry| entry.range == range)
    }

    fn replace(&mut self, next: CachedModelUsage) {
        if let Some(current) = self
            .entries
            .iter_mut()
            .find(|entry| entry.range == next.range)
        {
            *current = next;
        } else {
            self.entries.push(next);
        }
    }

    fn validate(&self) -> Result<(), String> {
        for (index, entry) in self.entries.iter().enumerate() {
            if entry.report.range != entry.range || refresh_after(&entry.cached_at).is_err() {
                return Err("本地会话快照格式无效".to_string());
            }
            if self.entries[..index]
                .iter()
                .any(|previous| previous.range == entry.range)
            {
                return Err("本地会话快照格式无效".to_string());
            }
        }
        Ok(())
    }
}

/// Resolves one range through the persisted snapshot unless the request
/// explicitly forces a new scan. A corrupt cache is recoverable derived state:
/// the fixed local roots are re-scanned and replace it atomically.
pub(crate) fn get_or_refresh(
    state: &LocalState,
    request: ModelUsageRequest,
) -> Result<ModelUsageRead, String> {
    let roots = crate::session_manager::session_roots()?;
    let native_root = crate::local_state::codex_paths::sqlite_root(&crate::session_manager::codex_root()?);
    let calendar = Local::now().format("%Y-%m-%d%:z").to_string();
    let encoded = serde_json::to_string(&("response-receipts-workflow-sources", &roots, native_root, calendar)).map_err(|_| "会话来源目录无法序列化")?;
    resolve_source(state, request, asb_switch::sha256_hex(&encoded), |range| {
        let (sources, issues) = crate::session_manager::scan_native_sources()?;
        Ok(crate::model_usage::report(range, sources, issues, chrono::Local::now()))
    })
}

#[cfg(test)]
fn resolve<F>(
    state: &LocalState,
    request: ModelUsageRequest,
    scan: F,
) -> Result<ModelUsageRead, String>
where
    F: FnOnce(ModelUsageRange) -> ModelUsageReport,
{
    resolve_source(state, request, "fixture-session-roots".into(), |range| Ok(scan(range)))
}

fn resolve_source<F>(
    state: &LocalState,
    request: ModelUsageRequest,
    source_roots_revision: String,
    scan: F,
) -> Result<ModelUsageRead, String>
where
    F: FnOnce(ModelUsageRange) -> Result<ModelUsageReport, String>,
{
    let (mut cache, mut warnings) = match state.load_model_usage_cache() {
        Ok(Some(cache)) if cache.validate().is_ok() => (cache, Vec::new()),
        Ok(None) => (ModelUsageCache::default(), Vec::new()),
        Ok(Some(_)) | Err(_) => (
            ModelUsageCache::default(),
            vec!["本地会话快照不可读，已重新汇总并重建缓存。".to_string()],
        ),
    };

    if cache.source_roots_revision != source_roots_revision {
        cache = ModelUsageCache {
            source_roots_revision,
            entries: Vec::new(),
        };
    }

    if !request.force_refresh {
        if let Some(cached) = cache.cached(request.range).filter(|entry|
            refresh_after(&entry.cached_at).ok().and_then(|at| DateTime::parse_from_rfc3339(&at).ok())
                .is_some_and(|after| after.with_timezone(&Utc) > Utc::now())) {
            return cached_read(cached);
        }
    }

    let report = scan(request.range)?;
    let cached = CachedModelUsage {
        range: request.range,
        cached_at: Utc::now().to_rfc3339(),
        report: report.clone(),
    };
    let refresh_after = refresh_after(&cached.cached_at)?;
    cache.replace(cached);
    if state.save_model_usage_cache(&cache).is_err() {
        warnings.push(
            "本次会话汇总已显示，但未能写入本地快照；下次打开应用可能需要重新汇总。".to_string(),
        );
    }

    Ok(ModelUsageRead {
        report,
        freshness: ModelUsageFreshness::Fresh,
        refresh_after,
        cache_warning: (!warnings.is_empty()).then(|| warnings.join("；")),
    })
}

fn cached_read(cached: &CachedModelUsage) -> Result<ModelUsageRead, String> {
    Ok(ModelUsageRead {
        report: cached.report.clone(),
        freshness: ModelUsageFreshness::Cached,
        refresh_after: refresh_after(&cached.cached_at)?,
        cache_warning: None,
    })
}

fn refresh_after(cached_at: &str) -> Result<String, String> {
    let cached_at = DateTime::parse_from_rfc3339(cached_at)
        .map_err(|_| "本地会话快照格式无效".to_string())?
        .with_timezone(&Utc);
    let expires = cached_at
        .checked_add_signed(MODEL_USAGE_REFRESH_INTERVAL)
        .ok_or_else(|| "本地会话快照格式无效".to_string())?;
    let midnight = cached_at.with_timezone(&Local).date_naive().succ_opt()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .and_then(|date| date.and_local_timezone(Local).earliest())
        .ok_or("无法确定本机日期边界，不能重用上一日期的用量快照")?.with_timezone(&Utc);
    Ok(expires.min(midnight).to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::ModelUsageTokens;
    use std::fs;

    fn request(range: ModelUsageRange, force_refresh: bool) -> ModelUsageRequest {
        ModelUsageRequest {
            range,
            force_refresh,
        }
    }

    fn report(range: ModelUsageRange, total_tokens: u64) -> ModelUsageReport {
        ModelUsageReport {
            range,
            generated_at: "2026-09-04T08:00:00Z".to_string(),
            groups: Vec::new(),
            days: Vec::new(),
            unassigned_tokens: ModelUsageTokens {
                total_tokens,
                ..ModelUsageTokens::default()
            },
            issues: Vec::new(), coverage: Default::default(),
        }
    }

    #[test]
    fn reopened_cache_returns_the_same_range_without_a_second_scan() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("state");
        let first = resolve(
            &LocalState::from_root(root.clone()),
            request(ModelUsageRange::Today, false),
            |range| report(range, 42),
        )
        .expect("first scan");
        assert_eq!(first.freshness, ModelUsageFreshness::Fresh);

        let cached = resolve(
            &LocalState::from_root(root.clone()),
            request(ModelUsageRange::Today, false),
            |_| panic!("cache hit must not scan"),
        )
        .expect("cached read");
        assert_eq!(cached.freshness, ModelUsageFreshness::Cached);
        assert_eq!(cached.report.unassigned_tokens.total_tokens, 42);

        let stored = fs::read_to_string(root.join("model-usage-cache.json")).expect("cache text");
        assert!(stored.contains("totalTokens"));
        assert!(!stored.contains("apiKey"));
    }

    #[test]
    fn forced_refresh_replaces_the_range_snapshot() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        resolve(&state, request(ModelUsageRange::Today, false), |range| {
            report(range, 42)
        })
        .expect("first scan");

        let refreshed = resolve(&state, request(ModelUsageRange::Today, true), |range| {
            report(range, 99)
        })
        .expect("forced scan");
        assert_eq!(refreshed.freshness, ModelUsageFreshness::Fresh);
        assert_eq!(refreshed.report.unassigned_tokens.total_tokens, 99);

        let cached = resolve(&state, request(ModelUsageRange::Today, false), |_| {
            panic!("fresh snapshot must be reused")
        })
        .expect("cached read");
        assert_eq!(cached.report.unassigned_tokens.total_tokens, 99);
    }

    #[test]
    fn malformed_cache_is_rebuilt_from_the_fixed_session_scan() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("state");
        fs::create_dir_all(&root).expect("create state");
        fs::write(root.join("model-usage-cache.json"), "not-json").expect("write malformed cache");
        let state = LocalState::from_root(root);

        let rebuilt = resolve(
            &state,
            request(ModelUsageRange::Last7Days, false),
            |range| report(range, 7),
        )
        .expect("rebuild cache");
        assert_eq!(rebuilt.freshness, ModelUsageFreshness::Fresh);
        assert_eq!(rebuilt.report.unassigned_tokens.total_tokens, 7);
        assert_eq!(
            rebuilt.cache_warning.as_deref(),
            Some("本地会话快照不可读，已重新汇总并重建缓存。")
        );
    }

    #[test]
    fn changing_codex_home_invalidates_otherwise_fresh_usage() {
        let temporary = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(temporary.path().join("state"));
        for (source, total) in [("first-root", 11), ("second-root", 29)] {
            let read = resolve_source(
                &state,
                request(ModelUsageRange::Today, false),
                source.to_string(),
                |range| Ok(report(range, total)),
            )
            .unwrap();
            assert_eq!(read.freshness, ModelUsageFreshness::Fresh);
            assert_eq!(read.report.unassigned_tokens.total_tokens, total);
        }
        let cached = resolve_source(
            &state,
            request(ModelUsageRange::Today, false),
            "second-root".to_string(),
            |_| panic!("same root should use cache"),
        )
        .unwrap();
        assert_eq!(cached.freshness, ModelUsageFreshness::Cached);
        assert_eq!(cached.report.unassigned_tokens.total_tokens, 29);
    }

    #[test]
    fn expired_cache_refreshes_and_failed_refresh_does_not_replace_the_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(directory.path().join("state"));
        resolve(&state, request(ModelUsageRange::Today, false), |range| report(range, 42)).unwrap();
        let mut cache = state.load_model_usage_cache().unwrap().unwrap();
        cache.entries[0].cached_at = (Utc::now() - Duration::hours(1)).to_rfc3339();
        state.save_model_usage_cache(&cache).unwrap();
        let failure = resolve_source(&state, request(ModelUsageRange::Today, false),
            "fixture-session-roots".into(), |_| Err("source unavailable".into()));
        assert!(failure.is_err());
        assert_eq!(state.load_model_usage_cache().unwrap().unwrap().entries[0].report.unassigned_tokens.total_tokens, 42);
        let next = resolve(&state, request(ModelUsageRange::Today, false), |range| report(range, 99)).unwrap();
        assert_eq!(next.freshness, ModelUsageFreshness::Fresh);
        assert_eq!(next.report.unassigned_tokens.total_tokens, 99);
    }

    #[test]
    fn refresh_deadline_never_reuses_a_snapshot_across_the_local_day_boundary() {
        use chrono::TimeZone;
        let before = Local.with_ymd_and_hms(2026, 10, 16, 23, 59, 30).single().unwrap();
        let after = DateTime::parse_from_rfc3339(&refresh_after(&before.to_rfc3339()).unwrap()).unwrap().with_timezone(&Local);
        assert_eq!(after.date_naive(), before.date_naive().succ_opt().unwrap());
        assert_eq!(after.time(), chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    }
}
