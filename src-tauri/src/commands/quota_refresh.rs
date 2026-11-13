//! One execution and timing owner for scheduled and manual official quota reads.

use super::error::{CommandError, operation_error};
use crate::config_store::providers::load_provider_files;
use crate::local_state::LocalState;
use asb_core::contracts::{AppKind, CodexOfficialQuota, CodexOfficialQuotaStatus, ProviderProfile, RouteMode};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

// The official profile list is small. Holding this gate across the request
// serializes manual/scheduled writes and makes the due check atomic with it.
static ATTEMPTS: OnceLock<Mutex<HashMap<(PathBuf, String), Instant>>> = OnceLock::new();

fn profile(state: &LocalState, id: &str) -> Result<ProviderProfile, CommandError> {
    let file = load_provider_files(&state.configuration(), AppKind::Codex)
        .map_err(|error| operation_error("profile-not-found", error.into()))?
        .into_iter().find(|file| file.id == id)
        .ok_or_else(|| {
            CommandError::keyed("profile-not-found", "errors.misc.providerNotFound", "供应商不存在")
        })?;
    let profile = file.into_profile(AppKind::Codex);
    if profile.route_mode != RouteMode::Official {
        return Err(CommandError::keyed(
            "official-codex-quota-unavailable",
            "errors.misc.profileNotCodexOfficial",
            "此档案不是 Codex 官方登录",
        ));
    }
    Ok(profile)
}

pub(crate) fn read(state: &LocalState, profile_id: &str) -> Result<Option<CodexOfficialQuota>, CommandError> {
    profile(state, profile_id)?;
    let auth_path = LocalState::codex_auth_path()
        .map_err(|error| CommandError::new("codex-auth-path-unavailable", error))?;
    crate::codex_auth::cached_profile_quota(state.root(), profile_id, &auth_path)
        .map_err(|error| CommandError::new("codex-account-quota-unavailable", error))
}

pub(crate) fn refresh(state: &LocalState, id: &str, force: bool,
    gate: &super::ConfigWriteGate) -> Result<bool, CommandError> {
    let mut attempts = ATTEMPTS.get_or_init(|| Mutex::new(HashMap::new())).lock()
        .map_err(|_| {
            CommandError::keyed(
                "official-quota-busy",
                "errors.misc.quotaLockUnavailable",
                "官方额度查询锁不可用",
            )
        })?;
    let profile = {
        let _guard = gate.lock().map_err(|error| CommandError::new("config-write-gate-unavailable", error))?;
        profile(state, id)?
    };
    let interval = profile.official_quota_refresh_interval_minutes.unwrap_or(0);
    let key = (state.root().to_path_buf(), id.to_string());
    if !force && (interval == 0 || attempts.get(&key).is_some_and(|at|
        at.elapsed() < Duration::from_secs(u64::from(interval) * 60))) {
        return Ok(false);
    }
    attempts.insert(key, Instant::now());
    query(state, &profile)?;
    Ok(true)
}

fn query(state: &LocalState, profile: &ProviderProfile) -> Result<(), CommandError> {
    let auth_path = LocalState::codex_auth_path()
        .map_err(|error| CommandError::new("codex-auth-path-unavailable", error))?;
    let selection = crate::codex_auth::binding(state.root(), &profile.id)
        .map_err(|error| CommandError::new("codex-account-binding-invalid", error))?;
    if selection != crate::codex_auth::contracts::AccountSelection::Native {
        let id = match &selection {
            crate::codex_auth::contracts::AccountSelection::Account { id } => Some(id.as_str()),
            _ => None,
        };
        let managed = crate::codex_auth::quota(state.root(), id, &auth_path)
            .map_err(|error| CommandError::new("codex-account-quota-unavailable", error))?;
        if let Some(warning) = managed.warning { log::warn!("{warning}"); }
        return Ok(());
    }
    let (mut quota, marker) = crate::codex_official_quota::query(&profile.id, &auth_path);
    if quota.status == CodexOfficialQuotaStatus::Available {
        quota.last_reset = super::quota::record_official_reset_read(state, marker.clone(), &quota);
        crate::codex_official_quota::store_result(&profile.id, marker.as_deref(), &quota);
    }
    Ok(())
}
