//! Typed commands exposed to the UI.
//!
//! The UI passes only profile ids, drafts, app kinds, and preview hashes. This
//! module resolves the two supported local configuration paths and is the
//! sole backend entry point for preview, switch, restore, and discovery.

use crate::local_state::LocalState;
use asb_core::contracts::{
    AppKind, BackupRecord, CommonConfigPatch, KeyChange, MatchStatus, ProviderDraft,
    ProviderProfile, RouteState, SwitchLog, SwitchPlan,
};
use asb_core::discovery::{self, DiscoveryPaths, DiscoveryReport};
use asb_core::{adapter, LockStatus};
use asb_switch::io::{FsIo, SwitchIo};
use asb_switch::lockfile::{self, RecoveryEntry};
use asb_switch::{
    execute, list_backups as scan_backups, read_preview, restore, sha256_hex, FilePreview,
    RestoreOutcome, SwitchOutcome,
};
use serde::Serialize;
use std::io::ErrorKind;
use std::path::PathBuf;
use tauri::AppHandle;

/// Structured command error surfaced to the UI as a typed object.
#[derive(Debug, Clone, Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl CommandError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: adapter::scrub_message(message.into()),
        }
    }
}

impl From<asb_switch::SwitchError> for CommandError {
    fn from(error: asb_switch::SwitchError) -> Self {
        let code = match &error {
            asb_switch::SwitchError::ReadCurrent { .. } => "read-current",
            asb_switch::SwitchError::PlanRejected { .. } => "plan-rejected",
            asb_switch::SwitchError::BlockedByLock { .. } => "blocked-by-lock",
            asb_switch::SwitchError::ExternalChange { .. } => "external-change",
            asb_switch::SwitchError::PlanChanged => "preview-stale",
            asb_switch::SwitchError::CommitFailed { .. } => "commit-failed",
        };
        CommandError::new(code, error.to_string())
    }
}

fn state(app: &AppHandle) -> Result<LocalState, CommandError> {
    LocalState::from_app(app).map_err(|error| CommandError::new("app-state-unavailable", error))
}

fn require_write_confirmation(confirm_write: bool, operation: &str) -> Result<(), CommandError> {
    if confirm_write {
        return Ok(());
    }
    Err(CommandError::new(
        "write-not-confirmed",
        format!("{operation}前必须进行显式确认"),
    ))
}

const AUDIT_LOG_WARNING: &str = "配置已写入，但本地审计记录保存失败；本次操作无法从应用中撤回";

fn record_audit_or_warn(result: Result<(), String>, warnings: &mut Vec<String>) {
    if result.is_err() {
        warnings.push(AUDIT_LOG_WARNING.to_string());
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFileStatus {
    pub app: AppKind,
    pub path: String,
    pub exists: bool,
    pub syntax_ok: bool,
    pub route: Option<RouteState>,
    pub read_error: Option<String>,
    pub match_status: MatchStatus,
    pub last_switch: Option<SwitchLog>,
}

fn classify_match_status(
    last: Option<&SwitchLog>,
    current_hash: &str,
    matching_profile: Option<&ProviderProfile>,
) -> MatchStatus {
    if let Some(record) = last {
        if record.content_hash == current_hash && record.profile_id.is_none() {
            return MatchStatus::RestoredBackup {
                at: record.at.clone(),
            };
        }
    }
    if let Some(profile) = matching_profile {
        return MatchStatus::MatchesProfile {
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
        };
    }

    match last {
        Some(record) if record.content_hash == current_hash => MatchStatus::ProfileChanged {
            profile_name: record
                .profile_name
                .clone()
                .unwrap_or_else(|| "已删除档案".to_string()),
        },
        Some(record) => MatchStatus::ExternallyModified {
            at: record.at.clone(),
        },
        None => MatchStatus::Unmanaged,
    }
}

/// Decides whether the live content still matches a current profile, a
/// restored backup, or the app's last record. Requires the file to be readable
/// and syntactically valid.
fn match_status_for(
    state: &LocalState,
    kind: AppKind,
    text: &str,
) -> Result<MatchStatus, CommandError> {
    let store = state
        .load_store()
        .map_err(|error| CommandError::new("store-unreadable", error))?;
    let hash = sha256_hex(text);
    let last = store
        .switch_log
        .iter()
        .rev()
        .find(|entry| entry.app == kind);

    let common = store
        .common
        .iter()
        .find(|patch| patch.app == kind)
        .cloned()
        .unwrap_or(CommonConfigPatch {
            app: kind,
            entries: vec![],
        });
    let matching_profile = store
        .profiles
        .iter()
        .filter(|p| p.app == kind)
        .find(|profile| {
            let profile = *profile;
            let plan = SwitchPlan {
                app: kind,
                profile: profile.clone(),
                common: common.clone(),
            };
            let unchanged = asb_core::validate_plan(&plan.profile, &plan.common).is_ok()
                && matches!(
                    adapter::preview(text, &plan, ""),
                    Ok(preview) if preview.changes.is_empty()
                );
            unchanged
        });

    Ok(classify_match_status(last, &hash, matching_profile))
}

#[tauri::command]
pub fn config_status(app: AppHandle) -> Result<Vec<ConfigFileStatus>, CommandError> {
    let state = state(&app)?;
    let io = FsIo;
    [AppKind::Codex, AppKind::Claude]
        .into_iter()
        .map(|kind| {
            let target = state
                .target(kind)
                .map_err(|error| CommandError::new("config-path-unavailable", error))?;
            let last_switch = state
                .latest_switch(kind)
                .map_err(|error| CommandError::new("store-unreadable", error))?;
            let status = match io.read_file(&target) {
                Ok(text) => match adapter::validate_syntax(kind, &text) {
                    Ok(()) => ConfigFileStatus {
                        app: kind,
                        path: target.to_string_lossy().to_string(),
                        exists: true,
                        syntax_ok: true,
                        route: Some(adapter::route_state(kind, &text)),
                        read_error: None,
                        match_status: match_status_for(&state, kind, &text)?,
                        last_switch,
                    },
                    Err(_) => ConfigFileStatus {
                        app: kind,
                        path: target.to_string_lossy().to_string(),
                        exists: true,
                        syntax_ok: false,
                        route: None,
                        read_error: None,
                        match_status: MatchStatus::Unknown,
                        last_switch,
                    },
                },
                Err(error) if error.kind() == ErrorKind::NotFound => ConfigFileStatus {
                    app: kind,
                    path: target.to_string_lossy().to_string(),
                    exists: false,
                    syntax_ok: false,
                    route: None,
                    read_error: None,
                    match_status: MatchStatus::Unknown,
                    last_switch,
                },
                Err(_) => ConfigFileStatus {
                    app: kind,
                    path: target.to_string_lossy().to_string(),
                    exists: true,
                    syntax_ok: false,
                    route: None,
                    read_error: Some("无法读取配置文件".to_string()),
                    match_status: MatchStatus::Unknown,
                    last_switch,
                },
            };
            Ok(status)
        })
        .collect()
}

#[tauri::command]
pub fn list_profiles(app: AppHandle) -> Result<Vec<ProviderProfile>, CommandError> {
    state(&app)?
        .list_profiles()
        .map_err(|error| CommandError::new("store-unreadable", error))
}

#[tauri::command]
pub fn create_profile(
    app: AppHandle,
    draft: ProviderDraft,
) -> Result<ProviderProfile, CommandError> {
    state(&app)?
        .create_profile(draft)
        .map_err(|error| CommandError::new("profile-create-failed", error))
}

#[tauri::command]
pub fn update_profile(
    app: AppHandle,
    profile_id: String,
    draft: ProviderDraft,
) -> Result<ProviderProfile, CommandError> {
    state(&app)?
        .update_profile(&profile_id, draft)
        .map_err(|error| CommandError::new("profile-update-failed", error))
}

#[tauri::command]
pub fn delete_profile(app: AppHandle, profile_id: String) -> Result<(), CommandError> {
    state(&app)?
        .delete_profile(&profile_id)
        .map_err(|error| CommandError::new("profile-delete-failed", error))
}

#[tauri::command]
pub fn import_discovered_profile(
    app: AppHandle,
    target: AppKind,
) -> Result<ProviderProfile, CommandError> {
    let proposal = discover_local()?
        .import_proposals
        .into_iter()
        .find(|candidate| candidate.app == target)
        .ok_or_else(|| CommandError::new("import-unavailable", "当前配置没有可导入的供应商"))?;
    state(&app)?
        .import_profile(proposal.draft)
        .map_err(|error| CommandError::new("profile-import-failed", error))
}

#[tauri::command]
pub fn get_common(app: AppHandle, target: AppKind) -> Result<CommonConfigPatch, CommandError> {
    state(&app)?
        .get_common(target)
        .map_err(|error| CommandError::new("store-unreadable", error))
}

#[tauri::command]
pub fn set_common(
    app: AppHandle,
    target: AppKind,
    patch: CommonConfigPatch,
) -> Result<(), CommandError> {
    state(&app)?
        .set_common(target, patch)
        .map_err(|error| CommandError::new("common-save-failed", error))
}

fn build_plan(state: &LocalState, profile_id: &str) -> Result<SwitchPlan, CommandError> {
    let profile = state
        .find_profile(profile_id)
        .map_err(|error| CommandError::new("profile-not-found", error))?;
    let common = state
        .get_common(profile.app)
        .map_err(|error| CommandError::new("store-unreadable", error))?;
    let plan = SwitchPlan {
        app: profile.app,
        profile,
        common,
    };
    asb_core::validate_plan(&plan.profile, &plan.common)
        .map_err(|error| CommandError::new("invalid-plan", error.to_string()))?;
    Ok(plan)
}

#[tauri::command]
pub fn preview_switch(app: AppHandle, profile_id: String) -> Result<FilePreview, CommandError> {
    let state = state(&app)?;
    let plan = build_plan(&state, &profile_id)?;
    let target = state
        .target(plan.app)
        .map_err(|error| CommandError::new("config-path-unavailable", error))?;
    let backup_dir = state.backup_dir();
    let mut preview = read_preview(&FsIo, &target, &plan, &backup_dir.to_string_lossy())
        .map_err(CommandError::from)?;
    preview.preview.target = target.to_string_lossy().to_string();
    Ok(preview)
}

#[tauri::command]
pub fn execute_switch(
    app: AppHandle,
    profile_id: String,
    expected_hash: String,
    expected_rendered_hash: String,
    confirm_write: bool,
) -> Result<SwitchOutcome, CommandError> {
    require_write_confirmation(confirm_write, "写入配置")?;
    let state = state(&app)?;
    let plan = build_plan(&state, &profile_id)?;
    let target = state
        .target(plan.app)
        .map_err(|error| CommandError::new("config-path-unavailable", error))?;
    let backup_dir = state.backup_dir();
    let mut outcome = execute(
        &FsIo,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &expected_hash,
            expected_rendered_hash: &expected_rendered_hash,
        },
    )
    .map_err(CommandError::from)?;
    outcome.preview.target = target.to_string_lossy().to_string();
    record_audit_or_warn(
        state.record_switch(SwitchLog {
            app: plan.app,
            profile_id: Some(plan.profile.id.clone()),
            profile_name: Some(plan.profile.name.clone()),
            content_hash: outcome.final_hash.clone(),
            backup_id: outcome.backup.id.clone(),
            at: outcome.backup.created_at.clone(),
        }),
        &mut outcome.warnings,
    );
    Ok(outcome)
}

#[tauri::command]
pub fn list_backups(app: AppHandle) -> Result<Vec<BackupRecord>, CommandError> {
    local_backups(&state(&app)?)
}

fn local_backups(state: &LocalState) -> Result<Vec<BackupRecord>, CommandError> {
    let targets = [AppKind::Codex, AppKind::Claude]
        .into_iter()
        .map(|app| {
            state
                .target(app)
                .map(|target| (app, target))
                .map_err(|error| CommandError::new("config-path-unavailable", error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(scan_backups(&FsIo, &state.backup_dir())
        .into_iter()
        .filter(|record| {
            targets.iter().any(|(app, target)| {
                record.app == *app
                    && PathBuf::from(&record.target_path).as_path() == target.as_path()
            })
        })
        .collect())
}

fn find_backup(state: &LocalState, backup_id: &str) -> Result<BackupRecord, CommandError> {
    local_backups(state)?
        .into_iter()
        .find(|record| record.id == backup_id)
        .ok_or_else(|| CommandError::new("backup-not-found", "找不到指定备份"))
}

/// Shared restore path: validates the target, restores, and records the
/// operation in the switch log so it can be undone in turn.
fn run_restore(state: &LocalState, record: &BackupRecord) -> Result<RestoreOutcome, CommandError> {
    let target = state
        .target(record.app)
        .map_err(|error| CommandError::new("config-path-unavailable", error))?;
    if PathBuf::from(&record.target_path) != target {
        return Err(CommandError::new(
            "backup-target-invalid",
            "备份不属于当前本机配置路径，已拒绝恢复",
        ));
    }
    let mut outcome = restore(&FsIo, record, &target).map_err(CommandError::from)?;
    record_audit_or_warn(
        state.record_switch(SwitchLog {
            app: record.app,
            profile_id: None,
            profile_name: None,
            content_hash: outcome.restored_hash.clone(),
            backup_id: outcome.pre_restore_backup.id.clone(),
            at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        }),
        &mut outcome.warnings,
    );
    Ok(outcome)
}

#[tauri::command]
pub fn restore_backup(
    app: AppHandle,
    backup_id: String,
    confirm_write: bool,
) -> Result<RestoreOutcome, CommandError> {
    require_write_confirmation(confirm_write, "恢复配置")?;
    let state = state(&app)?;
    let record = find_backup(&state, &backup_id)?;
    run_restore(&state, &record)
}

/// Undoes the most recent switch for one client by restoring the backup that
/// switch created.
#[tauri::command]
pub fn undo_last_switch(
    app: AppHandle,
    target: AppKind,
    confirm_write: bool,
) -> Result<RestoreOutcome, CommandError> {
    require_write_confirmation(confirm_write, "撤回切换")?;
    let state = state(&app)?;
    let last = state
        .latest_switch(target)
        .map_err(|error| CommandError::new("store-unreadable", error))?
        .ok_or_else(|| CommandError::new("undo-unavailable", "该客户端没有可撤回的切换记录"))?;
    let record = find_backup(&state, &last.backup_id)?;
    run_restore(&state, &record)
}

/// Owned-key difference between the live file and one backup. `before` is
/// the backup value, `after` the current one.
#[tauri::command]
pub fn backup_diff(app: AppHandle, backup_id: String) -> Result<Vec<KeyChange>, CommandError> {
    let state = state(&app)?;
    let record = find_backup(&state, &backup_id)?;
    let target = state
        .target(record.app)
        .map_err(|error| CommandError::new("config-path-unavailable", error))?;
    let current = FsIo
        .read_file(&target)
        .map_err(|_| CommandError::new("read-current", "无法读取当前配置文件，无法生成差异"))?;
    let backup_text = FsIo
        .read_file(PathBuf::from(&record.backup_path).as_path())
        .map_err(|_| CommandError::new("backup-unreadable", "备份文件不可读，无法生成差异"))?;
    if sha256_hex(&backup_text) != record.content_hash {
        return Err(CommandError::new(
            "backup-hash-mismatch",
            "备份内容与记录哈希不符，拒绝生成差异",
        ));
    }
    adapter::owned_diff(record.app, &current, &backup_text)
        .map_err(|error| CommandError::new("diff-failed", error.to_string()))
}

#[tauri::command]
pub fn lock_status(app: AppHandle, target: AppKind) -> Result<LockStatus, CommandError> {
    let state = state(&app)?;
    let target = state
        .target(target)
        .map_err(|error| CommandError::new("config-path-unavailable", error))?;
    Ok(lockfile::probe_lock(&FsIo, &target))
}

#[tauri::command]
pub fn recover_stale_lock(app: AppHandle, target: AppKind) -> Result<RecoveryEntry, CommandError> {
    let state = state(&app)?;
    let target = state
        .target(target)
        .map_err(|error| CommandError::new("config-path-unavailable", error))?;
    lockfile::recover_stale(&FsIo, &target)
        .map_err(|_| CommandError::new("lock-not-stale", "当前锁不是可恢复的遗留状态"))
}

/// Result of one manual endpoint probe. It is informational only: nothing is
/// selected or switched automatically.
pub use crate::probe::ProbeResult;

#[tauri::command]
pub fn probe_endpoint(url: String) -> Result<ProbeResult, CommandError> {
    crate::probe::probe(&url).map_err(|error| CommandError::new("probe-failed", error))
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

/// Read-only discovery of local Codex and Claude Code configuration. Reads at
/// most two files; never writes, creates, or locks either target.
#[tauri::command]
pub fn discover_local() -> Result<DiscoveryReport, CommandError> {
    let paths = local_config_paths()
        .map_err(|error| CommandError::new("config-path-unavailable", error))?;
    let read = |path: &str| match std::fs::read_to_string(path) {
        Ok(text) if std::path::Path::new(path).is_file() => Ok(Some(text)),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err("无法读取配置文件".to_string()),
    };
    Ok(discovery::discover(&paths, read))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> ProviderProfile {
        ProviderProfile {
            id: "p1".to_string(),
            app: AppKind::Codex,
            mode: asb_core::RouteMode::Official,
            name: "当前档案".to_string(),
            model: Some("gpt-5.4".to_string()),
            base_url: None,
            env_key: None,
            model_options: None,
        }
    }

    fn log(content_hash: &str, profile_id: Option<&str>, profile_name: Option<&str>) -> SwitchLog {
        SwitchLog {
            app: AppKind::Codex,
            profile_id: profile_id.map(str::to_string),
            profile_name: profile_name.map(str::to_string),
            content_hash: content_hash.to_string(),
            backup_id: "b1".to_string(),
            at: "2026-08-26T08:00:00Z".to_string(),
        }
    }

    #[test]
    fn writes_require_explicit_confirmation() {
        let error = require_write_confirmation(false, "写入配置").expect_err("must reject");
        assert_eq!(error.code, "write-not-confirmed");
        assert!(error.message.contains("显式确认"));
        assert!(require_write_confirmation(true, "写入配置").is_ok());
    }

    #[test]
    fn successful_configuration_write_reports_audit_store_failure_as_a_warning() {
        let mut warnings = vec![];
        record_audit_or_warn(Err("store unavailable".to_string()), &mut warnings);
        assert_eq!(warnings, vec![AUDIT_LOG_WARNING.to_string()]);

        record_audit_or_warn(Ok(()), &mut warnings);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn match_status_never_activates_a_restored_backup_or_an_edited_profile() {
        let current = profile();
        let restored =
            classify_match_status(Some(&log("same", None, None)), "same", Some(&current));
        assert!(matches!(restored, MatchStatus::RestoredBackup { .. }));

        let stale_profile =
            classify_match_status(Some(&log("same", Some("p1"), Some("旧档案"))), "same", None);
        assert_eq!(
            stale_profile,
            MatchStatus::ProfileChanged {
                profile_name: "旧档案".to_string(),
            }
        );

        let matching = classify_match_status(
            Some(&log("old", Some("p1"), Some("旧档案"))),
            "current",
            Some(&current),
        );
        assert_eq!(
            matching,
            MatchStatus::MatchesProfile {
                profile_id: "p1".to_string(),
                profile_name: "当前档案".to_string(),
            }
        );
    }
}
