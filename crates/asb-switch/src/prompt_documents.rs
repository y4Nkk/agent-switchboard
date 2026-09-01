//! Transactional writes for the two user-global instruction documents.
//!
//! These are Markdown documents rather than client configuration syntax, but
//! they still follow the executor boundary: lock, external-change check,
//! backup, temporary write, atomic replace, post-write verification, and
//! recovery from the immediately preceding backup.

use crate::executor::{
    sha256_hex, timestamp_name, write_backup_metadata, RecoveryOutcome, SwitchError, PROCESS_NAME,
};
use crate::io::SwitchIo;
use crate::lockfile::{self, AcquireOutcome};
use crate::restore::restore_backup_content;
use asb_core::{AppKind, BackupRecord, GlobalPromptDocument};
use std::io::ErrorKind;
use std::path::Path;

/// The renderer supplies document text and the hash it last read. It never
/// supplies a target path; command code resolves that backend-only value.
pub struct GlobalPromptDocumentRequest<'a> {
    pub target: &'a Path,
    pub app: AppKind,
    pub content: &'a str,
    pub backup_dir: &'a Path,
    pub expected_hash: &'a str,
}

/// Successful prompt-document write details. Command code returns only the
/// renderer-safe document snapshot, keeping backup paths backend-owned.
#[derive(Debug, Clone)]
pub struct GlobalPromptDocumentOutcome {
    pub document: GlobalPromptDocument,
    pub backup: BackupRecord,
}

fn document_snapshot(app: AppKind, content: String, exists: bool) -> GlobalPromptDocument {
    GlobalPromptDocument {
        app,
        file_name: app.global_prompt_file_name().to_string(),
        content_hash: sha256_hex(&content),
        content,
        exists,
    }
}

fn read_current_or_empty<Io: SwitchIo>(
    io: &Io,
    target: &Path,
) -> Result<(String, bool), SwitchError> {
    match io.read_file(target) {
        Ok(content) => Ok((content, true)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok((String::new(), false)),
        Err(error) => Err(SwitchError::ReadCurrent {
            message: error.to_string(),
        }),
    }
}

/// Reads the global instruction document without creating, locking, or
/// exposing its absolute target path.
pub fn read_global_prompt_document<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    app: AppKind,
) -> Result<GlobalPromptDocument, SwitchError> {
    let (content, exists) = read_current_or_empty(io, target)?;
    Ok(document_snapshot(app, content, exists))
}

fn read_expected_current<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    expected_hash: &str,
) -> Result<(String, bool, String), SwitchError> {
    let (content, exists) = read_current_or_empty(io, target)?;
    let found_hash = sha256_hex(&content);
    if found_hash != expected_hash {
        return Err(SwitchError::ExternalChange {
            expected_hash: expected_hash.to_string(),
            found_hash,
        });
    }
    Ok((content, exists, found_hash))
}

fn back_up_current<Io: SwitchIo>(
    io: &Io,
    request: &GlobalPromptDocumentRequest,
    current: &str,
    current_hash: &str,
    target_existed: bool,
) -> Result<BackupRecord, SwitchError> {
    io.ensure_dir(request.backup_dir)
        .map_err(|error| SwitchError::CommitFailed {
            stage: "backup-dir",
            message: error.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        })?;
    let timestamp = timestamp_name(io);
    let file_name = request
        .target
        .file_name()
        .expect("global prompt target has a file name")
        .to_string_lossy();
    let backup_path = request
        .backup_dir
        .join(format!("{file_name}.{timestamp}.bak"));
    io.write_new_file(&backup_path, current)
        .map_err(|error| SwitchError::CommitFailed {
            stage: "backup",
            message: error.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        })?;
    let backup = BackupRecord {
        id: format!(
            "{}-{timestamp}",
            &current_hash[..12.min(current_hash.len())]
        ),
        app: request.app,
        target_path: request.target.to_string_lossy().to_string(),
        backup_path: backup_path.to_string_lossy().to_string(),
        created_at: io.now_rfc3339(),
        content_hash: current_hash.to_string(),
        target_existed,
        reason: "prompt-management".to_string(),
    };
    write_backup_metadata(io, &backup, "backup-meta")?;
    Ok(backup)
}

fn commit_document<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    content: &str,
    backup: &BackupRecord,
) -> Result<(), SwitchError> {
    let file_name = target
        .file_name()
        .expect("global prompt target has a file name")
        .to_string_lossy();
    let temporary = target.with_file_name(format!("{file_name}.{}.asb-tmp", std::process::id()));
    io.write_new_file(&temporary, content)
        .map_err(|error| SwitchError::CommitFailed {
            stage: "temp-write",
            message: error.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        })?;
    if let Err(error) = io.rename_replace(&temporary, target) {
        let _ = io.remove(&temporary);
        return Err(SwitchError::CommitFailed {
            stage: "atomic-replace",
            message: error.to_string(),
            recovery: RecoveryOutcome::NotNeeded,
        });
    }
    let verified = matches!(io.read_file(target), Ok(live) if live == content);
    if !verified {
        return Err(SwitchError::CommitFailed {
            stage: "post-verify",
            message: "替换后校验失败".to_string(),
            recovery: restore_backup_content(io, target, backup),
        });
    }
    Ok(())
}

fn execute_locked<Io: SwitchIo>(
    io: &Io,
    request: &GlobalPromptDocumentRequest,
) -> Result<GlobalPromptDocumentOutcome, SwitchError> {
    let finish = |result| match (result, lockfile::release(io, request.target)) {
        (Ok(outcome), Ok(())) => Ok(outcome),
        (Ok(_), Err(reason)) => Err(SwitchError::CommitFailed {
            stage: "lock-release",
            message: reason,
            recovery: RecoveryOutcome::NotNeeded,
        }),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(reason)) => Err(SwitchError::CommitFailed {
            stage: "lock-release",
            message: format!("{error}; {reason}"),
            recovery: RecoveryOutcome::NotNeeded,
        }),
    };
    let (current, target_existed, current_hash) =
        match read_expected_current(io, request.target, request.expected_hash) {
            Ok(current) => current,
            Err(error) => return finish(Err(error)),
        };
    // A second read immediately before creating the backup closes the gap
    // between the original version check and the first mutation.
    if let Err(error) = read_expected_current(io, request.target, request.expected_hash) {
        return finish(Err(error));
    }
    let backup = match back_up_current(io, request, &current, &current_hash, target_existed) {
        Ok(backup) => backup,
        Err(error) => return finish(Err(error)),
    };
    if let Err(error) = commit_document(io, request.target, request.content, &backup) {
        return finish(Err(error));
    }
    finish(Ok(GlobalPromptDocumentOutcome {
        document: document_snapshot(request.app, request.content.to_string(), true),
        backup,
    }))
}

/// Saves a global prompt document through the same observable transaction
/// guarantees as client configuration writes.
pub fn write_global_prompt_document<Io: SwitchIo>(
    io: &Io,
    request: &GlobalPromptDocumentRequest,
) -> Result<GlobalPromptDocumentOutcome, SwitchError> {
    if let Some(parent) = request.target.parent() {
        io.ensure_dir(parent)
            .map_err(|error| SwitchError::CommitFailed {
                stage: "target-dir",
                message: error.to_string(),
                recovery: RecoveryOutcome::NotNeeded,
            })?;
    }
    match lockfile::acquire(io, request.target, PROCESS_NAME) {
        AcquireOutcome::Acquired => execute_locked(io, request),
        AcquireOutcome::Busy(status) => Err(SwitchError::BlockedByLock { status }),
    }
}
