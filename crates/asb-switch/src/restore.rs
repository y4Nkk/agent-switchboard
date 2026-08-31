//! Public restore operations and backup listing.
//!
//! Restoring puts a recorded backup back over its target through the same
//! observable rules as a switch: the lock is taken, the current content is
//! backed up first so the restore itself is reversible, and every failure
//! reports what happened to the live file.

use crate::executor::{
    metadata_path, read_current_or_empty, sha256_hex, timestamp_name, write_backup_metadata,
    RecoveryOutcome, SwitchError, PROCESS_NAME,
};
use crate::io::SwitchIo;
use crate::lockfile::{self, AcquireOutcome};
use asb_core::BackupRecord;
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
    let backup_path = Path::new(&backup.backup_path);
    let Ok(content) = io.read_file(backup_path) else {
        return RecoveryOutcome::RestoreFailed {
            reason: "备份文件不可读".to_string(),
            backup_path: backup.backup_path.clone(),
        };
    };
    if sha256_hex(&content) != backup.content_hash {
        return RecoveryOutcome::RestoreFailed {
            reason: "备份内容与记录哈希不符".to_string(),
            backup_path: backup.backup_path.clone(),
        };
    }
    if !backup.target_existed {
        if let Err(error) = io.remove(target) {
            if error.kind() != ErrorKind::NotFound {
                return RecoveryOutcome::RestoreFailed {
                    reason: format!("移除配置文件失败: {error}"),
                    backup_path: backup.backup_path.clone(),
                };
            }
        }
        return match io.read_file(target) {
            Err(error) if error.kind() == ErrorKind::NotFound => RecoveryOutcome::Restored {
                backup: backup.clone(),
            },
            Ok(_) => RecoveryOutcome::RestoreFailed {
                reason: "移除配置文件后仍可读取内容".to_string(),
                backup_path: backup.backup_path.clone(),
            },
            Err(error) => RecoveryOutcome::RestoreFailed {
                reason: format!("移除后无法确认配置文件状态: {error}"),
                backup_path: backup.backup_path.clone(),
            },
        };
    }
    let file_name = target
        .file_name()
        .expect("target has a file name")
        .to_string_lossy()
        .to_string();
    let temp = target.with_file_name(format!("{file_name}.{}.asb-restore", std::process::id()));
    if let Err(e) = io.write_new_file(&temp, &content) {
        return RecoveryOutcome::RestoreFailed {
            reason: format!("恢复临时文件写入失败: {e}"),
            backup_path: backup.backup_path.clone(),
        };
    }
    if let Err(e) = io.rename_replace(&temp, target) {
        let _ = io.remove(&temp);
        return RecoveryOutcome::RestoreFailed {
            reason: format!("恢复替换失败: {e}"),
            backup_path: backup.backup_path.clone(),
        };
    }
    match io.read_file(target) {
        Ok(restored) if sha256_hex(&restored) == backup.content_hash => RecoveryOutcome::Restored {
            backup: backup.clone(),
        },
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

/// Public restore operation: puts a recorded backup back over its target,
/// taking the lock and backing up the current content first.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOutcome {
    pub pre_restore_backup: BackupRecord,
    pub restored_hash: String,
    /// Warnings added by the desktop command after the configuration write
    /// succeeds, such as an unavailable local audit store.
    pub warnings: Vec<String>,
}

pub fn restore<Io: SwitchIo>(
    io: &Io,
    backup: &BackupRecord,
    target: &Path,
) -> Result<RestoreOutcome, SwitchError> {
    if let Some(parent) = target.parent() {
        io.ensure_dir(parent)
            .map_err(|error| SwitchError::CommitFailed {
                stage: "target-dir",
                message: error.to_string(),
                recovery: RecoveryOutcome::NotNeeded,
            })?;
    }
    match lockfile::acquire(io, target, PROCESS_NAME) {
        AcquireOutcome::Acquired => {}
        AcquireOutcome::Busy(status) => return Err(SwitchError::BlockedByLock { status }),
    }
    let result = (|| {
        let backup_path = Path::new(&backup.backup_path);
        let content = io
            .read_file(backup_path)
            .map_err(|e| SwitchError::ReadCurrent {
                message: format!("备份文件不可读: {e}"),
            })?;
        let restored_hash = sha256_hex(&content);
        if restored_hash != backup.content_hash {
            return Err(SwitchError::ExternalChange {
                expected_hash: backup.content_hash.clone(),
                found_hash: restored_hash,
            });
        }

        // Snapshot whatever is live right now so the restore itself is
        // reversible.
        let (current, target_existed) =
            read_current_or_empty(io, target, backup.app).map_err(|e| {
                SwitchError::ReadCurrent {
                    message: e.to_string(),
                }
            })?;
        let backup_dir = backup_path.parent().expect("backup has a parent dir");
        let ts = timestamp_name(io);
        let file_name = target
            .file_name()
            .expect("target has a file name")
            .to_string_lossy()
            .to_string();
        let pre_path = backup_dir.join(format!("{file_name}.{ts}.prerestore.bak"));
        let pre_record = BackupRecord {
            id: format!("prerestore-{ts}"),
            app: backup.app,
            target_path: target.to_string_lossy().to_string(),
            backup_path: pre_path.to_string_lossy().to_string(),
            created_at: io.now_rfc3339(),
            content_hash: sha256_hex(&current),
            target_existed,
            reason: "restore-precheck".to_string(),
        };
        io.write_new_file(&pre_path, &current)
            .map_err(|e| SwitchError::CommitFailed {
                stage: "restore-precheck",
                message: e.to_string(),
                recovery: RecoveryOutcome::NotNeeded,
            })?;
        write_backup_metadata(io, &pre_record, "restore-meta")?;

        if backup.target_existed {
            let temp =
                target.with_file_name(format!("{file_name}.{}.asb-restore", std::process::id()));
            io.write_new_file(&temp, &content)
                .map_err(|e| SwitchError::CommitFailed {
                    stage: "restore-write",
                    message: e.to_string(),
                    recovery: RecoveryOutcome::NotNeeded,
                })?;
            io.rename_replace(&temp, target).map_err(|e| {
                let _ = io.remove(&temp);
                SwitchError::CommitFailed {
                    stage: "restore-replace",
                    message: e.to_string(),
                    recovery: RecoveryOutcome::NotNeeded,
                }
            })?;
        } else if target_existed {
            io.remove(target).map_err(|e| SwitchError::CommitFailed {
                stage: "restore-replace",
                message: e.to_string(),
                recovery: RecoveryOutcome::NotNeeded,
            })?;
        }

        let restored = match io.read_file(target) {
            Ok(text) => backup.target_existed && sha256_hex(&text) == restored_hash,
            Err(error) if error.kind() == ErrorKind::NotFound => !backup.target_existed,
            Err(_) => false,
        };
        if restored {
            Ok(RestoreOutcome {
                pre_restore_backup: pre_record,
                restored_hash,
                warnings: vec![],
            })
        } else {
            let recovery = restore_backup_content(io, target, &pre_record);
            Err(SwitchError::CommitFailed {
                stage: "restore-verify",
                message: "恢复后校验失败".to_string(),
                recovery,
            })
        }
    })();
    lockfile::release(io, target);
    result
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
