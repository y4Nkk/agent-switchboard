//! Typed commands exposed to the UI.
//!
//! The UI passes only profile ids, drafts, app kinds, and preview hashes. The
//! command modules resolve the two supported local configuration paths and
//! are the sole backend entry point for preview, switch, restore, and
//! discovery. Split by domain:
//!
//! - [`error`]: the typed error and shared command guards
//! - [`window`]: title-bar window controls and the inspector toggle
//! - [`status`]: read-only configuration status and lock observation
//! - [`common_settings`]: general-configuration overlay commands
//! - [`official_login`]: official client login start/poll/cancel commands
//! - [`prompt_management`]: global AGENTS.md / CLAUDE.md document commands
//! - [`switching`]: switch, backup, restore, and undo commands
//!
//! Profile store CRUD, application settings, probing, discovery, sessions,
//! and CC Switch import live here as thin delegations to their owner modules.
//!
//! Every command whose work touches files, the CC Switch database, or the
//! network runs through [`error::blocking`] so the main thread never stalls
//! behind I/O. Only the fast native window controls stay synchronous.

pub(crate) mod cloud_backup;
pub(crate) mod common_settings;
pub(crate) mod error;
pub(crate) mod model_usage;
pub(crate) mod official_login;
pub(crate) mod prompt_management;
pub(crate) mod runtime_log;
pub(crate) mod status;
pub(crate) mod switching;
pub(crate) mod usage_history;
pub(crate) mod window;

pub(crate) use status::config_status_report;
pub(crate) use status::ConfigFileStatus;
pub(crate) use window::apply_desktop_settings;

use crate::local_state::{AppSettings, LocalState};
use crate::runtime_log::RuntimeLogAction;
use asb_core::contracts::{AppKind, ProviderDraft, ProviderRecord, RouteMode};
use asb_core::discovery::{self, DiscoveryPaths, DiscoveryReport};
use error::{blocking, observe, state, store_error, CommandError};
use std::collections::BTreeMap;

/// Reads application-runtime settings. This is deliberately separate from the
/// Codex / Claude common configuration contract.
#[tauri::command]
pub async fn get_app_settings(app: tauri::AppHandle) -> Result<AppSettings, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        state
            .get_app_settings()
            .map_err(|error| CommandError::new("app-settings-unavailable", error))
    })
    .await
}

/// Updates application-runtime settings and returns the persisted value.
#[tauri::command]
pub async fn set_app_settings(
    app: tauri::AppHandle,
    settings: AppSettings,
) -> Result<AppSettings, CommandError> {
    let saved = observe(RuntimeLogAction::AppSettingsSaved, async move {
        let current_state = state(&app)?;
        settings
            .validate()
            .map_err(|error| CommandError::new("app-settings-invalid", error))?;
        let previous = blocking(move || {
            current_state
                .get_app_settings()
                .map_err(|error| CommandError::new("app-settings-unavailable", error))
        })
        .await?;
        if let Err(error) = apply_desktop_settings(&app, &settings) {
            let _ = apply_desktop_settings(&app, &previous);
            return Err(error);
        }
        let state = state(&app)?;
        let saved = blocking(move || {
            state
                .set_app_settings(&settings)
                .map_err(|error| CommandError::new("app-settings-save-failed", error))?;
            // The cache is not a second setting store: it applies the
            // just-persisted threshold to this result and later events.
            crate::runtime_log::set_level(settings.runtime_log_level);
            Ok(settings)
        })
        .await;
        if saved.is_err() {
            let _ = apply_desktop_settings(&app, &previous);
        }
        saved
    })
    .await?;
    Ok(saved)
}

/// One-click repair for an invalid app-settings file: replaces it with
/// validated defaults. Persisting runs before the live desktop settings are
/// applied so a repair is never lost to a window-state failure.
#[tauri::command]
pub async fn repair_app_settings(app: tauri::AppHandle) -> Result<AppSettings, CommandError> {
    let app_for_apply = app.clone();
    let repaired = observe(RuntimeLogAction::AppSettingsRepaired, async move {
        let state = state(&app)?;
        blocking(move || {
            state
                .repair_app_settings()
                .map_err(|error| CommandError::new("app-settings-repair-failed", error))
        })
        .await
    })
    .await?;
    apply_desktop_settings(&app_for_apply, &repaired)?;
    crate::runtime_log::set_level(repaired.runtime_log_level);
    Ok(repaired)
}

/// Installed system font families, offered by the interface-font picker.
#[tauri::command]
pub async fn list_system_fonts() -> Result<Vec<String>, CommandError> {
    blocking(|| Ok(crate::fonts::system_font_families())).await
}

#[tauri::command]
pub async fn list_profiles(app: tauri::AppHandle) -> Result<Vec<ProviderRecord>, CommandError> {
    let state = state(&app)?;
    blocking(move || state.configuration().list_providers().map_err(store_error)).await
}

#[tauri::command]
pub async fn reset_profile_store(
    app: tauri::AppHandle,
    confirm_write: bool,
) -> Result<(), CommandError> {
    let refresh_app = app.clone();
    let result = observe(RuntimeLogAction::ProfileStoreReset, async move {
        error::require_write_confirmation(confirm_write, "重置供应商数据")?;
        let state = state(&app)?;
        blocking(move || {
            state
                .configuration()
                .reset()
                .map_err(|error| CommandError::new("profile-store-reset-failed", error))
        })
        .await
    })
    .await;
    if result.is_ok() {
        if let Ok(state) = LocalState::from_app(&refresh_app) {
            if let Err(error) = crate::usage_cache::clear(&state) {
                log::warn!("无法清除托盘用量缓存: {error}");
            }
            if let Err(error) = crate::usage_history::clear_providers(&state) {
                log::warn!("无法清除供应商用量历史: {error}");
            }
        }
        crate::codex_official_quota::clear();
        crate::tray::refresh(&refresh_app);
    }
    result
}

#[tauri::command]
pub async fn create_profile(
    app: tauri::AppHandle,
    draft: ProviderDraft,
) -> Result<ProviderRecord, CommandError> {
    let refresh_app = app.clone();
    let result = observe(RuntimeLogAction::ProfileCreated, async move {
        let state = state(&app)?;
        blocking(move || {
            state
                .configuration()
                .create_provider(draft)
                .map_err(|error| CommandError::new("profile-create-failed", error))
        })
        .await
    })
    .await;
    if result.is_ok() {
        crate::tray::refresh(&refresh_app);
    }
    result
}

#[tauri::command]
pub async fn update_profile(
    app: tauri::AppHandle,
    profile_id: String,
    draft: ProviderDraft,
    expected_file_hash: String,
) -> Result<ProviderRecord, CommandError> {
    let refresh_app = app.clone();
    let cache_profile_id = profile_id.clone();
    let result = observe(RuntimeLogAction::ProfileUpdated, async move {
        let state = state(&app)?;
        blocking(move || {
            state
                .configuration()
                .update_provider(&profile_id, draft, &expected_file_hash)
                .map_err(|error| CommandError::new("profile-update-failed", error))
        })
        .await
    })
    .await;
    if result.is_ok() {
        if let Ok(state) = LocalState::from_app(&refresh_app) {
            if let Err(error) = crate::usage_cache::invalidate(&state, &cache_profile_id) {
                log::warn!("无法清除更新供应商的托盘用量缓存: {error}");
            }
            if let Err(error) = crate::usage_history::invalidate_provider(&state, &cache_profile_id)
            {
                log::warn!("无法清除更新供应商的用量历史: {error}");
            }
        }
        crate::codex_official_quota::invalidate(&cache_profile_id);
        crate::tray::refresh(&refresh_app);
    }
    result
}

#[tauri::command]
pub async fn delete_profile(
    app: tauri::AppHandle,
    profile_id: String,
    expected_file_hash: String,
) -> Result<(), CommandError> {
    let refresh_app = app.clone();
    let cache_profile_id = profile_id.clone();
    let result = observe(RuntimeLogAction::ProfileDeleted, async move {
        let state = state(&app)?;
        blocking(move || {
            state
                .configuration()
                .delete_provider(&profile_id, &expected_file_hash)
                .map_err(|error| CommandError::new("profile-delete-failed", error))
        })
        .await
    })
    .await;
    if result.is_ok() {
        if let Ok(state) = LocalState::from_app(&refresh_app) {
            if let Err(error) = crate::usage_cache::invalidate(&state, &cache_profile_id) {
                log::warn!("无法清除已删除供应商的托盘用量缓存: {error}");
            }
            if let Err(error) = crate::usage_history::invalidate_provider(&state, &cache_profile_id)
            {
                log::warn!("无法清除已删除供应商的用量历史: {error}");
            }
        }
        crate::codex_official_quota::invalidate(&cache_profile_id);
        crate::tray::refresh(&refresh_app);
    }
    result
}

#[tauri::command]
pub async fn reorder_profiles(
    app: tauri::AppHandle,
    target: AppKind,
    ordered_ids: Vec<String>,
    expected_file_hashes: BTreeMap<String, String>,
) -> Result<Vec<ProviderRecord>, CommandError> {
    let refresh_app = app.clone();
    let result = observe(RuntimeLogAction::ProfilesReordered, async move {
        let state = state(&app)?;
        blocking(move || {
            state
                .configuration()
                .reorder_providers(target, &ordered_ids, &expected_file_hashes)
                .map_err(|error| CommandError::new("profile-reorder-failed", error))
        })
        .await
    })
    .await;
    if result.is_ok() {
        crate::tray::refresh(&refresh_app);
    }
    result
}

#[tauri::command]
pub async fn import_discovered_profile(
    app: tauri::AppHandle,
    target: AppKind,
) -> Result<ProviderRecord, CommandError> {
    let refresh_app = app.clone();
    let result = observe(RuntimeLogAction::ProfileImported, async move {
        let state = state(&app)?;
        blocking(move || {
            let proposal = discovery_report()?
                .import_proposals
                .into_iter()
                .find(|candidate| candidate.app == target)
                .ok_or_else(|| {
                    CommandError::new("import-unavailable", "当前配置没有可导入的供应商")
                })?;
            state
                .configuration()
                .import_provider(proposal.draft)
                .map_err(|error| CommandError::new("profile-import-failed", error))
        })
        .await
    })
    .await;
    if result.is_ok() {
        // The imported configuration no longer matches the scanned snapshot.
        if let Ok(state) = LocalState::from_app(&refresh_app) {
            if let Err(error) = state.clear_discovery_cache() {
                log::warn!("无法清除导入后的发现扫描缓存: {error}");
            }
        }
        crate::tray::refresh(&refresh_app);
    }
    result
}

/// Result of one manual endpoint probe. It is informational only: nothing is
/// selected or switched automatically.
pub use crate::probe::ProbeResult;

#[tauri::command]
pub async fn probe_endpoint(url: String) -> Result<ProbeResult, CommandError> {
    blocking(move || {
        crate::probe::probe(&url).map_err(|error| CommandError::new("probe-failed", error))
    })
    .await
}

/// Models (id plus optional vendor) from the provider's OpenAI-compatible
/// `/v1/models` endpoint. The current editor draft supplies its API key. The
/// key travels only in request headers and is never included in errors or
/// persisted by this command.
#[tauri::command]
pub async fn fetch_provider_models(
    url: String,
    api_key: String,
) -> Result<Vec<crate::probe::ProviderModel>, CommandError> {
    blocking(move || {
        crate::probe::fetch_models(&url, &api_key)
            .map_err(|error| CommandError::new("models-fetch-failed", error))
    })
    .await
}

/// Numbers from one manual usage-balance query. The current editor draft
/// supplies the query and its credential; nothing is persisted by this
/// command and the credential never appears in errors or the summary.
pub use asb_core::contracts::{UsageQuery, UsageSummary};

#[tauri::command]
pub async fn test_usage_query(
    query: UsageQuery,
    api_key: String,
    base_url: Option<String>,
) -> Result<UsageSummary, CommandError> {
    blocking(move || {
        crate::usage_query::run_usage_query(&query, &api_key, base_url.as_deref())
            .map_err(|error| CommandError::new("usage-query-failed", error))
    })
    .await
}

/// Runs the persisted query of one provider and records the successful
/// credential-free summary for the native tray. The renderer passes only the
/// stable profile id; this backend boundary owns the query and API key.
#[tauri::command]
pub async fn query_profile_usage(
    app: tauri::AppHandle,
    profile_id: String,
) -> Result<UsageSummary, CommandError> {
    let state = state(&app)?;
    let summary = blocking(move || {
        let profile = state
            .configuration()
            .find_provider(&profile_id)
            .map_err(|error| CommandError::new("profile-not-found", error))?;
        let query = profile.usage_query.clone().ok_or_else(|| {
            CommandError::new("usage-query-not-configured", "该供应商尚未配置用量查询")
        })?;
        let summary = crate::usage_query::run_usage_query(
            &query,
            &profile.api_key,
            profile.base_url.as_deref(),
        )
        .map_err(|error| CommandError::new("usage-query-failed", error))?;
        if let Err(error) = crate::usage_cache::store(&state, &profile, summary.clone()) {
            log::warn!("用量查询成功，但无法保存托盘用量快照: {error}");
        }
        if let Err(error) = crate::usage_history::record_provider(&state, &profile, &summary) {
            log::warn!("用量查询成功，但无法保存趋势历史: {error}");
        }
        Ok(summary)
    })
    .await?;
    crate::tray::refresh(&app);
    Ok(summary)
}

/// Reads the native Codex ChatGPT-login quota for one official Codex profile.
/// This is intentionally a separate contract from provider usage scripts:
/// the renderer supplies only a stable profile id and receives no OAuth
/// credential, account identifier, endpoint, or raw upstream response.
#[tauri::command]
pub async fn query_codex_official_quota(
    app: tauri::AppHandle,
    profile_id: String,
) -> Result<asb_core::contracts::CodexOfficialQuota, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let profile = state
            .configuration()
            .find_provider(&profile_id)
            .map_err(|error| CommandError::new("profile-not-found", error))?;
        if profile.app != AppKind::Codex || profile.route_mode != RouteMode::Official {
            return Err(CommandError::new(
                "official-codex-quota-unavailable",
                "此档案不是 Codex 官方登录",
            ));
        }
        let auth_path = LocalState::codex_auth_path()
            .map_err(|error| CommandError::new("codex-auth-path-unavailable", error))?;
        let (mut quota, marker) = crate::codex_official_quota::query(&profile.id, &auth_path);
        if quota.status == asb_core::contracts::CodexOfficialQuotaStatus::Available {
            quota.last_reset = record_official_reset_read(&state, marker, &quota);
        }
        Ok(quota)
    })
    .await
}

/// Records one successful official read in the persisted detection baseline
/// and returns the latest locally detected reset. The baseline is a
/// regenerable derived cache: an unreadable or corrupt file self-heals on the
/// next save instead of hiding the fresh quota.
fn record_official_reset_read(
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

/// A typed overview snapshot and its display freshness. Both are
/// informational and global; they never read an account's quota or credentials.
pub use crate::codex_reset::CodexResetRead;

/// Reads the last successful public signal snapshot without contacting the
/// network. An empty cache is a normal first-run result.
#[tauri::command]
pub async fn get_cached_codex_reset_status(
    app: tauri::AppHandle,
) -> Result<Option<CodexResetRead>, CommandError> {
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
) -> Result<CodexResetRead, CommandError> {
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

/// Standard user-level configuration locations. This resolver does not read,
/// create, or write any target.
pub fn local_config_paths() -> Result<DiscoveryPaths, String> {
    Ok(DiscoveryPaths {
        codex: LocalState::user_config_path(AppKind::Codex)?
            .to_string_lossy()
            .to_string(),
        codex_auth: LocalState::codex_auth_path()?.to_string_lossy().to_string(),
        claude: LocalState::user_config_path(AppKind::Claude)?
            .to_string_lossy()
            .to_string(),
    })
}

fn discovery_report() -> Result<DiscoveryReport, CommandError> {
    let paths = local_config_paths()
        .map_err(|error| CommandError::new("config-path-unavailable", error))?;
    let read = |path: &str| match std::fs::read_to_string(path) {
        Ok(text) if std::path::Path::new(path).is_file() => Ok(Some(text)),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("无法读取配置文件".to_string()),
    };
    Ok(discovery::discover(&paths, read))
}

/// Read-only discovery of local Codex and Claude Code configuration. Reads at
/// most three files; never writes, creates, or locks any target. A successful
/// scan replaces the local display cache; a cache-write failure is logged and
/// never hides the fresh result.
#[tauri::command]
pub async fn discover_local(app: tauri::AppHandle) -> Result<DiscoveryReport, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let report = discovery_report()?;
        if let Err(error) = state.save_discovery_cache(&report) {
            log::warn!("无法保存发现扫描缓存: {error}");
        }
        Ok(report)
    })
    .await
}

/// The previous successful scan, shown before the next one runs. Null before
/// the first scan ever completed.
#[tauri::command]
pub async fn discover_cached(
    app: tauri::AppHandle,
) -> Result<Option<DiscoveryReport>, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        state
            .load_discovery_cache()
            .map_err(|error| CommandError::new("discovery-cache-unavailable", error))
    })
    .await
}

/// Read-only scan of the local Codex and Claude Code JSONL session stores.
/// The session manager never receives a source path from the renderer, so it
/// cannot be redirected to unrelated user files.
#[tauri::command]
pub async fn list_sessions() -> Result<crate::session_manager::SessionScan, CommandError> {
    blocking(move || Ok(crate::session_manager::scan_sessions())).await
}

/// Loads one transcript after resolving its source from the approved session
/// roots. No session file is created, modified, or deleted by this command.
#[tauri::command]
pub async fn get_session_messages(
    app: AppKind,
    session_id: String,
) -> Result<Vec<crate::session_manager::SessionMessage>, CommandError> {
    blocking(move || {
        crate::session_manager::load_messages(app, &session_id)
            .map_err(|message| CommandError::new("session-unavailable", message))
    })
    .await
}

/// Starts the selected session in a new Windows Command Prompt window. The
/// session source is resolved by the backend; the renderer never submits a
/// path or executable command.
#[tauri::command]
pub async fn resume_session(
    app: AppKind,
    session_id: String,
) -> Result<crate::session_manager::SessionResume, CommandError> {
    observe(RuntimeLogAction::SessionResumed, async move {
        blocking(move || {
            crate::session_manager::resume_session(app, &session_id)
                .map_err(|message| CommandError::new("session-resume-failed", message))
        })
        .await
    })
    .await
}

/// Read-only scan of the local CC Switch database. Secrets never cross this
/// boundary: the returned items carry routing facts only.
#[tauri::command]
pub async fn scan_ccswitch(
    app: tauri::AppHandle,
) -> Result<crate::ccswitch_source::CcSwitchScan, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        crate::ccswitch_source::scan(&state)
            .map_err(|error| CommandError::new("ccswitch-unavailable", error))
    })
    .await
}

/// Imports selected CC Switch providers into the app's own profile store.
/// Never touches real Codex or Claude Code configuration files.
#[tauri::command]
pub async fn import_ccswitch_profiles(
    app: tauri::AppHandle,
    keys: Vec<String>,
) -> Result<crate::ccswitch_source::CcSwitchImportOutcome, CommandError> {
    observe(RuntimeLogAction::CcSwitchProfilesImported, async move {
        let state = state(&app)?;
        blocking(move || {
            crate::ccswitch_source::import(&state, &keys)
                .map_err(|error| CommandError::new("ccswitch-import-failed", error))
        })
        .await
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::{
        CodexOfficialQuota, CodexOfficialQuotaStatus, CodexOfficialQuotaWindow,
    };
    use tempfile::tempdir;

    fn quota(at: &str, used_percent: f64) -> CodexOfficialQuota {
        CodexOfficialQuota {
            status: CodexOfficialQuotaStatus::Available,
            windows: vec![CodexOfficialQuotaWindow {
                label: "7 天".to_string(),
                used_percent,
                resets_at: None,
            }],
            at: Some(at.to_string()),
            stale: false,
            last_reset: None,
        }
    }

    #[test]
    fn missing_quota_baseline_replaces_the_existing_official_trend() {
        let directory = tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        crate::usage_history::record_official(&state, &quota("2026-09-03T08:00:00Z", 20.0), false)
            .expect("record old account trend");

        record_official_reset_read(
            &state,
            Some("new-account-marker".to_string()),
            &quota("2026-09-03T09:00:00Z", 40.0),
        );

        let series = crate::usage_history::official_series(&state).expect("official trend");
        assert_eq!(series.len(), 1);
        assert_eq!(series[0].points.len(), 1);
        assert_eq!(series[0].points[0].value, 40.0);
    }
}
