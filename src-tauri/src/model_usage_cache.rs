//! Persisted, credential-free snapshots of local session token totals.
//!
//! This cache owns the one refresh lifetime for the model-usage page. It is
//! deliberately separate from provider and official-quota history: its values
//! come only from local Codex and Claude Code session records.

use crate::local_state::LocalState;
use asb_core::contracts::{
    ModelUsageFreshness, ModelUsageRange, ModelUsageRead, ModelUsageReport, ModelUsageRequest,
};
use chrono::{DateTime, Duration, Utc};

pub(crate) const MODEL_USAGE_REFRESH_INTERVAL: Duration = Duration::minutes(5);

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ModelUsageCache {
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
    resolve(state, request, crate::model_usage::scan_model_usage_report)
}

fn resolve<F>(
    state: &LocalState,
    request: ModelUsageRequest,
    scan: F,
) -> Result<ModelUsageRead, String>
where
    F: FnOnce(ModelUsageRange) -> ModelUsageReport,
{
    let (mut cache, mut warnings) = match state.load_model_usage_cache() {
        Ok(Some(cache)) if cache.validate().is_ok() => (cache, Vec::new()),
        Ok(None) => (ModelUsageCache::default(), Vec::new()),
        Ok(Some(_)) | Err(_) => (
            ModelUsageCache::default(),
            vec!["本地会话快照不可读，已重新汇总并重建缓存。".to_string()],
        ),
    };

    if !request.force_refresh {
        if let Some(cached) = cache.cached(request.range) {
            return cached_read(cached);
        }
    }

    let report = scan(request.range);
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
    cached_at
        .checked_add_signed(MODEL_USAGE_REFRESH_INTERVAL)
        .map(|timestamp| timestamp.to_rfc3339())
        .ok_or_else(|| "本地会话快照格式无效".to_string())
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
            issues: Vec::new(),
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
}
