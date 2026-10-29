use super::error::{blocking, state, CommandError};
use crate::codex_reset::{CodexResetHubRead, CodexResetRead, CodexResetStatus};
use crate::local_state::LocalState;

/// Refreshes the account-bound quota cache for one official Codex profile.
/// The renderer supplies only a stable id and re-reads the shared cache;
/// OAuth credentials and raw upstream responses stay behind this boundary.
#[tauri::command]
pub async fn query_codex_official_quota(
    app: tauri::AppHandle,
    profile_id: String,
) -> Result<(), CommandError> {
    let state = state(&app)?;
    let result = blocking(move || super::quota_refresh::refresh(&state, &profile_id, true).map(|_| ())).await;
    crate::tray::refresh(&app);
    result
}

#[tauri::command]
pub async fn read_codex_official_quota(
    app: tauri::AppHandle,
    profile_id: String,
) -> Result<Option<asb_core::contracts::CodexOfficialQuota>, CommandError> {
    let state = state(&app)?;
    blocking(move || super::quota_refresh::read(&state, &profile_id)).await
}

/// Records one successful official read in the persisted detection baseline
/// and returns the latest locally detected reset. The baseline is a
/// regenerable derived cache: an unreadable or corrupt file self-heals on the
/// next save instead of hiding the fresh quota.
pub(super) fn record_official_reset_read(
    state: &LocalState,
    marker: Option<String>,
    quota: &asb_core::contracts::CodexOfficialQuota,
) -> Option<asb_core::contracts::CodexOfficialQuotaReset> {
    let baseline = state.load_codex_quota_baseline().unwrap_or_else(|error| {
        log::warn!("Codex 官方额度基线不可读，将从本次读取重建: {error}");
        None
    });
    // The history ledger deliberately has no account marker. It may continue
    // only when the persisted baseline proves this is the same account.
    let reset_history = !baseline
        .as_ref()
        .and_then(|previous| previous.account_marker.as_deref().zip(marker.as_deref()))
        .is_some_and(|(previous, current)| previous == current);
    let at = quota.at.clone().unwrap_or_default();
    let (baseline, _) = crate::codex_official_quota::apply_read(baseline, marker, quota, &at);
    if let Err(error) = state.save_codex_quota_baseline(&baseline) {
        log::warn!("官方额度读取成功，但无法保存重置检测基线: {error}");
    }
    if let Err(error) = crate::usage_history::record_official(state, quota, reset_history) {
        log::warn!("官方额度读取成功，但无法保存趋势历史: {error}");
    }
    baseline.last_reset
}

/// Reads the persisted baseline's last successful official read without
/// contacting the network. An absent baseline is a normal first-run result.
#[tauri::command]
pub async fn get_cached_codex_official_reset(
    app: tauri::AppHandle,
) -> Result<Option<asb_core::contracts::CodexOfficialQuota>, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let baseline = state
            .load_codex_quota_baseline()
            .map_err(|error| CommandError::new("codex-official-reset-cache-unavailable", error))?;
        Ok(baseline.and_then(|baseline| {
            baseline
                .last_read
                .map(|read| asb_core::contracts::CodexOfficialQuota {
                    status: asb_core::contracts::CodexOfficialQuotaStatus::Available,
                    windows: read.windows,
                    at: Some(read.at),
                    stale: false,
                    last_reset: baseline.last_reset,
                    failure: None,
                })
        }))
    })
    .await
}

/// One explicit read of the machine's Codex official login for the overview.
/// The result is account-scoped and profile-independent; failed reads are
/// returned as statuses for the panel to render.
#[tauri::command]
pub async fn refresh_codex_official_reset(
    app: tauri::AppHandle,
) -> Result<asb_core::contracts::CodexOfficialQuota, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let auth_path = LocalState::codex_auth_path().map_err(|error| {
            CommandError::new("codex-official-reset-refresh-unavailable", error)
        })?;
        let (mut quota, marker) = crate::codex_official_quota::query_login(&auth_path);
        if quota.status == asb_core::contracts::CodexOfficialQuotaStatus::Available {
            quota.last_reset = record_official_reset_read(&state, marker, &quota);
        }
        Ok(quota)
    })
    .await
}

/// Reads the last successful public signal snapshot without contacting the
/// network. An empty cache is a normal first-run result.
#[tauri::command]
pub async fn get_cached_codex_reset_status(
    app: tauri::AppHandle,
) -> Result<Option<CodexResetRead<CodexResetStatus>>, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        state
            .load_codex_reset_cache()
            .map(|status| status.map(CodexResetRead::cached))
            .map_err(|error| CommandError::new("codex-reset-cache-unavailable", error))
    })
    .await
}

/// One explicit read of the public reset-status feed. A successful read
/// replaces the local snapshot; a cache-write failure does not hide the fresh
/// informational result from the overview.
#[tauri::command]
pub async fn check_codex_reset_status(
    app: tauri::AppHandle,
) -> Result<CodexResetRead<CodexResetStatus>, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let status = crate::codex_reset::check()
            .map_err(|error| CommandError::new("codex-reset-status-unavailable", error))?;
        let cache_warning = state.save_codex_reset_cache(&status).err().map(|_| {
            "最新公开信号已显示，但未能写入本地缓存；下次打开应用可能无法保留该结果。".to_string()
        });
        Ok(CodexResetRead::live(status, cache_warning))
    })
    .await
}

/// Reads the last successful aggregate snapshot from the second monitor
/// without contacting the network. An empty cache is a normal first-run
/// result.
#[tauri::command]
pub async fn get_cached_codex_reset_hub_status(
    app: tauri::AppHandle,
) -> Result<Option<CodexResetHubRead>, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        state
            .load_codex_reset_hub_cache()
            .map(|status| status.map(CodexResetHubRead::cached))
            .map_err(|error| CommandError::new("codex-reset-hub-cache-unavailable", error))
    })
    .await
}

/// One explicit read of the second monitor's four endpoints. A successful
/// read replaces the local snapshot; a cache-write failure does not hide the
/// fresh informational result from the overview.
#[tauri::command]
pub async fn check_codex_reset_hub_status(
    app: tauri::AppHandle,
) -> Result<CodexResetHubRead, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let status = crate::codex_reset::check_hub()
            .map_err(|error| CommandError::new("codex-reset-hub-status-unavailable", error))?;
        let cache_warning = state.save_codex_reset_hub_cache(&status).err().map(|_| {
            "最新公开信号已显示，但未能写入本地缓存；下次打开应用可能无法保留该结果。".to_string()
        });
        Ok(CodexResetHubRead::live(status, cache_warning))
    })
    .await
}
