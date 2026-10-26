//! Profile-switch, backup, restore, and undo commands. Each write walks the
//! executor transaction and records an audit entry so it can be undone.

pub(in crate::commands) mod backups;
mod backup_views;
pub use backup_views::*;
mod client_settings_backup;
pub(in crate::commands) use backups::validate_client_configuration_backup;
pub(crate) mod claude_gateway;
mod codex_backfill;
pub(crate) mod codex_policy;
mod codex_profile_save;
pub(in crate::commands) mod codex_restore_auth;
mod internal;
pub(in crate::commands) mod plan;
mod profile_rollback;
mod profile_save;
mod projection_transaction;
mod recovery;
pub(super) mod transaction;

pub(crate) use codex_policy::CodexPolicyPreparations;
pub(crate) use codex_profile_save::CodexProfileSavePreparations;
pub(crate) use codex_profile_save::validate_subagent_route_reference;
pub(crate) use internal::switch_provider_internal;
pub use profile_save::{ProfileSaveIntent, ProfileSavePreparation, ProfileSaveSource};
pub(crate) use profile_save::ProfileSavePreparations;
pub(crate) use recovery::{ensure_profile_save_recovered, recover_pending_profile_save};

use crate::commands::error::{
    blocking, observe, require_write_confirmation, state, store_error, CommandError,
};
use crate::commands::ConfigWriteGate;
use crate::runtime_log::RuntimeLogAction;
use asb_core::contracts::{
    AppKind, BackupRecord, ProfileSaveKind, ProviderDraft, ProviderRecord,
    WriteOperation,
};
use asb_switch::{RestoreOutcome, SwitchOutcome};
use backups::{find_backup, local_backups, run_restore};
use codex_profile_save::{
    commit as commit_codex_profile_save_data, prepare_data as prepare_codex_profile_save_data,
    PreparedCodexProfileSave,
};
use plan::{build_plan, preview_projection};
use profile_save::{
    commit_prepared_profile_save, invalidate_provider_cache, prepare_profile_save_data,
    PreparedProfileSave,
};
use std::time::Instant;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn cancel_profile_save(
    app: AppHandle,
    source: ProfileSaveSource,
    preparation_id: String,
) -> Result<(), CommandError> {
    let missing = || CommandError::keyed("profile-save-unavailable",
        "errors.sw.preparationStateUninitialized", "供应商保存准备状态尚未初始化");
    match source {
        ProfileSaveSource::Profile => app.try_state::<ProfileSavePreparations>()
            .ok_or_else(missing)?.cancel(&preparation_id),
        ProfileSaveSource::Codex => app.try_state::<CodexProfileSavePreparations>()
            .ok_or_else(missing)?.cancel(&preparation_id),
    }
}

/// Validates a draft and binds it to one one-shot server-side preparation. No
/// provider or client file is modified here.
#[tauri::command]
pub async fn prepare_profile_save(
    app: AppHandle,
    profile_id: Option<String>,
    draft: ProviderDraft,
    expected_file_hash: Option<String>,
    intent: ProfileSaveIntent,
) -> Result<ProfileSavePreparation, CommandError> {
    let state = state(&app)?;
    let gateway = app
        .state::<crate::gateway::GatewayController>()
        .inner()
        .clone();
    let preparations = app
        .try_state::<ProfileSavePreparations>()
        .ok_or_else(|| {
            CommandError::keyed(
                "profile-save-unavailable",
                "errors.sw.preparationStateUninitialized",
                "供应商保存准备状态尚未初始化",
            )
        })?
        .inner()
        .clone();
    blocking(move || {
        let (kind, preview, projection_profile, candidate_revision) = prepare_profile_save_data(
            &state,
            &gateway,
            profile_id.as_deref(),
            &draft,
            expected_file_hash.as_deref(),
            intent,
        )?;
        let preparation_id = preparations.issue(PreparedProfileSave {
            intent,
            created_at: Instant::now(),
            profile_id,
            expected_file_hash,
            draft,
            kind,
            preview: preview.clone(),
            projection_profile,
            candidate_revision,
        })?;
        Ok(ProfileSavePreparation {
            preparation_id,
            kind,
            preview,
        })
    })
    .await
}

/// Commits exactly the draft bound by [`prepare_profile_save`]. A preparation
/// is one-shot, so duplicate confirmation clicks and stale renderer state
/// cannot execute an old plan.
#[tauri::command]
pub async fn commit_profile_save(
    app: AppHandle,
    preparation_id: String,
    confirm_write: bool,
) -> Result<ProviderRecord, CommandError> {
    let preparations = app
        .try_state::<ProfileSavePreparations>()
        .ok_or_else(|| {
            CommandError::keyed(
                "profile-save-unavailable",
                "errors.sw.preparationStateUninitialized",
                "供应商保存准备状态尚未初始化",
            )
        })?
        .inner()
        .clone();
    let prepared = preparations.take(&preparation_id)?;
    let action = if prepared.kind == ProfileSaveKind::Create {
        RuntimeLogAction::ProfileCreated
    } else {
        RuntimeLogAction::ProfileUpdated
    };
    let invalidates_readings = prepared.kind != ProfileSaveKind::NoChange;
    let refresh_app = app.clone();
    let result = observe(action, async move {
        let state = state(&app)?;
        let gateway = app
            .state::<crate::gateway::GatewayController>()
            .inner()
            .clone();
        blocking(move || {
            let _commit_guard = preparations.lock_commit()?;
            let write_gate = write_gate(&app)?;
            let _write_gate = write_gate
                .lock()
                .map_err(|error| CommandError::new("config-write-gate-unavailable", error))?;
            ensure_profile_save_recovered(&app)?;
            commit_prepared_profile_save(&state, &gateway, &prepared, confirm_write)
        })
        .await
    })
    .await;
    if invalidates_readings {
        if let Ok(record) = &result {
            invalidate_provider_cache(&refresh_app, &record.profile.id);
        }
    }
    result
}

#[tauri::command]
pub async fn preview_switch(
    app: AppHandle,
    profile_id: String,
) -> Result<asb_switch::FilePreview, CommandError> {
    let state = state(&app)?;
    let gateway = app
        .state::<crate::gateway::GatewayController>()
        .inner()
        .clone();
    blocking(move || {
        let projection = build_plan(&state, &gateway, &profile_id)?;
        preview_projection(&state, &projection)
    })
    .await
}

#[tauri::command]
pub async fn prepare_codex_profile_save(
    app: AppHandle,
    profile_id: String,
    draft: asb_core::contracts::CodexProviderDraft,
    expected_file_hash: String,
    intent: ProfileSaveIntent,
) -> Result<ProfileSavePreparation, CommandError> {
    let state = state(&app)?;
    let gateway = app
        .state::<crate::gateway::GatewayController>()
        .inner()
        .clone();
    let preparations = app
        .try_state::<CodexProfileSavePreparations>()
        .ok_or_else(|| {
            CommandError::keyed(
                "codex-profile-save-unavailable",
                "errors.sw.codexPreparationStateUninitialized",
                "Codex 供应商保存准备状态尚未初始化",
            )
        })?
        .inner()
        .clone();
    blocking(move || {
        let (kind, preview, projection_profile, candidate_revision) = prepare_codex_profile_save_data(
            &state,
            &gateway,
            &profile_id,
            &draft,
            &expected_file_hash,
            intent,
        )?;
        let preparation_id = preparations.issue(PreparedCodexProfileSave {
            intent,
            created_at: Instant::now(),
            profile_id,
            expected_file_hash,
            draft,
            kind,
            preview: preview.clone(),
            projection_profile,
            candidate_revision,
        })?;
        Ok(ProfileSavePreparation {
            preparation_id,
            kind,
            preview,
        })
    })
    .await
}

#[tauri::command]
pub async fn commit_codex_profile_save(
    app: AppHandle,
    preparation_id: String,
    confirm_write: bool,
) -> Result<asb_core::contracts::CodexProviderRecord, CommandError> {
    let preparations = app
        .try_state::<CodexProfileSavePreparations>()
        .ok_or_else(|| {
            CommandError::keyed(
                "codex-profile-save-unavailable",
                "errors.sw.codexPreparationStateUninitialized",
                "Codex 供应商保存准备状态尚未初始化",
            )
        })?
        .inner()
        .clone();
    let prepared = preparations.take(&preparation_id)?;
    let refresh_app = app.clone();
    let result = observe(RuntimeLogAction::ProfileUpdated, async move {
        let state = state(&app)?;
        let gateway = app
            .state::<crate::gateway::GatewayController>()
            .inner()
            .clone();
        blocking(move || {
            let _commit_guard = preparations.lock_commit()?;
            let write_gate = write_gate(&app)?;
            let _write_gate = write_gate
                .lock()
                .map_err(|error| CommandError::new("config-write-gate-unavailable", error))?;
            ensure_profile_save_recovered(&app)?;
            commit_codex_profile_save_data(&state, &gateway, &prepared, confirm_write)
        })
        .await
    })
    .await;
    if let Ok(record) = &result {
        invalidate_provider_cache(&refresh_app, &record.profile.id);
    }
    result
}

fn write_gate(app: &AppHandle) -> Result<ConfigWriteGate, CommandError> {
    app.try_state::<ConfigWriteGate>()
        .map(|gate| gate.inner().clone())
        .ok_or_else(|| {
            CommandError::keyed(
                "app-state-unavailable",
                "errors.sw.writeGateUninitialized",
                "写入闸门尚未初始化",
            )
        })
}

#[tauri::command]
pub async fn execute_switch(
    app: AppHandle,
    profile_id: String,
    expected_hash: String,
    expected_rendered_hash: String,
    confirm_write: bool,
    auth_hash: Option<String>,
    auth_existed: Option<bool>,
    auth_rendered_hash: Option<String>,
) -> Result<SwitchOutcome, CommandError> {
    observe(RuntimeLogAction::ConfigurationSwitched, async move {
        require_write_confirmation(confirm_write, "写入配置")?;
        execute_switch_core(
            app,
            profile_id,
            expected_hash,
            expected_rendered_hash,
            auth_hash,
            auth_existed,
            auth_rendered_hash,
        )
        .await
    })
    .await
}

async fn execute_switch_core(
    app: AppHandle,
    profile_id: String,
    expected_hash: String,
    expected_rendered_hash: String,
    auth_hash: Option<String>,
    auth_existed: Option<bool>,
    auth_rendered_hash: Option<String>,
) -> Result<SwitchOutcome, CommandError> {
    let state = state(&app)?;
    let gateway = app
        .state::<crate::gateway::GatewayController>()
        .inner()
        .clone();
    blocking(move || {
        let write_gate = write_gate(&app)?;
        let _write_gate = write_gate
            .lock()
            .map_err(|error| CommandError::new("config-write-gate-unavailable", error))?;
        ensure_profile_save_recovered(&app)?;
        let projection = build_plan(&state, &gateway, &profile_id)?;
        let mut outcome = plan::execute_projection_with_auth(
            &state,
            &gateway,
            &projection,
            &expected_hash,
            &expected_rendered_hash,
            auth_hash.as_deref(),
            auth_existed,
            auth_rendered_hash.as_deref(),
        )?;
        // The Claude plugin marker lives in a separate client file with its
        // own lock and backups; the switch itself is already committed, so
        // a failure here is reported, not rolled back.
        if projection.plan.app() == AppKind::Claude {
            if let Err(message) = crate::claude_integration::reconcile_after_switch(
                state.root(),
                projection.plan.profile.route_mode,
            ) {
                outcome.warnings.push(asb_core::contracts::LocalizedMessage::new(
                    "warnings.claude.pluginMarkerUnsynced",
                    serde_json::json!({ "detail": message }),
                    format!("Claude 插件集成标记未同步：{message}"),
                ));
            }
        }
        crate::tray::refresh(&app);
        Ok(outcome)
    })
    .await
}

#[tauri::command]
pub async fn list_backups(app: AppHandle) -> Result<Vec<BackupRecord>, CommandError> {
    let state = state(&app)?;
    blocking(move || local_backups(&state)).await
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
        let gateway = app
            .state::<crate::gateway::GatewayController>()
            .inner()
            .clone();
        blocking(move || {
            let write_gate = write_gate(&app)?;
            let _write_gate = write_gate
                .lock()
                .map_err(|error| CommandError::new("config-write-gate-unavailable", error))?;
            if let Some(outcome) =
                transaction::restore_pending_backup(&state, &gateway, &backup_id)?
            {
                crate::tray::refresh(&app);
                return Ok(outcome);
            }
            ensure_profile_save_recovered(&app)?;
            let record = find_backup(&state, &backup_id)?;
            let outcome = run_restore(&state, &gateway, &record, None)?;
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
        let gateway = app
            .state::<crate::gateway::GatewayController>()
            .inner()
            .clone();
        blocking(move || {
            let write_gate = write_gate(&app)?;
            let _write_gate = write_gate.lock().map_err(|error| {
                CommandError::new("config-write-gate-unavailable", error)
            })?;
            ensure_profile_save_recovered(&app)?;
            let last = state
                .configuration()
                .latest_config_write(target)
                .map_err(store_error)?
                .ok_or_else(|| {
                    CommandError::keyed(
                        "undo-unavailable",
                        "errors.sw.noUndoRecord",
                        "该客户端没有可撤回的切换记录",
                    )
                })?;
            if last.operation == WriteOperation::GatewayPortChange {
                return Err(CommandError::keyed(
                    "undo-unavailable",
                    "errors.sw.undoGatewayPortUnsupported",
                    "网关端口修改涉及全部客户端与监听器，请在网关页修改端口，不能撤回单个客户端配置",
                ));
            }
            let record = find_backup(&state, &last.backup_id)?;
            let outcome = run_restore(&state, &gateway, &record, None)?;
            crate::tray::refresh(&app);
            Ok(outcome)
        })
        .await
    })
    .await
}
