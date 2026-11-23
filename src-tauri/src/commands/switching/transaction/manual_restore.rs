//! The confirmed backup path for an interrupted executor transaction.
use super::super::profile_rollback;
use super::*;
use asb_switch::{RecoveryOutcome, RestoreOutcome, SwitchError};

#[cfg(test)]
mod tests;

fn checked_pending(
    state: &LocalState,
    intent: &SwitchIntent,
) -> Result<Option<PendingConfigWrite>, String> {
    let pending = asb_switch::pending_config_write(&FsIo, &state.backup_dir(), intent.app)
        .map_err(|failure| failure.to_string())?;
    if let Some(pending) = &pending {
        validate_recovery_target(state, intent)?;
        validate_pending(intent, pending)?;
    }
    Ok(pending)
}

pub(crate) fn recovery_backup_id(state: &LocalState) -> Option<String> {
    let intent = load(state).ok().flatten()?;
    checked_pending(state, &intent).ok().flatten().map(|pending| pending.backup.id)
}

pub(in crate::commands) fn recovery_status(state: &LocalState) -> Option<String> {
    let intent = match load(state) {
        Ok(Some(intent)) => intent,
        Ok(None) => return reject_orphan(state).err(),
        Err(failure) => return Some(failure),
    };
    let client = match intent.app { AppKind::Codex => "Codex", AppKind::Claude => "Claude" };
    Some(match checked_pending(state, &intent) {
        Ok(Some(pending)) => format!(
            "{client} 配置事务未完成。请在设置的备份恢复中选择备份 {}；恢复前保留当前文件和事务记录。",
            pending.backup.id
        ),
        Ok(None) => format!("{client} 配置事务未完成。请保留当前文件和事务记录，重试原操作以完成恢复；若仍失败，请查看日志中的具体原因。"),
        Err(failure) => failure,
    })
}

pub(in crate::commands::switching) fn restore_pending_backup(
    state: &LocalState,
    gateway: &GatewayController,
    backup_id: &str,
    transaction_only: bool,
) -> Result<Option<RestoreOutcome>, CommandError> {
    if let Some(intent) = load(state).map_err(error)? {
        if let Some(pending) = checked_pending(state, &intent).map_err(error)?
            .filter(|pending| pending.backup.id == backup_id)
        {
            return restore_confirmed(state, gateway, &intent, &pending).map(Some);
        }
    }
    if transaction_only {
        return Err(transaction_backup_mismatch());
    }
    Ok(None)
}

pub(in crate::commands::switching) fn transaction_backup_mismatch() -> CommandError {
    CommandError::keyed(
        "transaction-backup-mismatch",
        "errors.sw.transactionBackupMismatch",
        "所选备份与当前未完成事务不匹配。请重新检查应用数据，再选择标为事务备份的记录。",
    )
}

fn restore_confirmed(
    state: &LocalState,
    gateway: &GatewayController,
    intent: &SwitchIntent,
    pending: &PendingConfigWrite,
) -> Result<RestoreOutcome, CommandError> {
    let store = state.configuration();
    let profile = if let Some(save) = store.pending_profile_save().map_err(|e| error(e.to_string()))? {
        if save.app != intent.app || intent.profile_id.as_deref() != Some(save.projection_profile_id.as_str()) {
            return Err(CommandError::keyed(
                "config-recovery-required",
                "errors.sw.saveTransactionMismatch",
                "供应商保存与配置事务不匹配",
            ));
        }
        let revision = profile_revision(state, save.app, &save.profile_id).map_err(error)?;
        if revision != save.previous_file_hash {
            profile_rollback::validate_saved_revision(state, &save.profile_id, &revision).map_err(error)?;
        }
        Some((save.profile_id, revision))
    } else {
        None
    };
    let outcome = asb_switch::rollback_pending_config(
        &FsIo, &state.backup_dir(), pending,
        |backup| {
            settings::capture_current_backup(state, backup)?;
            if let Some((id, revision)) = &profile {
                profile_rollback::restore(state, id, revision)?;
            }
            Ok(())
        },
        || {
            restore_application_snapshot(state, gateway, intent, Some(pending))?;
            store.clear_profile_save()
        },
    ).map_err(restore_failure)?;
    clear(state).map_err(error)?;
    profile_rollback::clear(state).map_err(error)?;
    Ok(outcome)
}

fn restore_failure(failure: SwitchError) -> CommandError {
    match failure {
        SwitchError::PlanRejected { message, .. } => error(message),
        SwitchError::CommitFailed { message, recovery, .. } => {
            let detail = match recovery {
                RecoveryOutcome::RestoreFailed { reason, .. } => format!("{message}。{reason}"),
                _ => message,
            };
            CommandError::new("transaction-backup-restore-failed", detail)
        }
        other => other.into(),
    }
}
