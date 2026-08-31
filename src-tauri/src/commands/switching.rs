//! Profile-switch, backup, restore, and undo commands. Each write walks the
//! executor transaction and records an audit entry so it can be undone.

use super::error::{
    blocking, observe, record_audit_or_warn, require_write_confirmation, state, store_error,
    CommandError,
};
use crate::runtime_log::RuntimeLogAction;
use asb_core::adapter;
use asb_core::contracts::{AppKind, BackupRecord, KeyChange, SwitchLog, SwitchOp, SwitchPlan};
use asb_switch::io::{FsIo, SwitchIo};
use asb_switch::{
    execute, list_backups as scan_backups, read_preview, restore, sha256_hex, RestoreOutcome,
    SwitchOutcome,
};
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

fn build_plan(
    state: &crate::local_state::LocalState,
    profile_id: &str,
) -> Result<SwitchPlan, CommandError> {
    let profile = state
        .find_profile(profile_id)
        .map_err(|error| CommandError::new("profile-not-found", error))?;
    let common = state.get_common(profile.app).map_err(store_error)?;
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
pub async fn preview_switch(
    app: AppHandle,
    profile_id: String,
) -> Result<asb_switch::FilePreview, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let plan = build_plan(&state, &profile_id)?;
        let target = state
            .target(plan.app)
            .map_err(|error| CommandError::new("config-path-unavailable", error))?;
        let backup_dir = state.backup_dir();
        let mut preview = read_preview(&FsIo, &target, &plan, &backup_dir.to_string_lossy())
            .map_err(CommandError::from)?;
        preview.preview.target = target.to_string_lossy().to_string();
        Ok(preview)
    })
    .await
}

#[tauri::command]
pub async fn execute_switch(
    app: AppHandle,
    profile_id: String,
    expected_hash: String,
    expected_rendered_hash: String,
    confirm_write: bool,
) -> Result<SwitchOutcome, CommandError> {
    observe(RuntimeLogAction::ConfigurationSwitched, async move {
        require_write_confirmation(confirm_write, "写入配置")?;
        let state = state(&app)?;
        blocking(move || {
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
                    operation: SwitchOp::Switch,
                }),
                &mut outcome.warnings,
            );
            crate::tray::refresh(&app);
            Ok(outcome)
        })
        .await
    })
    .await
}

#[tauri::command]
pub async fn list_backups(app: AppHandle) -> Result<Vec<BackupRecord>, CommandError> {
    let state = state(&app)?;
    blocking(move || local_backups(&state)).await
}

fn local_backups(
    state: &crate::local_state::LocalState,
) -> Result<Vec<BackupRecord>, CommandError> {
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

fn find_backup(
    state: &crate::local_state::LocalState,
    backup_id: &str,
) -> Result<BackupRecord, CommandError> {
    local_backups(state)?
        .into_iter()
        .find(|record| record.id == backup_id)
        .ok_or_else(|| CommandError::new("backup-not-found", "找不到指定备份"))
}

/// Shared restore path: validates the target, restores, and records the
/// operation in the switch log so it can be undone in turn.
fn run_restore(
    state: &crate::local_state::LocalState,
    record: &BackupRecord,
) -> Result<RestoreOutcome, CommandError> {
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
            operation: SwitchOp::Switch,
        }),
        &mut outcome.warnings,
    );
    Ok(outcome)
}

#[tauri::command]
pub async fn restore_backup(
    app: AppHandle,
    backup_id: String,
    confirm_write: bool,
) -> Result<RestoreOutcome, CommandError> {
    observe(RuntimeLogAction::BackupRestored, async move {
        require_write_confirmation(confirm_write, "恢复配置")?;
        let state = state(&app)?;
        blocking(move || {
            let record = find_backup(&state, &backup_id)?;
            let outcome = run_restore(&state, &record)?;
            crate::tray::refresh(&app);
            Ok(outcome)
        })
        .await
    })
    .await
}

/// Undoes the most recent switch for one client by restoring the backup that
/// switch created.
#[tauri::command]
pub async fn undo_last_switch(
    app: AppHandle,
    target: AppKind,
    confirm_write: bool,
) -> Result<RestoreOutcome, CommandError> {
    observe(RuntimeLogAction::SwitchUndone, async move {
        require_write_confirmation(confirm_write, "撤回切换")?;
        let state = state(&app)?;
        blocking(move || {
            let last = state
                .latest_switch(target)
                .map_err(store_error)?
                .ok_or_else(|| {
                    CommandError::new("undo-unavailable", "该客户端没有可撤回的切换记录")
                })?;
            let record = find_backup(&state, &last.backup_id)?;
            let outcome = run_restore(&state, &record)?;
            crate::tray::refresh(&app);
            Ok(outcome)
        })
        .await
    })
    .await
}

/// Owned-key difference between the live file and one backup. `before` is
/// the backup value, `after` the current one.
#[tauri::command]
pub async fn backup_diff(
    app: AppHandle,
    backup_id: String,
) -> Result<Vec<KeyChange>, CommandError> {
    let state = state(&app)?;
    blocking(move || {
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
    })
    .await
}

/// Opens the app-owned backup directory in the system file manager. The
/// directory is created on demand so an empty history still opens cleanly.
#[tauri::command]
pub async fn open_backup_dir(app: AppHandle) -> Result<(), CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let dir = state.backup_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|_| CommandError::new("backup-dir-unavailable", "无法创建备份目录"))?;
        app.opener()
            .open_path(dir.to_string_lossy().into_owned(), None::<&str>)
            .map_err(|_| CommandError::new("backup-dir-open-failed", "无法打开备份文件夹"))
    })
    .await
}
