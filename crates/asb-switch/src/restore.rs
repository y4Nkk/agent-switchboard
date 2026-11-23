//! Restore helpers and backup listing.
//!
//! Restoring puts a recorded backup back over its target through the same
//! observable rules as a switch. The public transaction entry point lives in
//! `executor`; this module only supplies its locked restore body and recovery
//! primitives.

mod codex_auth;
mod transaction;
#[cfg(test)]
mod tests;
use codex_auth::{
    linked_auth_backup, read_optional_file, read_verified_auth_source, write_auth_restore_candidate,
};
pub(crate) use transaction::restore_locked;

use crate::executor::{
    metadata_path, read_current_or_empty, sha256_hex, timestamp_name, verify_live_snapshot,
    write_backup_metadata, RecoveryOutcome, SwitchError,
};
use crate::io::SwitchIo;
use asb_core::{adapter, BackupRecord};
use serde::Serialize;
use std::io::ErrorKind;
use std::path::Path;

/// Puts one recorded backup's content back over its target, restoring a
/// missing target when the backup predates the file. Returns
/// [`RecoveryOutcome::RestoreFailed`] instead of panicking on any IO error.
pub(crate) fn restore_backup_content<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    backup: &BackupRecord,
) -> RecoveryOutcome {
    match read_verified_rollback_backup(io, backup) {
        Ok(content) => restore_verified_backup(io, target, backup, &content),
        Err(reason) => RecoveryOutcome::RestoreFailed {
            reason: reason.into(),
            backup_path: backup.backup_path.clone(),
        },
    }
}

pub(crate) fn read_verified_rollback_backup<Io: SwitchIo>(
    io: &Io,
    backup: &BackupRecord,
) -> Result<String, &'static str> {
    let backup_path = Path::new(&backup.backup_path);
    let content = io.read_file(backup_path).map_err(|_| "备份文件不可读")?;
    if sha256_hex(&content) != backup.content_hash {
        return Err("备份内容与记录哈希不符");
    }
    Ok(content)
}

pub(crate) fn restore_verified_backup<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    backup: &BackupRecord,
    content: &str,
) -> RecoveryOutcome {
    if !backup.target_existed {
        return restore_absent_target(io, target, backup);
    }
    let temp = match stage_restore_file(io, target, content) {
        Ok(temp) => temp,
        Err(reason) => {
            return RecoveryOutcome::RestoreFailed {
                reason,
                backup_path: backup.backup_path.clone(),
            }
        }
    };
    if let Err(e) = io.rename_replace(&temp, target) {
        let _ = io.remove(&temp);
        return RecoveryOutcome::RestoreFailed {
            reason: format!("恢复替换失败: {e}"),
            backup_path: backup.backup_path.clone(),
        };
    }
    if let Err(error) = io.sync_dir(target.parent().expect("restore target parent")) {
        return RecoveryOutcome::RestoreFailed {
            reason: format!("恢复文件已替换，但目录同步失败：{error}"),
            backup_path: backup.backup_path.clone(),
        };
    }
    match io.read_file(target) {
        Ok(restored)
            if sha256_hex(&restored) == backup.content_hash
                // A rollback must restore its exact preceding bytes even if
                // that preceding file was already invalid. Normal switch and
                // restore candidates are validated before replacement.
                && restored == content =>
        {
            RecoveryOutcome::Restored {
                backup: backup.clone(),
            }
        }
        Ok(_) => RecoveryOutcome::RestoreFailed {
            reason: "恢复后校验失败".to_string(),
            backup_path: backup.backup_path.clone(),
        },
        Err(e) => RecoveryOutcome::RestoreFailed {
            reason: format!("恢复后读取失败: {e}"),
            backup_path: backup.backup_path.clone(),
        },
    }
}

fn restore_absent_target<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    backup: &BackupRecord,
) -> RecoveryOutcome {
    if let Err(error) = io.remove(target) {
        if error.kind() != ErrorKind::NotFound {
            return RecoveryOutcome::RestoreFailed {
                reason: format!("移除配置文件失败: {error}"),
                backup_path: backup.backup_path.clone(),
            };
        }
    }
    match io.read_file(target) {
        Err(error) if error.kind() == ErrorKind::NotFound => RecoveryOutcome::Restored {
            backup: backup.clone(),
        },
        Ok(_) => RecoveryOutcome::RestoreFailed {
            reason: "移除配置文件后仍可读取内容".into(),
            backup_path: backup.backup_path.clone(),
        },
        Err(error) => RecoveryOutcome::RestoreFailed {
            reason: format!("移除后无法确认配置文件状态: {error}"),
            backup_path: backup.backup_path.clone(),
        },
    }
}

/// Public restore operation: puts a recorded backup back over its target,
/// taking the lock and backing up the current content first.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOutcome {
    pub pre_restore_backup: BackupRecord,
    pub restored_hash: String,
    /// Observable non-rollback warnings, such as a failed lock release after
    /// the state record has committed.
    pub warnings: Vec<asb_core::contracts::LocalizedMessage>,
}

fn read_verified_restore_source<Io: SwitchIo>(
    io: &Io,
    backup: &BackupRecord,
) -> Result<String, SwitchError> {
    let content = io
        .read_file(Path::new(&backup.backup_path))
        .map_err(|error| SwitchError::CommitFailed {
            stage: "restore-backup-verify",
            message: format!("备份文件不可读: {error}"),
            recovery: RecoveryOutcome::NotNeeded,
        })?;
    if sha256_hex(&content) != backup.content_hash {
        return Err(SwitchError::ExternalChange {
            expected_hash: backup.content_hash.clone(),
            found_hash: sha256_hex(&content),
        });
    }
    adapter::validate_syntax(backup.app, &content).map_err(|_| SwitchError::CommitFailed {
        stage: "restore-backup-verify",
        message: "待恢复备份格式无效".to_string(),
        recovery: RecoveryOutcome::NotNeeded,
    })?;
    validate_restore_contract(backup, &content)?;
    Ok(content)
}

fn validate_restore_contract(backup: &BackupRecord, content: &str) -> Result<(), SwitchError> {
    if backup.app != asb_core::AppKind::Codex {
        return Ok(());
    }
    adapter::codex::validate_restore_configuration(content).map_err(|error| {
        SwitchError::PlanRejected {
            message: error.message,
            line: error.line,
        }
    })
}

fn write_pre_restore_backup<Io: SwitchIo>(
    io: &Io,
    pre_path: &Path,
    current: &str,
    record: &BackupRecord,
) -> Result<(), SwitchError> {
    io.write_new_file(pre_path, current)
        .map_err(|error| SwitchError::CommitFailed {
            stage: "restore-precheck",
            message: error.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        })?;
    if Path::new(&record.target_path)
        .file_name()
        .is_some_and(|name| name == "auth.json")
    {
        io.set_mode(pre_path, 0o600)
            .map_err(|error| SwitchError::CommitFailed {
                stage: "auth-backup-permissions",
                message: error.to_string(),
                recovery: RecoveryOutcome::NotNeeded,
            })?;
    }
    match io.read_file(pre_path) {
        Ok(read_back) if read_back == current => {}
        Ok(_) => {
            return Err(SwitchError::CommitFailed {
                stage: "restore-precheck-verify",
                message: "恢复前备份回读内容不匹配".to_string(),
                recovery: RecoveryOutcome::NotNeeded,
            });
        }
        Err(error) => {
            return Err(SwitchError::CommitFailed {
                stage: "restore-precheck-verify",
                message: error.to_string(),
                recovery: RecoveryOutcome::NotNeeded,
            });
        }
    }
    write_backup_metadata(io, record, "restore-meta")
}

fn write_restore_candidate<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    app: asb_core::AppKind,
    content: &str,
    current: &str,
    target_existed: bool,
) -> Result<(), SwitchError> {
    let file_name = target
        .file_name()
        .expect("target has a file name")
        .to_string_lossy()
        .to_string();
    let temp = target.with_file_name(format!("{file_name}.{}.asb-restore", std::process::id()));
    io.write_new_file(&temp, content)
        .map_err(|error| SwitchError::CommitFailed {
            stage: "restore-write",
            message: error.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        })?;
    let temp_content = match io.read_file(&temp) {
        Ok(read_back) if read_back == content => read_back,
        Ok(_) => {
            let _ = io.remove(&temp);
            return Err(SwitchError::CommitFailed {
                stage: "restore-temp-verify",
                message: "恢复临时文件回读内容不匹配".to_string(),
                recovery: RecoveryOutcome::NotNeeded,
            });
        }
        Err(error) => {
            let _ = io.remove(&temp);
            return Err(SwitchError::CommitFailed {
                stage: "restore-temp-verify",
                message: error.to_string(),
                recovery: RecoveryOutcome::NotNeeded,
            });
        }
    };
    if adapter::validate_syntax(app, &temp_content).is_err() {
        let _ = io.remove(&temp);
        return Err(SwitchError::CommitFailed {
            stage: "restore-temp-validate",
            message: "恢复临时文件格式无效".to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        });
    }
    if let Err(error) = verify_live_snapshot(io, target, app, current, target_existed) {
        let _ = io.remove(&temp);
        return Err(error);
    }
    io.rename_replace(&temp, target).map_err(|error| {
        let _ = io.remove(&temp);
        SwitchError::CommitFailed {
            stage: "restore-replace",
            message: error.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        }
    })
}

pub(crate) fn snapshot_before_restore<Io: SwitchIo>(
    io: &Io,
    backup: &BackupRecord,
    target: &Path,
    current: &str,
    target_existed: bool,
) -> Result<BackupRecord, SwitchError> {
    let backup_path = Path::new(&backup.backup_path);
    let backup_dir = backup_path.parent().expect("backup has a parent dir");
    let timestamp = timestamp_name(io);
    let file_name = target
        .file_name()
        .expect("target has a file name")
        .to_string_lossy()
        .to_string();
    let pre_path = backup_dir.join(format!("{file_name}.{timestamp}.prerestore.bak"));
    let pre_record = BackupRecord {
        id: format!("prerestore-{timestamp}"),
        app: backup.app,
        target_path: target.to_string_lossy().to_string(),
        backup_path: pre_path.to_string_lossy().to_string(),
        created_at: io.now_rfc3339(),
        content_hash: sha256_hex(current),
        target_existed,
        linked_backup_id: None,
        reason: "restore-precheck".to_string(),
    };
    write_pre_restore_backup(io, &pre_path, current, &pre_record)?;

    Ok(pre_record)
}

/// Lists backup records for one target from the sidecar metadata files.
pub fn list_backups<Io: SwitchIo>(io: &Io, backup_dir: &Path) -> Vec<BackupRecord> {
    let mut records = Vec::new();
    let Ok(entries) = io.list_dir(backup_dir) else {
        return records;
    };
    let mut paths: Vec<std::path::PathBuf> = entries
        .into_iter()
        .filter(|p| p.to_string_lossy().ends_with(".meta.json"))
        .collect();
    paths.sort();
    for path in paths {
        if let Ok(text) = io.read_file(&path) {
            if let Ok(record) = serde_json::from_str::<BackupRecord>(&text) {
                // The record must describe the sidecar that carried it. A
                // hand-written metadata file cannot redirect a restore to an
                // arbitrary path outside this backup directory.
                if metadata_path(Path::new(&record.backup_path)) == path {
                    records.push(record);
                }
            }
        }
    }
    records
}

pub(crate) fn restore_backup_if_unchanged<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    before: &BackupRecord,
    after: &str,
    after_existed: bool,
) -> RecoveryOutcome {
    let current = read_optional_file(io, target);
    if let Ok((text, existed)) = &current {
        if *existed == before.target_existed && sha256_hex(text) == before.content_hash {
            return RecoveryOutcome::NotNeeded;
        }
        if *existed == after_existed && text == after {
            return restore_backup_content(io, target, before);
        }
    }
    RecoveryOutcome::RestoreFailed {
        reason: "配置或认证已在恢复期间外改，已保留外部内容".into(),
        backup_path: before.backup_path.clone(),
    }
}

fn stage_restore_file<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    content: &str,
) -> Result<std::path::PathBuf, String> {
    let name = target
        .file_name()
        .ok_or("恢复目标缺少文件名")?
        .to_string_lossy();
    let temp = target.with_file_name(format!("{name}.{}.asb-restore", std::process::id()));
    io.write_new_file(&temp, content)
        .map_err(|e| format!("恢复临时文件写入失败：{e}"))?;
    let prepared = (|| {
        if name == "auth.json" {
            io.set_mode(&temp, 0o600).map_err(|e| e.to_string())?;
        }
        if io.read_file(&temp).map_err(|e| e.to_string())? != content {
            return Err("恢复临时文件回读内容不匹配".to_string());
        }
        io.sync_file(&temp).map_err(|e| e.to_string())
    })();
    if let Err(error) = prepared {
        let _ = io.remove(&temp);
        return Err(error);
    }
    Ok(temp)
}
