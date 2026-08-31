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
use crate::restore::restore_backup_content;
use asb_core::{
    adapter, AdapterError, AppKind, BackupRecord, LockStatus, SwitchPlan, SwitchPreview,
};
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

pub struct CommonRequest<'a> {
    pub target: &'a Path,
    pub app: AppKind,
    pub common: &'a asb_core::CommonConfigPatch,
    pub backup_dir: &'a Path,
    /// Hash captured when the toggle change was computed; a mismatch blocks
    /// the write.
    pub expected_hash: &'a str,
    /// Hash of the candidate file shown in the preview.
    pub expected_rendered_hash: &'a str,
}

/// One pending overlay write: either a full profile switch or a
/// general-settings-only apply. The transaction below treats both alike.
enum Job<'a> {
    Switch(&'a SwitchPlan),
    Common {
        app: AppKind,
        common: &'a asb_core::CommonConfigPatch,
    },
}

impl Job<'_> {
    fn app(&self) -> AppKind {
        match self {
            Job::Switch(plan) => plan.app,
            Job::Common { app, .. } => *app,
        }
    }

    /// Reason recorded on the pre-write backup for this kind of write.
    fn backup_reason(&self) -> &'static str {
        match self {
            Job::Switch(_) => "switch",
            Job::Common { .. } => "common-settings",
        }
    }

    fn preview(&self, current: &str, backup_dir: &str) -> Result<SwitchPreview, AdapterError> {
        match self {
            Job::Switch(plan) => adapter::preview(current, plan, backup_dir),
            Job::Common { common, .. } => adapter::common_preview(current, common, backup_dir),
        }
    }

    fn render(&self, current: &str) -> Result<String, AdapterError> {
        match self {
            Job::Switch(plan) => adapter::render(current, plan),
            Job::Common { common, .. } => adapter::common_render(current, common),
        }
    }
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
    read_preview_for(io, target, &Job::Switch(plan), backup_dir)
}

/// Same as [`read_preview`] for a general-settings-only apply.
pub fn read_common_preview<Io: SwitchIo>(
    io: &Io,
    req: &CommonRequest,
) -> Result<FilePreview, SwitchError> {
    read_preview_for(
        io,
        req.target,
        &Job::Common {
            app: req.app,
            common: req.common,
        },
        &req.backup_dir.to_string_lossy(),
    )
}

fn read_preview_for<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    job: &Job,
    backup_dir: &str,
) -> Result<FilePreview, SwitchError> {
    let (current, target_existed) =
        read_current_or_empty(io, target, job.app()).map_err(|e| SwitchError::ReadCurrent {
            message: e.to_string(),
        })?;
    let content_hash = sha256_hex(&current);
    let mut preview = job.preview(&current, backup_dir).map_err(plan_rejected)?;
    if !target_existed {
        preview
            .warnings
            .push("配置文件尚不存在，确认后将创建新的用户级配置".to_string());
    }
    let rendered = job.render(&current).map_err(plan_rejected)?;
    let rendered_hash = sha256_hex(&rendered);
    Ok(FilePreview {
        preview,
        content_hash,
        rendered_hash,
        content: display_content(job.app(), &rendered),
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

/// Executes one switch transactionally. All exit paths release the lock.
pub fn execute<Io: SwitchIo>(io: &Io, req: &SwitchRequest) -> Result<SwitchOutcome, SwitchError> {
    execute_job(
        io,
        req.target,
        req.backup_dir,
        req.expected_hash,
        req.expected_rendered_hash,
        &Job::Switch(req.plan),
    )
}

/// Executes a general-settings-only apply through the same transaction:
/// lock → hash check → preview → render → backup → atomic replace → verify.
pub fn execute_common<Io: SwitchIo>(
    io: &Io,
    req: &CommonRequest,
) -> Result<SwitchOutcome, SwitchError> {
    execute_job(
        io,
        req.target,
        req.backup_dir,
        req.expected_hash,
        req.expected_rendered_hash,
        &Job::Common {
            app: req.app,
            common: req.common,
        },
    )
}

fn execute_job<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    backup_dir: &Path,
    expected_hash: &str,
    expected_rendered_hash: &str,
    job: &Job,
) -> Result<SwitchOutcome, SwitchError> {
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
    execute_locked(
        io,
        target,
        backup_dir,
        expected_hash,
        expected_rendered_hash,
        job,
    )
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

/// Produces the preview and the exact private candidate rendering, refusing
/// a plan whose rendering changed after the user saw the preview.
fn plan_candidate(
    job: &Job,
    current: &str,
    backup_dir: &str,
    expected_rendered_hash: &str,
) -> Result<(SwitchPreview, String), SwitchError> {
    let preview = job.preview(current, backup_dir).map_err(plan_rejected)?;
    let rendered = job.render(current).map_err(plan_rejected)?;
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
    job: &Job,
) -> Result<BackupRecord, SwitchError> {
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
    let backup = BackupRecord {
        id: format!("{}-{ts}", &found_hash[..12.min(found_hash.len())]),
        app: job.app(),
        target_path: target.to_string_lossy().to_string(),
        backup_path: backup_path.to_string_lossy().to_string(),
        created_at,
        content_hash: found_hash.to_string(),
        target_existed,
        reason: job.backup_reason().to_string(),
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

    if let Err(e) = adapter::validate_syntax(app, rendered) {
        let _ = io.remove(&temp_path);
        return Err(SwitchError::CommitFailed {
            stage: "temp-validate",
            message: format!("临时文件校验失败: {e}"),
            recovery: RecoveryOutcome::NotNeeded,
        });
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
fn execute_locked<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    backup_dir: &Path,
    expected_hash: &str,
    expected_rendered_hash: &str,
    job: &Job,
) -> Result<SwitchOutcome, SwitchError> {
    let finish = |result| {
        lockfile::release(io, target);
        result
    };

    let (current, target_existed, found_hash) =
        match read_unchanged_current(io, target, job.app(), expected_hash) {
            Ok(verified) => verified,
            Err(error) => return finish(Err(error)),
        };
    let backup_dir_label = backup_dir.to_string_lossy().to_string();
    let (preview, rendered) =
        match plan_candidate(job, &current, &backup_dir_label, expected_rendered_hash) {
            Ok(candidate) => candidate,
            Err(error) => return finish(Err(error)),
        };
    // Re-check immediately before mutation: the file must still hash to the
    // previewed state.
    if let Err(error) = read_unchanged_current(io, target, job.app(), expected_hash) {
        return finish(Err(error));
    }
    let backup = match back_up_current(
        io,
        target,
        backup_dir,
        &current,
        &found_hash,
        target_existed,
        job,
    ) {
        Ok(backup) => backup,
        Err(error) => return finish(Err(error)),
    };
    if let Err(error) = commit_rendered(io, target, job.app(), &rendered, &backup) {
        return finish(Err(error));
    }

    finish(Ok(SwitchOutcome {
        lock: LockStatus::Free,
        acquired_at: backup.created_at.clone(),
        changed: vec![target.to_string_lossy().to_string()],
        warnings: preview.warnings.clone(),
        backup,
        preview,
        recovery: RecoveryOutcome::NotNeeded,
        final_hash: sha256_hex(&rendered),
    }))
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
