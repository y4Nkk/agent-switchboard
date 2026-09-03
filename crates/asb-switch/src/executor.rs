//! The switch executor: the only layer allowed to write live configuration
//! files.
//!
//! One switch walks: lock → read & hash → external-change check → preview →
//! render → re-check → backup → temporary write → syntax validation → atomic
//! replacement → post-write verification → lock release. Any failure after
//! the file has been replaced restores the immediately preceding backup.

use crate::display::display_content;
use crate::io::SwitchIo;
use crate::lockfile::{self, AcquireOutcome};
use crate::restore::{restore_backup_content, restore_locked};
use asb_core::{
    adapter, AdapterError, AppKind, BackupRecord, LockStatus, SwitchPlan, SwitchPreview,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub use crate::restore::RestoreOutcome;

pub const PROCESS_NAME: &str = "agent-switchboard";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePreview {
    pub preview: SwitchPreview,
    pub content_hash: String,
    /// Hash of the exact private candidate used for execution. Execution
    /// refuses a plan whose original rendering changed after this preview.
    pub rendered_hash: String,
    /// Redacted candidate file text for the pretty-printed UI view. The
    /// executor keeps the original candidate private for hashing and writes.
    pub content: String,
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
    /// The transaction reached another result, but its process-owned lock
    /// could not be released. The original result remains available instead
    /// of silently treating a potentially active lock as gone.
    LockReleaseFailed {
        message: String,
        prior: Box<SwitchError>,
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
            SwitchError::LockReleaseFailed { prior, .. } => {
                write!(f, "写入锁释放失败；原操作结果：{prior}")
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
        "backup-verify" => "回读校验备份文件",
        "backup-meta" => "记录备份信息",
        "target-dir" => "准备配置目录",
        "temp-write" => "写入临时文件",
        "temp-verify" => "回读校验临时文件",
        "temp-validate" => "校验临时文件",
        "atomic-replace" => "替换配置文件",
        "post-verify" => "验证写入结果",
        "state-save" => "保存应用状态",
        "lock-release" => "释放写入锁",
        "restore-precheck" => "创建恢复前备份",
        "restore-backup-verify" => "校验待恢复备份",
        "restore-precheck-verify" => "回读校验恢复前备份",
        "restore-meta" => "记录恢复前备份信息",
        "restore-write" => "写入恢复临时文件",
        "restore-temp-verify" => "回读校验恢复临时文件",
        "restore-temp-validate" => "校验恢复临时文件",
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

/// SHA-256 digest bytes for one content string; [`sha256_hex`] is its hex
/// rendering.
pub fn sha256_digest(content: &str) -> Vec<u8> {
    Sha256::digest(content.as_bytes()).to_vec()
}

pub fn sha256_hex(content: &str) -> String {
    sha256_digest(content)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
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

fn empty_configuration(app: AppKind) -> &'static str {
    match app {
        AppKind::Codex => "",
        AppKind::Claude => "{}",
    }
}

pub(crate) fn read_current_or_empty<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    app: AppKind,
) -> Result<(String, bool), std::io::Error> {
    match io.read_file(target) {
        Ok(text) => Ok((text, true)),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            Ok((empty_configuration(app).to_string(), false))
        }
        Err(error) => Err(error),
    }
}

pub(crate) fn metadata_path(backup_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.meta.json", backup_path.to_string_lossy()))
}

pub(crate) fn write_backup_metadata<Io: SwitchIo>(
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
    let app = plan.app();
    let (current, target_existed) =
        read_current_or_empty(io, target, app).map_err(|e| SwitchError::ReadCurrent {
            message: e.to_string(),
        })?;
    let content_hash = sha256_hex(&current);
    let mut preview = adapter::preview(&current, plan, backup_dir).map_err(plan_rejected)?;
    if !target_existed {
        preview
            .warnings
            .push("配置文件尚不存在，确认后将创建新的用户级配置".to_string());
    }
    let rendered = adapter::render(&current, plan).map_err(plan_rejected)?;
    let rendered_hash = sha256_hex(&rendered);
    Ok(FilePreview {
        preview,
        content_hash,
        rendered_hash,
        content: display_content(app, &rendered),
    })
}

fn plan_rejected(e: AdapterError) -> SwitchError {
    SwitchError::PlanRejected {
        message: e.message,
        line: e.line,
    }
}

pub(crate) fn timestamp_name<Io: SwitchIo>(io: &Io) -> String {
    io.now_rfc3339()
        .chars()
        .filter(|c| c.is_ascii_digit())
        .take(17)
        .collect()
}

/// Executes one provider projection transactionally. A successful client
/// write is not complete until `commit` records the same fact in application
/// state; if that commit fails, this function restores the just-created
/// backup before releasing the lock. There is deliberately no no-commit
/// execution entry point.
pub fn execute<Io: SwitchIo, Commit>(
    io: &Io,
    req: &SwitchRequest,
    commit: Commit,
) -> Result<SwitchOutcome, SwitchError>
where
    Commit: FnOnce(&SwitchOutcome) -> Result<(), String>,
{
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
    execute_locked(
        io,
        req.target,
        req.backup_dir,
        req.expected_hash,
        req.expected_rendered_hash,
        req.plan,
        commit,
    )
}

/// Restores one recorded client configuration backup through the same locked
/// transaction boundary as provider projection. The caller must commit the
/// corresponding application write record before this function releases the
/// lock; a commit failure restores the pre-restore snapshot.
pub fn restore<Io: SwitchIo, Commit>(
    io: &Io,
    backup: &BackupRecord,
    target: &Path,
    commit: Commit,
) -> Result<RestoreOutcome, SwitchError>
where
    Commit: FnOnce(&RestoreOutcome) -> Result<(), String>,
{
    if let Some(parent) = target.parent() {
        io.ensure_dir(parent)
            .map_err(|error| SwitchError::CommitFailed {
                stage: "target-dir",
                message: error.to_string(),
                recovery: RecoveryOutcome::NotNeeded,
            })?;
    }
    match lockfile::acquire(io, target, PROCESS_NAME) {
        AcquireOutcome::Busy(status) => Err(SwitchError::BlockedByLock { status }),
        AcquireOutcome::Acquired => {
            let result = restore_locked(io, backup, target, commit);
            match lockfile::release(io, target) {
                Ok(()) => result,
                Err(message) => match result {
                    Ok(mut outcome) => {
                        outcome
                            .warnings
                            .push(format!("配置已恢复，但写入锁释放失败：{message}"));
                        Ok(outcome)
                    }
                    Err(prior) => Err(SwitchError::LockReleaseFailed {
                        message,
                        prior: Box::new(prior),
                    }),
                },
            }
        }
    }
}

/// Reads the live file and refuses to continue when its hash no longer
/// matches the previewed state. Returns the text, whether the target
/// existed, and the verified hash.
fn read_unchanged_current<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    app: AppKind,
    expected_hash: &str,
) -> Result<(String, bool, String), SwitchError> {
    let (current, target_existed) =
        read_current_or_empty(io, target, app).map_err(|e| SwitchError::ReadCurrent {
            message: e.to_string(),
        })?;
    let found_hash = sha256_hex(&current);
    if found_hash != expected_hash {
        return Err(SwitchError::ExternalChange {
            expected_hash: expected_hash.to_string(),
            found_hash,
        });
    }
    Ok((current, target_existed, found_hash))
}

/// Re-reads the target at the last recoverable point before a mutation. The
/// comparison includes existence because an empty document and a missing file
/// have the same content hash but different restoration semantics.
pub(crate) fn verify_live_snapshot<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    app: AppKind,
    expected_content: &str,
    expected_existed: bool,
) -> Result<(), SwitchError> {
    let (found_content, found_existed) =
        read_current_or_empty(io, target, app).map_err(|error| SwitchError::ReadCurrent {
            message: error.to_string(),
        })?;
    if found_content == expected_content && found_existed == expected_existed {
        return Ok(());
    }
    Err(SwitchError::ExternalChange {
        expected_hash: sha256_hex(expected_content),
        found_hash: sha256_hex(&found_content),
    })
}

/// Produces the preview and the exact private candidate rendering, refusing
/// a plan whose rendering changed after the user saw the preview.
fn plan_candidate(
    plan: &SwitchPlan,
    current: &str,
    backup_dir: &str,
    expected_rendered_hash: &str,
) -> Result<(SwitchPreview, String), SwitchError> {
    let preview = adapter::preview(current, plan, backup_dir).map_err(plan_rejected)?;
    let rendered = adapter::render(current, plan).map_err(plan_rejected)?;
    if sha256_hex(&rendered) != expected_rendered_hash {
        return Err(SwitchError::PlanChanged);
    }
    Ok((preview, rendered))
}

/// Snapshots the current content as the pre-write backup, sidecar metadata
/// included.
fn back_up_current<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    backup_dir: &Path,
    current: &str,
    found_hash: &str,
    target_existed: bool,
    plan: &SwitchPlan,
) -> Result<BackupRecord, SwitchError> {
    verify_live_snapshot(io, target, plan.app(), current, target_existed)?;
    io.ensure_dir(backup_dir)
        .map_err(|e| SwitchError::CommitFailed {
            stage: "backup-dir",
            message: e.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        })?;
    let ts = timestamp_name(io);
    let file_name = target
        .file_name()
        .expect("target has a file name")
        .to_string_lossy()
        .to_string();
    let backup_path = backup_dir.join(format!("{file_name}.{ts}.bak"));
    let created_at = io.now_rfc3339();
    io.write_new_file(&backup_path, current)
        .map_err(|e| SwitchError::CommitFailed {
            stage: "backup",
            message: e.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        })?;
    let backup_text = io
        .read_file(&backup_path)
        .map_err(|error| SwitchError::CommitFailed {
            stage: "backup-verify",
            message: error.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        })?;
    if backup_text != current || adapter::validate_syntax(plan.app(), &backup_text).is_err() {
        return Err(SwitchError::CommitFailed {
            stage: "backup-verify",
            message: "备份回读内容或语法不匹配".to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        });
    }
    let backup = BackupRecord {
        id: format!("{}-{ts}", &found_hash[..12.min(found_hash.len())]),
        app: plan.app(),
        target_path: target.to_string_lossy().to_string(),
        backup_path: backup_path.to_string_lossy().to_string(),
        created_at,
        content_hash: found_hash.to_string(),
        target_existed,
        reason: "provider-projection".to_string(),
    };
    write_backup_metadata(io, &backup, "backup-meta")?;
    Ok(backup)
}

/// Writes the rendered candidate: temporary file → syntax validation →
/// atomic replacement → post-write verification. A failed stage after the
/// replacement restores the just-created backup.
fn commit_rendered<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    app: AppKind,
    rendered: &str,
    backup: &BackupRecord,
    expected_current: &str,
) -> Result<(), SwitchError> {
    let file_name = target
        .file_name()
        .expect("target has a file name")
        .to_string_lossy()
        .to_string();
    let temp_path = target.with_file_name(format!("{}.{}.asb-tmp", file_name, std::process::id()));
    io.write_new_file(&temp_path, rendered)
        .map_err(|e| SwitchError::CommitFailed {
            stage: "temp-write",
            message: e.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        })?;

    let temp_text = match io.read_file(&temp_path) {
        Ok(text) if text == rendered => text,
        Ok(_) => {
            let _ = io.remove(&temp_path);
            return Err(SwitchError::CommitFailed {
                stage: "temp-verify",
                message: "临时文件回读内容不匹配".to_string(),
                recovery: RecoveryOutcome::NotNeeded,
            });
        }
        Err(error) => {
            let _ = io.remove(&temp_path);
            return Err(SwitchError::CommitFailed {
                stage: "temp-verify",
                message: error.to_string(),
                recovery: RecoveryOutcome::NotNeeded,
            });
        }
    };
    if let Err(e) = adapter::validate_syntax(app, &temp_text) {
        let _ = io.remove(&temp_path);
        return Err(SwitchError::CommitFailed {
            stage: "temp-validate",
            message: format!("临时文件校验失败: {e}"),
            recovery: RecoveryOutcome::NotNeeded,
        });
    }

    // The temp is now known good. Check the live target once more before the
    // irreversible replacement so a host edit made while creating the backup
    // cannot be silently overwritten.
    if let Err(error) =
        verify_live_snapshot(io, target, app, expected_current, backup.target_existed)
    {
        let _ = io.remove(&temp_path);
        return Err(error);
    }

    if let Err(e) = io.rename_replace(&temp_path, target) {
        let _ = io.remove(&temp_path);
        return Err(SwitchError::CommitFailed {
            stage: "atomic-replace",
            message: e.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        });
    }

    // Post-write verification: the live file must equal the rendered
    // candidate and parse cleanly. Otherwise restore the backup.
    let verified = match io.read_file(target) {
        Ok(text) => text == rendered && adapter::validate_syntax(app, &text).is_ok(),
        Err(_) => false,
    };
    if !verified {
        let recovery = restore_backup_content(io, target, backup);
        return Err(SwitchError::CommitFailed {
            stage: "post-verify",
            message: "替换后校验失败".to_string(),
            recovery,
        });
    }
    Ok(())
}

/// The locked body of one transaction: verify → plan → re-verify → backup →
/// commit. Every exit path releases the lock through `finish`.
fn execute_locked<Io: SwitchIo, Commit>(
    io: &Io,
    target: &Path,
    backup_dir: &Path,
    expected_hash: &str,
    expected_rendered_hash: &str,
    plan: &SwitchPlan,
    commit: Commit,
) -> Result<SwitchOutcome, SwitchError>
where
    Commit: FnOnce(&SwitchOutcome) -> Result<(), String>,
{
    let finish = |result: Result<SwitchOutcome, SwitchError>| match lockfile::release(io, target) {
        Ok(()) => result,
        Err(message) => match result {
            Ok(mut outcome) => {
                outcome
                    .warnings
                    .push(format!("写入已完成，但无法释放写入锁：{message}"));
                Ok(outcome)
            }
            Err(prior) => Err(SwitchError::LockReleaseFailed {
                message,
                prior: Box::new(prior),
            }),
        },
    };

    let (initial_current, initial_target_existed, _) =
        match read_unchanged_current(io, target, plan.app(), expected_hash) {
            Ok(verified) => verified,
            Err(error) => return finish(Err(error)),
        };
    let backup_dir_label = backup_dir.to_string_lossy().to_string();
    let (current, target_existed, found_hash) =
        match read_unchanged_current(io, target, plan.app(), expected_hash) {
            Ok(verified) => verified,
            Err(error) => return finish(Err(error)),
        };
    if current != initial_current || target_existed != initial_target_existed {
        return finish(Err(SwitchError::ExternalChange {
            expected_hash: sha256_hex(&initial_current),
            found_hash,
        }));
    }
    let (preview, rendered) =
        match plan_candidate(plan, &current, &backup_dir_label, expected_rendered_hash) {
            Ok(candidate) => candidate,
            Err(error) => return finish(Err(error)),
        };
    let backup = match back_up_current(
        io,
        target,
        backup_dir,
        &current,
        &found_hash,
        target_existed,
        plan,
    ) {
        Ok(backup) => backup,
        Err(error) => return finish(Err(error)),
    };
    if let Err(error) = commit_rendered(io, target, plan.app(), &rendered, &backup, &current) {
        return finish(Err(error));
    }

    let outcome = SwitchOutcome {
        lock: LockStatus::Free,
        acquired_at: backup.created_at.clone(),
        changed: vec![target.to_string_lossy().to_string()],
        warnings: preview.warnings.clone(),
        backup,
        preview,
        recovery: RecoveryOutcome::NotNeeded,
        final_hash: sha256_hex(&rendered),
    };
    if let Err(message) = commit(&outcome) {
        let recovery = restore_backup_content(io, target, &outcome.backup);
        return finish(Err(SwitchError::CommitFailed {
            stage: "state-save",
            message,
            recovery,
        }));
    }

    finish(Ok(outcome))
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
