//! Durable intent for a client projection and its optional Codex auth sibling.
use crate::{io::SwitchIo, RecoveryOutcome, SwitchError};
use asb_core::AppKind;
use asb_core::BackupRecord;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PendingConfigWrite {
    pub version: u8,
    pub app: AppKind,
    pub profile_id: Option<String>,
    pub backup: BackupRecord,
    pub after_hash: String,
    pub after_existed: bool,
    #[serde(default)]
    pub auth: Option<PendingAuthWrite>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PendingAuthWrite {
    pub backup: BackupRecord,
    pub after_hash: String,
    pub after_existed: bool,
}

pub fn config_journal_path(directory: &Path, app: AppKind) -> PathBuf {
    directory.join(match app {
        AppKind::Codex => "pending-codex.json",
        AppKind::Claude => "pending-claude.json",
    })
}

pub fn pending_config_write<Io: SwitchIo>(
    io: &Io,
    directory: &Path,
    app: AppKind,
) -> Result<Option<PendingConfigWrite>, SwitchError> {
    let path = config_journal_path(directory, app);
    let text = match io.read_file(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(recovery_error("无法读取未完成配置事务", &path)),
    };
    let pending: PendingConfigWrite =
        serde_json::from_str(&text).map_err(|_| recovery_error("未完成配置事务格式无效", &path))?;
    if pending.version != 1
        || pending.app != app
        || pending.backup.app != app
        || Path::new(&pending.backup.backup_path).parent() != Some(directory)
        || pending.backup.linked_backup_id.is_some()
    {
        return Err(recovery_error("未完成配置事务身份不匹配", &path));
    }
    if let Some(auth) = &pending.auth {
        let auth_target = Path::new(&auth.backup.target_path);
        let expected_auth_target =
            Path::new(&pending.backup.target_path).with_file_name("auth.json");
        if pending.app != AppKind::Codex
            || auth.backup.app != AppKind::Codex
            || auth.backup.linked_backup_id.as_deref() != Some(pending.backup.id.as_str())
            || auth_target != expected_auth_target
            || Path::new(&auth.backup.backup_path).parent() != Some(directory)
        {
            return Err(recovery_error("未完成认证事务身份不匹配", &path));
        }
    }
    Ok(Some(pending))
}

pub(crate) fn require_clear<Io: SwitchIo>(
    io: &Io,
    directory: &Path,
    app: AppKind,
) -> Result<(), SwitchError> {
    if pending_config_write(io, directory, app)?.is_some() {
        return Err(recovery_error(
            "请先恢复未完成配置事务，再执行新的写入",
            &config_journal_path(directory, app),
        ));
    }
    Ok(())
}

pub(crate) fn track<Io: SwitchIo, T>(
    io: &Io,
    pending: PendingConfigWrite,
    execute: impl FnOnce() -> Result<T, SwitchError>,
) -> Result<T, SwitchError> {
    let directory = Path::new(&pending.backup.backup_path)
        .parent()
        .expect("backup directory");
    require_clear(io, directory, pending.app)?;
    let path = config_journal_path(directory, pending.app);
    let text = serde_json::to_string(&pending).expect("journal serializes");
    io.sync_file(Path::new(&pending.backup.backup_path))
        .map_err(|_| recovery_error("配置事务备份无法持久化", &path))?;
    if let Some(auth) = &pending.auth {
        io.sync_file(Path::new(&auth.backup.backup_path))
            .map_err(|_| recovery_error("认证事务备份无法持久化", &path))?;
    }
    io.write_new_file(&path, &text)
        .and_then(|_| io.sync_file(&path))
        .and_then(|_| io.sync_dir(directory))
        .map_err(|_| recovery_error("无法持久化配置事务；尚未写入配置", &path))?;
    let result = match execute() {
        Ok(result) => result,
        Err(failure) => {
            if matches!(
                &failure,
                SwitchError::ExternalChange { .. }
                    | SwitchError::CommitFailed {
                        recovery: RecoveryOutcome::NotNeeded,
                        ..
                    }
            ) {
                io.remove(&path)
                    .map_err(|_| recovery_error("未写入配置，但事务清理失败", &path))?;
            }
            return Err(failure);
        }
    };
    let target = Path::new(&pending.backup.target_path);
    if pending.after_existed {
        io.sync_file(target)
            .map_err(|_| recovery_error("配置提交尚未持久化，保留恢复记录", &path))?;
    }
    if let Some(auth) = &pending.auth {
        if auth.after_existed {
            io.sync_file(Path::new(&auth.backup.target_path))
                .map_err(|_| recovery_error("认证提交尚未持久化，保留恢复记录", &path))?;
        }
    }
    io.sync_dir(target.parent().expect("target directory"))
        .map_err(|_| recovery_error("配置目录提交尚未持久化，保留恢复记录", &path))?;
    io.remove(&path)
        .and_then(|_| io.sync_dir(directory))
        .map_err(|_| recovery_error("配置已提交，但事务清理未完成", &path))?;
    Ok(result)
}

/// A recovery never guesses when a user edited the file after the transaction.
pub fn finish_config_recovery<Io: SwitchIo>(
    io: &Io,
    directory: &Path,
    pending: &PendingConfigWrite,
    expected_hash: &str,
    expected_existed: bool,
    commit: impl FnOnce() -> Result<(), String>,
) -> Result<(), SwitchError> {
    let target = Path::new(&pending.backup.target_path);
    let path = config_journal_path(directory, pending.app);
    match crate::lockfile::acquire(io, target, crate::executor::PROCESS_NAME) {
        crate::AcquireOutcome::Acquired => {}
        crate::AcquireOutcome::Busy(status) => return Err(SwitchError::BlockedByLock { status }),
    }
    let result = (|| {
        let (current, existed) = crate::executor::read_current_or_empty(io, target, pending.app)
            .map_err(|_| recovery_error("无法读取恢复目标", &path))?;
        if existed != expected_existed || crate::sha256_hex(&current) != expected_hash {
            return Err(recovery_error(
                "配置已发生额外变化，请从事务备份显式恢复",
                &path,
            ));
        }
        if let Some((auth_hash, auth_existed)) =
            auth_expected_state(pending, expected_hash, expected_existed)
        {
            verify_sibling_snapshot(io, pending, auth_hash, auth_existed, &path)?;
        }
        commit().map_err(|message| recovery_error(&message, &path))?;
        sync_target(io, target, expected_existed, &path)?;
        sync_auth_target(io, pending, &path)?;
        io.remove(&path)
            .and_then(|_| io.sync_dir(directory))
            .map_err(|_| recovery_error("恢复已完成，但事务清理失败", &path))
    })();
    let released = crate::lockfile::release(io, target);
    match (result, released) {
        (result, Ok(())) => result,
        (Err(prior), Err(message)) => Err(SwitchError::LockReleaseFailed {
            message,
            prior: Box::new(prior),
        }),
        (Ok(()), Err(message)) => Err(recovery_error(&message, &path)),
    }
}

/// Explicit compensation for the exact pending backup. Prepare pairs application
/// state with the pre-restore backup before any live file is restored.
pub fn rollback_pending_config<Io: SwitchIo>(
    io: &Io,
    directory: &Path,
    pending: &PendingConfigWrite,
    prepare: impl FnOnce(&BackupRecord) -> Result<(), String>,
    commit: impl FnOnce() -> Result<(), String>,
) -> Result<crate::RestoreOutcome, SwitchError> {
    let target = Path::new(&pending.backup.target_path);
    match crate::lockfile::acquire(io, target, crate::executor::PROCESS_NAME) {
        crate::AcquireOutcome::Acquired => {}
        crate::AcquireOutcome::Busy(status) => return Err(SwitchError::BlockedByLock { status }),
    }
    let journal_path = config_journal_path(directory, pending.app);
    let result = (|| {
        let content = crate::restore::read_verified_rollback_backup(io, &pending.backup)
            .map_err(|reason| recovery_error(reason, &journal_path))?;
        let auth = match &pending.auth {
            Some(auth) => {
                let content = crate::restore::read_verified_rollback_backup(io, &auth.backup)
                    .map_err(|reason| recovery_error(&format!("认证备份校验失败。{reason}"), &journal_path))?;
                Some((auth, content))
            }
            None => None,
        };
        let (current, existed) = crate::executor::read_current_or_empty(io, target, pending.app)
            .map_err(|_| recovery_error("无法读取显式补偿前的配置", &journal_path))?;
        let pre_restore_backup = crate::restore::snapshot_before_restore(
            io,
            &pending.backup,
            target,
            &current,
            existed,
        )?;
        prepare(&pre_restore_backup).map_err(|message| recovery_error(&message, &journal_path))?;
        let restored = crate::restore::restore_verified_backup(io, target, &pending.backup, &content);
        if !matches!(restored, RecoveryOutcome::Restored { .. }) {
            return Err(SwitchError::CommitFailed {
                stage: "transaction-recovery",
                message: "事务配置补偿未完成".into(),
                recovery: restored,
            });
        }
        if let Some((auth, content)) = auth {
            let restored_auth = crate::restore::restore_verified_backup(
                io,
                Path::new(&auth.backup.target_path),
                &auth.backup,
                &content,
            );
            if !matches!(restored_auth, RecoveryOutcome::Restored { .. }) {
                return Err(SwitchError::CommitFailed {
                    stage: "transaction-recovery",
                    message: "事务认证补偿未完成".into(),
                    recovery: restored_auth,
                });
            }
        }
        commit().map_err(|message| recovery_error(&message, &journal_path))?;
        sync_target(io, target, pending.backup.target_existed, &journal_path)?;
        sync_auth_target(io, pending, &journal_path)?;
        io.remove(&journal_path)
            .and_then(|_| io.sync_dir(directory))
            .map_err(|_| recovery_error("事务补偿清理失败", &journal_path))?;
        Ok(crate::RestoreOutcome {
            pre_restore_backup,
            restored_hash: pending.backup.content_hash.clone(),
            warnings: vec![],
        })
    })();
    match crate::lockfile::release(io, target) {
        Ok(()) => result,
        Err(message) => Err(recovery_error(&message, &journal_path)),
    }
}

fn sync_target<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    existed: bool,
    journal: &Path,
) -> Result<(), SwitchError> {
    if existed {
        io.sync_file(target)
            .map_err(|_| recovery_error("恢复配置尚未持久化", journal))?;
    }
    io.sync_dir(target.parent().expect("target directory"))
        .map_err(|_| recovery_error("恢复目录尚未持久化", journal))
}

fn auth_expected_state<'a>(
    pending: &'a PendingConfigWrite,
    expected_hash: &str,
    expected_existed: bool,
) -> Option<(&'a str, bool)> {
    let auth = pending.auth.as_ref()?;
    if pending.after_hash == expected_hash && pending.after_existed == expected_existed {
        return Some((&auth.after_hash, auth.after_existed));
    }
    if pending.backup.content_hash == expected_hash
        && pending.backup.target_existed == expected_existed
    {
        return Some((&auth.backup.content_hash, auth.backup.target_existed));
    }
    None
}

fn verify_sibling_snapshot<Io: SwitchIo>(
    io: &Io,
    pending: &PendingConfigWrite,
    expected_hash: &str,
    expected_existed: bool,
    journal: &Path,
) -> Result<(), SwitchError> {
    let Some(auth) = pending.auth.as_ref() else {
        return Ok(());
    };
    let target = Path::new(&auth.backup.target_path);
    let (content, existed) = match io.read_file(target) {
        Ok(content) => (content, true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (String::new(), false),
        Err(_) => return Err(recovery_error("无法读取恢复认证目标", journal)),
    };
    if existed == expected_existed && crate::sha256_hex(&content) == expected_hash {
        return Ok(());
    }
    Err(recovery_error(
        "认证文件已发生额外变化，请从事务备份显式恢复",
        journal,
    ))
}

fn sync_auth_target<Io: SwitchIo>(
    io: &Io,
    pending: &PendingConfigWrite,
    journal: &Path,
) -> Result<(), SwitchError> {
    let Some(auth) = pending.auth.as_ref() else {
        return Ok(());
    };
    let target = Path::new(&auth.backup.target_path);
    match io.read_file(target) {
        Ok(_) => io
            .sync_file(target)
            .map_err(|_| recovery_error("认证文件尚未持久化", journal))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(recovery_error("无法确认认证文件持久化状态", journal)),
    }
    Ok(())
}

fn recovery_error(message: &str, path: &Path) -> SwitchError {
    SwitchError::PlanRejected {
        message: format!("{message}；恢复记录：{}", path.display()),
        line: None,
    }
}
