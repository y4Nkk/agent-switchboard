//! Credential-free usage snapshots for the native tray.
//!
//! A successful provider query replaces its prior snapshot atomically. The
//! snapshot stores only the normalized readings and a digest of the profile's
//! current query, never a credential, endpoint, raw response, or script.

use crate::local_state::LocalState;
use asb_core::contracts::{ProviderProfile, UsageQuery, UsageSummary};
use asb_switch::sha256_hex;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct UsageCache {
    entries: BTreeMap<String, CachedUsage>,
}

impl Default for UsageCache {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CachedUsage {
    query_digest: String,
    summary: UsageSummary,
}

/// The stable digest for every persisted representation of a usage query.
/// It is intentionally one-way because the source can mention private URLs
/// or contain script text that must not enter display-oriented state.
pub(crate) fn query_digest(query: &UsageQuery) -> Result<String, String> {
    let serialized =
        serde_json::to_string(query).map_err(|_| "用量查询摘要序列化失败".to_string())?;
    Ok(sha256_hex(&serialized))
}

/// Replaces the snapshot for one profile after a successful real query.
pub(crate) fn store(
    state: &LocalState,
    profile: &ProviderProfile,
    summary: UsageSummary,
) -> Result<(), String> {
    let query = profile
        .usage_query
        .as_ref()
        .ok_or_else(|| "该供应商尚未配置用量查询".to_string())?;
    let mut cache = state.load_usage_cache()?.unwrap_or_default();
    cache.entries.insert(
        profile.id.clone(),
        CachedUsage {
            query_digest: query_digest(query)?,
            summary,
        },
    );
    state.save_usage_cache(&cache)
}

/// Returns the last successful reading only while it belongs to the
/// profile's current query. A profile edit never displays a stale result.
pub(crate) fn get(state: &LocalState, profile: &ProviderProfile) -> Option<UsageSummary> {
    let query = profile.usage_query.as_ref()?;
    let cache = state.load_usage_cache().ok()??;
    let digest = query_digest(query).ok()?;
    cache
        .entries
        .get(&profile.id)
        .filter(|cached| cached.query_digest == digest)
        .map(|cached| cached.summary.clone())
}

/// Removes a snapshot after its profile was changed or deleted.
pub(crate) fn invalidate(state: &LocalState, profile_id: &str) -> Result<(), String> {
    let Some(mut cache) = state.load_usage_cache()? else {
        return Ok(());
    };
    if cache.entries.remove(profile_id).is_some() {
        state.save_usage_cache(&cache)?;
    }
    Ok(())
}

/// Drops all snapshots when the application-owned profile store is reset.
pub(crate) fn clear(state: &LocalState) -> Result<(), String> {
    state.clear_usage_cache()
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::{AppKind, ProviderDraft, RouteMode, UsageReading};
    use std::fs;

    fn profile() -> ProviderProfile {
        ProviderProfile::from_draft(
            "profile-1".to_string(),
            ProviderDraft {
                app: AppKind::Claude,
                route_mode: RouteMode::Custom,
                name: "查询供应商".to_string(),
                model: None,
                base_url: Some("https://relay.example".to_string()),
                api_key: "test-api-key".to_string(),
                model_options: None,
                notes: None,
                website_url: None,
                usage_query: Some(UsageQuery::Declarative {
                    url: "{{baseUrl}}/usage".to_string(),
                    remaining_path: Some("/remaining".to_string()),
                    used_path: None,
                    total_path: None,
                    unit: Some("次".to_string()),
                    refresh_interval_minutes: 0,
                }),
            },
        )
    }

    fn summary() -> UsageSummary {
        UsageSummary {
            readings: vec![UsageReading {
                plan_name: Some("额度".to_string()),
                remaining: Some(92.0),
                used: Some(8.0),
                total: Some(100.0),
                unit: Some("%".to_string()),
            }],
            at: "2026-09-02T02:34:00Z".to_string(),
        }
    }

    #[test]
    fn last_successful_summary_survives_a_state_reopen() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("state");
        let profile = profile();
        let expected = summary();

        store(
            &LocalState::from_root(root.clone()),
            &profile,
            expected.clone(),
        )
        .expect("store summary");

        let persisted = fs::read_to_string(root.join("usage-cache.json")).expect("cache text");
        assert!(!persisted.contains("test-api-key"));
        assert!(!persisted.contains("relay.example"));
        assert!(!persisted.contains("{{baseUrl}}/usage"));

        assert_eq!(get(&LocalState::from_root(root), &profile), Some(expected));
    }

    #[test]
    fn a_changed_query_hides_its_prior_summary() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let profile = profile();
        store(&state, &profile, summary()).expect("store summary");
        let mut changed = profile.clone();
        changed.usage_query = Some(UsageQuery::Declarative {
            url: "{{baseUrl}}/new-usage".to_string(),
            remaining_path: Some("/remaining".to_string()),
            used_path: None,
            total_path: None,
            unit: Some("次".to_string()),
            refresh_interval_minutes: 0,
        });

        assert_eq!(get(&state, &changed), None);
    }

    #[test]
    fn clearing_the_profile_store_cache_removes_the_snapshot_file() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("state");
        let state = LocalState::from_root(root.clone());
        let profile = profile();
        store(&state, &profile, summary()).expect("store summary");

        clear(&state).expect("clear cache");

        assert!(!root.join("usage-cache.json").exists());
        assert_eq!(get(&state, &profile), None);
    }
}
