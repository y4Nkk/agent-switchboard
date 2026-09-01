//! In-memory usage snapshots shared by provider cards and the native tray.
//!
//! A snapshot never stores credentials and is deliberately not persisted:
//! the tray may present the latest successful reading without opening the
//! main window or triggering a network request.

use asb_core::contracts::{ProviderProfile, UsageQuery, UsageSummary};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Clone)]
struct CachedUsage {
    query: UsageQuery,
    summary: UsageSummary,
}

static USAGE: OnceLock<Mutex<HashMap<String, CachedUsage>>> = OnceLock::new();

fn entries() -> &'static Mutex<HashMap<String, CachedUsage>> {
    USAGE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Replaces the snapshot for one profile after a successful real query.
pub(crate) fn store(profile: &ProviderProfile, summary: UsageSummary) {
    let Some(query) = profile.usage_query.clone() else {
        return;
    };
    if let Ok(mut entries) = entries().lock() {
        entries.insert(profile.id.clone(), CachedUsage { query, summary });
    }
}

/// Returns a snapshot only when it belongs to the profile's current query.
/// Editing or removing a query therefore cannot expose an old reading.
pub(crate) fn get(profile: &ProviderProfile) -> Option<UsageSummary> {
    let query = profile.usage_query.as_ref()?;
    entries()
        .lock()
        .ok()?
        .get(&profile.id)
        .filter(|cached| &cached.query == query)
        .map(|cached| cached.summary.clone())
}

/// Removes a snapshot after its profile was changed or deleted.
pub(crate) fn invalidate(profile_id: &str) {
    if let Ok(mut entries) = entries().lock() {
        entries.remove(profile_id);
    }
}

/// Drops all snapshots when the application-owned profile store is reset.
pub(crate) fn clear() {
    if let Ok(mut entries) = entries().lock() {
        entries.clear();
    }
}
