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
pub(crate) mod prompt_management;
pub(crate) mod runtime_log;
pub(crate) mod status;
pub(crate) mod switching;
pub(crate) mod window;

pub(crate) use status::config_status_report;
pub(crate) use status::ConfigFileStatus;
pub(crate) use window::apply_desktop_settings;

use crate::local_state::{AppSettings, LocalState};
use crate::runtime_log::RuntimeLogAction;
use asb_core::contracts::{AppKind, ProviderDraft, ProviderRecord};
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
        crate::usage_cache::clear();
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
        crate::usage_cache::invalidate(&cache_profile_id);
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
        crate::usage_cache::invalidate(&cache_profile_id);
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

/// Model ids from the provider's OpenAI-compatible `/v1/models` endpoint.
/// The current editor draft supplies its API key. The key travels only in
/// request headers and is never included in errors or persisted by this
/// command.
#[tauri::command]
pub async fn fetch_provider_models(
    url: String,
    api_key: String,
) -> Result<Vec<String>, CommandError> {
    blocking(move || {
        crate::probe::fetch_models(&url, &api_key, "当前供应商档案")
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
        crate::usage_cache::store(&profile, summary.clone());
        Ok(summary)
    })
    .await?;
    crate::tray::refresh(&app);
    Ok(summary)
}

/// Result of one manual update check against the project's GitHub releases.
/// Informational only: nothing is downloaded or installed by this command.
pub use crate::update::UpdateCheck;

#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<UpdateCheck, CommandError> {
    let current_version = app.package_info().version.to_string();
    blocking(move || {
        crate::update::check(&current_version)
            .map_err(|error| CommandError::new("update-check-failed", error))
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
/// create, or write either path.
pub fn local_config_paths() -> Result<DiscoveryPaths, String> {
    Ok(DiscoveryPaths {
        codex: LocalState::user_config_path(AppKind::Codex)?
            .to_string_lossy()
            .to_string(),
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
/// most two files; never writes, creates, or locks either target.
#[tauri::command]
pub async fn discover_local() -> Result<DiscoveryReport, CommandError> {
    blocking(discovery_report).await
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
