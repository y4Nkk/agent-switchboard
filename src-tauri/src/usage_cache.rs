//! Credential-free usage snapshots for the custom tray panel.
//!
//! The desktop scheduler owns when a profile is re-queried; this cache is the
//! single persisted record of that timing. A successful provider query
//! replaces its prior snapshot atomically. The snapshot stores only the
//! normalized readings, a scrubbed failure, the query digest and last attempt
//! time. Credentials and raw responses are never stored.

use crate::local_state::LocalState;
use asb_core::contracts::{ProviderProfile, UsageQuery, UsageSummary, UsageSnapshot};
use asb_switch::sha256_hex;
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// Serializes every read-modify-write of the cache file so the scheduler
/// thread and a manual query can never lose each other's entries.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

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
    /// RFC 3339 UTC timestamp of when the most recent attempt was initiated.
    /// Failures advance it too, so a failing endpoint is retried on its own
    /// configured cadence instead of every scheduler tick. The single
    /// executor stamps the moment before the network call, so a slow
    /// response never lengthens the profile's cadence.
    attempted_at: String,
    /// The last completion and the retained successful reading for this query.
    snapshot: UsageSnapshot,
}

fn rfc3339(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
}

/// The stable digest for every persisted representation of a usage query.
/// It is intentionally one-way because the source can mention private URLs
/// or contain script text that must not enter display-oriented state.
pub(crate) fn query_digest(query: &UsageQuery) -> Result<String, String> {
    let serialized =
        serde_json::to_string(query).map_err(|_| "用量查询摘要序列化失败".to_string())?;
    Ok(sha256_hex(&serialized))
}

/// The same cache projection is read by provider rows and the tray.
pub(crate) fn get(state: &LocalState, profile: &ProviderProfile) -> Result<Option<UsageSnapshot>, String> {
    let Some(query) = profile.usage_query.as_ref() else { return Ok(None); };
    let Some(cache) = state.load_usage_cache()? else { return Ok(None); };
    cache.snapshot(&profile.id, Some(query))
}

impl UsageCache {
    pub(crate) fn snapshot(&self, id: &str, query: Option<&UsageQuery>) -> Result<Option<UsageSnapshot>, String> {
        let Some(query) = query else { return Ok(None); };
        let digest = query_digest(query)?;
        Ok(self.entries.get(id).filter(|cached| cached.query_digest == digest)
            .map(|cached| cached.snapshot.clone()))
    }
}

/// Replaces the snapshot for one profile, stamping the supplied attempt
/// initiation time after a successful real query. Returns false when a newer
/// attempt already owns the cache; callers must not append obsolete history.
pub(crate) fn record_success(
    state: &LocalState,
    profile: &ProviderProfile,
    summary: UsageSummary,
    attempted_at: DateTime<Utc>,
) -> Result<bool, String> {
    let query = profile
        .usage_query
        .as_ref()
        .ok_or_else(|| "该供应商尚未配置用量查询".to_string())?;
    let digest = query_digest(query)?;
    let _guard = WRITE_LOCK.lock().map_err(|_| "用量缓存写入锁不可用")?;
    let mut cache = state.load_usage_cache()?.unwrap_or_default();
    if superseded(&cache, &profile.id, attempted_at) {
        return Ok(false);
    }
    cache.entries.insert(
        profile.id.clone(),
        CachedUsage {
            query_digest: digest,
            attempted_at: rfc3339(attempted_at),
            snapshot: UsageSnapshot { summary: Some(summary), error: None },
        },
    );
    state.save_usage_cache(&cache)?;
    Ok(true)
}

/// Stamps the supplied attempt initiation time after a failed query, keeping
/// the last successful summary only while it still belongs to the profile's
/// current query.
pub(crate) fn record_failure(
    state: &LocalState,
    profile: &ProviderProfile,
    attempted_at: DateTime<Utc>,
    error: &str,
) -> Result<(), String> {
    let Some(query) = profile.usage_query.as_ref() else {
        return Ok(());
    };
    let digest = query_digest(query)?;
    let _guard = WRITE_LOCK.lock().map_err(|_| "用量缓存写入锁不可用")?;
    let mut cache = state.load_usage_cache()?.unwrap_or_default();
    if superseded(&cache, &profile.id, attempted_at) {
        return Ok(());
    }
    let retained = cache
        .entries
        .get(&profile.id)
        .filter(|cached| cached.query_digest == digest)
        .and_then(|cached| cached.snapshot.summary.clone());
    cache.entries.insert(
        profile.id.clone(),
        CachedUsage {
            query_digest: digest,
            attempted_at: rfc3339(attempted_at),
            snapshot: UsageSnapshot {
                summary: retained,
                error: Some(asb_core::adapter::scrub_message(
                    if profile.api_key.is_empty() { error.to_string() }
                    else { error.replace(&profile.api_key, "[REDACTED]") }
                )),
            },
        },
    );
    state.save_usage_cache(&cache)
}

fn superseded(cache: &UsageCache, profile_id: &str, attempted_at: DateTime<Utc>) -> bool {
    cache.entries.get(profile_id)
        .and_then(|entry| DateTime::parse_from_rfc3339(&entry.attempted_at).ok())
        .is_some_and(|previous| previous > attempted_at)
}

/// Whether the scheduler should query this profile now: an absent or
/// unparsable attempt time, one recorded for a different query, or one older
/// than the configured interval is due.
pub(crate) fn due(
    state: &LocalState,
    profile: &ProviderProfile,
    interval_minutes: u32,
    now: DateTime<Utc>,
) -> bool {
    let Some(query) = profile.usage_query.as_ref() else {
        return false;
    };
    let digest = query_digest(query).ok();
    let cache = state.load_usage_cache().ok().flatten();
    let attempted_at = cache
        .as_ref()
        .and_then(|cache| cache.entries.get(&profile.id))
        .filter(|cached| Some(&cached.query_digest) == digest.as_ref())
        .map(|cached| cached.attempted_at.as_str());
    attempt_is_due(attempted_at, interval_minutes, now)
}

/// The pure timing rule behind `due`, separated for direct testing.
pub(crate) fn attempt_is_due(
    attempted_at: Option<&str>,
    interval_minutes: u32,
    now: DateTime<Utc>,
) -> bool {
    let Some(attempted_at) = attempted_at
        .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
        .map(|at| at.with_timezone(&Utc))
    else {
        return true;
    };
    now.signed_duration_since(attempted_at).num_seconds() >= i64::from(interval_minutes) * 60
}

/// Removes the snapshot after its profile was changed or deleted.
pub(crate) fn invalidate(state: &LocalState, profile_id: &str) -> Result<(), String> {
    let _guard = WRITE_LOCK.lock().map_err(|_| "用量缓存写入锁不可用")?;
    let Some(mut cache) = state.load_usage_cache()? else {
        return Ok(());
    };
    if cache.entries.remove(profile_id).is_some() {
        state.save_usage_cache(&cache)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
