//! The switch executor: the only layer allowed to write live configuration
//! files.
//!
//! One switch walks: lock → read & hash → external-change check → preview →
//! render → re-check → backup → temporary write → syntax validation → atomic
//! replacement → post-write verification → lock release. Any failure after
//! the file has been replaced restores the immediately preceding backup.

use crate::io::SwitchIo;
use crate::lockfile::{self, AcquireOutcome};
use asb_core::{adapter, AdapterError, BackupRecord, LockStatus, SwitchPlan, SwitchPreview};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub const PROCESS_NAME: &str = "agent-switchboard";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    pub preview: SwitchPreview,
    pub content_hash: String,
    /// Hash of the exact candidate shown to the user. Execution refuses a
    /// plan whose rendering changed after this preview.
    pub rendered_hash: String,
}

/// What happened to the live file after a failure.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum RecoveryOutcome {
    /// The live file was never replaced; no restore was needed.
    NotNeeded,
    /// The pre-switch backup was restored over a failed write.
    Restored { backup: BackupRecord },
    /// Restoration itself failed; the backup path is reported loudly.
    RestoreFailed { reason: String, backup_path: String },
}

/// Structured execution result. The UI never has to infer any of this.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchOutcome {
    /// Lock acquisition report; the lock is released before returning.
    pub lock: LockStatus,
    pub acquired_at: String,
    pub changed: Vec<String>,
    pub warnings: Vec<String>,
    pub backup: BackupRecord,
    pub preview: SwitchPreview,
    pub recovery: RecoveryOutcome,
    /// SHA-256 hex digest of the file content that is now live; the switch
    /// log records it so later external edits are detectable.
    pub final_hash: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SwitchError {
    /// The target could not be read before planning.
    ReadCurrent { message: String },
    /// Validation or adapter parsing rejected the plan/current content.
    PlanRejected {
        message: String,
        line: Option<usize>,
    },
    /// A lock blocked the switch. Nothing was changed.
    BlockedByLock { status: LockStatus },
    /// The file changed since the preview was produced. Nothing was changed.
    ExternalChange {
        expected_hash: String,
        found_hash: String,
    },
    /// The selected profile or common settings changed after preview. Nothing
    /// was changed because the user has not confirmed the new candidate.
    PlanChanged,
    /// A write-path stage failed; `recovery` says what happened to the file.
    CommitFailed {
        stage: &'static str,
        message: String,
        recovery: RecoveryOutcome,
    },
}

impl std::fmt::Display for SwitchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwitchError::ReadCurrent { .. } => write!(f, "无法读取当前配置"),
            SwitchError::PlanRejected { message, .. } => write!(f, "配置计划无效：{message}"),
            SwitchError::BlockedByLock { status } => {
                write!(f, "切换被锁阻塞：{}", lock_status_label(status))
            }
            SwitchError::ExternalChange { .. } => {
                write!(f, "配置在预览之后被外部修改，已阻止切换")
            }
            SwitchError::PlanChanged => write!(f, "供应商或通用设置已变更，请重新查看差异"),
            SwitchError::CommitFailed {
                stage, recovery, ..
            } => {
                write!(
                    f,
                    "写入阶段“{}”失败；恢复结果：{}",
                    stage_label(stage),
                    recovery_label(recovery)
                )
            }
        }
    }
}

impl std::error::Error for SwitchError {}

fn lock_status_label(status: &LockStatus) -> &'static str {
    match status {
        LockStatus::Free => "无锁",
        LockStatus::Held(_) => "有进程正在持锁",
        LockStatus::Stale(_) => "发现遗留锁",
        LockStatus::Indeterminate { .. } => "锁状态无法确定",
    }
}

fn stage_label(stage: &str) -> &'static str {
    match stage {
        "backup-dir" => "准备备份目录",
        "backup" => "创建备份",
        "backup-meta" => "记录备份信息",
        "target-dir" => "准备配置目录",
        "temp-write" => "写入临时文件",
        "temp-validate" => "校验临时文件",
        "atomic-replace" => "替换配置文件",
        "post-verify" => "验证写入结果",
        "restore-precheck" => "创建恢复前备份",
        "restore-meta" => "记录恢复前备份信息",
        "restore-write" => "写入恢复临时文件",
        "restore-replace" => "替换恢复内容",
        "restore-verify" => "验证恢复结果",
        _ => "提交配置",
    }
}

fn recovery_label(recovery: &RecoveryOutcome) -> &'static str {
    match recovery {
        RecoveryOutcome::NotNeeded => "无需恢复",
        RecoveryOutcome::Restored { .. } => "已恢复上一份备份",
        RecoveryOutcome::RestoreFailed { .. } => "恢复失败，请使用备份文件手动恢复",
    }
}

pub fn sha256_hex(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

pub struct SwitchRequest<'a> {
    pub target: &'a Path,
    pub plan: &'a SwitchPlan,
    pub backup_dir: &'a Path,
    /// Hash captured when the user saw the preview; a mismatch blocks the
    /// switch.
    pub expected_hash: &'a str,
    /// Hash of the candidate file shown in the preview.
    pub expected_rendered_hash: &'a str,
}

fn empty_configuration(app: asb_core::AppKind) -> &'static str {
    match app {
        asb_core::AppKind::Codex => "",
        asb_core::AppKind::Claude => "{}",
    }
}

fn read_current_or_empty<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    app: asb_core::AppKind,
) -> Result<(String, bool), std::io::Error> {
    match io.read_file(target) {
        Ok(text) => Ok((text, true)),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            Ok((empty_configuration(app).to_string(), false))
        }
        Err(error) => Err(error),
    }
}

fn metadata_path(backup_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.meta.json", backup_path.to_string_lossy()))
}

fn write_backup_metadata<Io: SwitchIo>(
    io: &Io,
    record: &BackupRecord,
    stage: &'static str,
) -> Result<(), SwitchError> {
    let meta = serde_json::to_string(record).expect("backup record serializes");
    io.write_file_replace(&metadata_path(Path::new(&record.backup_path)), &meta)
        .map_err(|error| SwitchError::CommitFailed {
            stage,
            message: error.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        })
}

/// Reads the target and produces the side-effect-free preview plus the
/// content hash the executor will later verify.
pub fn read_preview<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    plan: &SwitchPlan,
    backup_dir: &str,
) -> Result<FilePreview, SwitchError> {
    let (current, target_existed) =
        read_current_or_empty(io, target, plan.app).map_err(|e| SwitchError::ReadCurrent {
            message: e.to_string(),
        })?;
    let content_hash = sha256_hex(&current);
    let mut preview = adapter::preview(&current, plan, backup_dir).map_err(plan_rejected)?;
    if !target_existed {
        preview
            .warnings
            .push("配置文件尚不存在，确认后将创建新的用户级配置".to_string());
    }
    let rendered_hash = sha256_hex(&adapter::render(&current, plan).map_err(plan_rejected)?);
    Ok(FilePreview {
        preview,
        content_hash,
        rendered_hash,
    })
}

fn plan_rejected(e: AdapterError) -> SwitchError {
    SwitchError::PlanRejected {
        message: e.message,
        line: e.line,
    }
}

fn timestamp_name<Io: SwitchIo>(io: &Io) -> String {
    io.now_rfc3339()
        .chars()
        .filter(|c| c.is_ascii_digit())
        .take(17)
        .collect()
}

/// Executes one switch transactionally. All exit paths release the lock.
pub fn execute<Io: SwitchIo>(io: &Io, req: &SwitchRequest) -> Result<SwitchOutcome, SwitchError> {
    if let Some(parent) = req.target.parent() {
        io.ensure_dir(parent)
            .map_err(|error| SwitchError::CommitFailed {
                stage: "target-dir",
                message: error.to_string(),
                recovery: RecoveryOutcome::NotNeeded,
            })?;
    }
    match lockfile::acquire(io, req.target, PROCESS_NAME) {
        AcquireOutcome::Acquired => {}
        AcquireOutcome::Busy(status) => return Err(SwitchError::BlockedByLock { status }),
    }
    execute_locked(io, req)
}

fn execute_locked<Io: SwitchIo>(
    io: &Io,
    req: &SwitchRequest,
) -> Result<SwitchOutcome, SwitchError> {
    let finish = |result| {
        lockfile::release(io, req.target);
        result
    };

    let (current, target_existed) = match read_current_or_empty(io, req.target, req.plan.app) {
        Ok(current) => current,
        Err(e) => {
            return finish(Err(SwitchError::ReadCurrent {
                message: e.to_string(),
            }));
        }
    };
    let found_hash = sha256_hex(&current);
    if found_hash != req.expected_hash {
        return finish(Err(SwitchError::ExternalChange {
            expected_hash: req.expected_hash.to_string(),
            found_hash,
        }));
    }

    let backup_dir_label = req.backup_dir.to_string_lossy().to_string();
    let preview = match adapter::preview(&current, req.plan, &backup_dir_label) {
        Ok(p) => p,
        Err(e) => return finish(Err(plan_rejected(e))),
    };
    let rendered = match adapter::render(&current, req.plan) {
        Ok(r) => r,
        Err(e) => return finish(Err(plan_rejected(e))),
    };
    if sha256_hex(&rendered) != req.expected_rendered_hash {
        return finish(Err(SwitchError::PlanChanged));
    }

    // Re-check immediately before mutation: the file must still hash to the
    // previewed state.
    match read_current_or_empty(io, req.target, req.plan.app) {
        Ok((text, _)) => {
            let found = sha256_hex(&text);
            if found != req.expected_hash {
                return finish(Err(SwitchError::ExternalChange {
                    expected_hash: req.expected_hash.to_string(),
                    found_hash: found,
                }));
            }
        }
        Err(e) => {
            return finish(Err(SwitchError::ReadCurrent {
                message: e.to_string(),
            }));
        }
    }

    if let Err(e) = io.ensure_dir(req.backup_dir) {
        return finish(Err(SwitchError::CommitFailed {
            stage: "backup-dir",
            message: e.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        }));
    }

    let ts = timestamp_name(io);
    let file_name = req
        .target
        .file_name()
        .expect("target has a file name")
        .to_string_lossy()
        .to_string();
    let backup_path = req.backup_dir.join(format!("{file_name}.{ts}.bak"));
    let created_at = io.now_rfc3339();
    if let Err(e) = io.write_new_file(&backup_path, &current) {
        return finish(Err(SwitchError::CommitFailed {
            stage: "backup",
            message: e.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        }));
    }
    let backup = BackupRecord {
        id: format!("{}-{ts}", &found_hash[..12.min(found_hash.len())]),
        app: req.plan.app,
        target_path: req.target.to_string_lossy().to_string(),
        backup_path: backup_path.to_string_lossy().to_string(),
        created_at,
        content_hash: found_hash.clone(),
        target_existed,
        reason: "switch".to_string(),
    };
    if let Err(error) = write_backup_metadata(io, &backup, "backup-meta") {
        return finish(Err(error));
    }

    let temp_path =
        req.target
            .with_file_name(format!("{}.{}.asb-tmp", file_name, std::process::id()));
    if let Err(e) = io.write_new_file(&temp_path, &rendered) {
        return finish(Err(SwitchError::CommitFailed {
            stage: "temp-write",
            message: e.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        }));
    }

    if let Err(e) = adapter::validate_syntax(req.plan.app, &rendered) {
        let _ = io.remove(&temp_path);
        return finish(Err(SwitchError::CommitFailed {
            stage: "temp-validate",
            message: format!("临时文件校验失败: {e}"),
            recovery: RecoveryOutcome::NotNeeded,
        }));
    }

    if let Err(e) = io.rename_replace(&temp_path, req.target) {
        let _ = io.remove(&temp_path);
        return finish(Err(SwitchError::CommitFailed {
            stage: "atomic-replace",
            message: e.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        }));
    }

    // Post-write verification: the live file must equal the rendered
    // candidate and parse cleanly. Otherwise restore the backup.
    let verified = match io.read_file(req.target) {
        Ok(text) => text == rendered && adapter::validate_syntax(req.plan.app, &text).is_ok(),
        Err(_) => false,
    };
    if !verified {
        let recovery = restore_backup_content(io, req.target, &backup);
        return finish(Err(SwitchError::CommitFailed {
            stage: "post-verify",
            message: "替换后校验失败".to_string(),
            recovery,
        }));
    }

    finish(Ok(SwitchOutcome {
        lock: LockStatus::Free,
        acquired_at: backup.created_at.clone(),
        changed: vec![req.target.to_string_lossy().to_string()],
        warnings: preview.warnings.clone(),
        backup,
        preview,
        recovery: RecoveryOutcome::NotNeeded,
        final_hash: sha256_hex(&rendered),
    }))
}

fn restore_backup_content<Io: SwitchIo>(
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
    let mut paths: Vec<PathBuf> = entries
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_errors_do_not_expose_internal_english_details() {
        let read_error = SwitchError::ReadCurrent {
            message: "access denied".to_string(),
        };
        assert_eq!(format!("{read_error}"), "无法读取当前配置");

        let commit_error = SwitchError::CommitFailed {
            stage: "backup-dir",
            message: "access denied".to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        };
        assert_eq!(
            format!("{commit_error}"),
            "写入阶段“准备备份目录”失败；恢复结果：无需恢复"
        );
    }
}
