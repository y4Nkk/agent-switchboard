//! Observable lock file handling.
//!
//! Rules enforced here:
//! - acquiring creates the lock with create-new semantics, so two executors
//!   cannot both believe they hold it;
//! - a pre-existing lock is classified with [`asb_core::classify_lock`] and
//!   never deleted implicitly;
//! - stale-lock recovery is an explicit, logged action.

use crate::io::SwitchIo;
use crate::pids::pid_liveness;
use asb_core::{LockFileData, LockStatus, PidLiveness};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Sibling lock path for a target file: `config.toml` → `config.toml.asb-lock`.
pub fn lock_path_for(target: &Path) -> PathBuf {
    let mut name = target
        .file_name()
        .expect("target has a file name")
        .to_os_string();
    name.push(".asb-lock");
    target.with_file_name(name)
}

enum LockReadError {
    Missing,
    Invalid(String),
}

fn read_lock_data<Io: SwitchIo>(io: &Io, path: &Path) -> Result<LockFileData, LockReadError> {
    let text = io.read_file(path).map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            LockReadError::Missing
        } else {
            LockReadError::Invalid("无法读取写入锁文件".to_string())
        }
    })?;
    serde_json::from_str(&text)
        .map_err(|_| LockReadError::Invalid("写入锁文件格式无效".to_string()))
}

fn read_error_reason(error: LockReadError) -> String {
    match error {
        LockReadError::Missing => "lock 文件不存在".to_string(),
        LockReadError::Invalid(reason) => reason,
    }
}

/// Reports the observable status of the lock for `target` without changing it.
pub fn probe_lock<Io: SwitchIo>(io: &Io, target: &Path) -> LockStatus {
    let path = lock_path_for(target);
    match read_lock_data(io, &path) {
        Ok(data) => {
            let liveness = data.pid.map(pid_liveness).unwrap_or(PidLiveness::Dead);
            asb_core::classify_lock(&data, liveness)
        }
        Err(LockReadError::Missing) => LockStatus::Free,
        Err(error) => LockStatus::Indeterminate {
            reason: read_error_reason(error),
        },
    }
}

/// Classification with an injected liveness probe (tests and diagnostics).
pub fn probe_lock_with<Io: SwitchIo>(
    io: &Io,
    target: &Path,
    probe: impl Fn(u32) -> PidLiveness,
) -> LockStatus {
    let path = lock_path_for(target);
    match read_lock_data(io, &path) {
        Ok(data) => {
            let liveness = data.pid.map(&probe).unwrap_or(PidLiveness::Dead);
            asb_core::classify_lock(&data, liveness)
        }
        Err(LockReadError::Missing) => LockStatus::Free,
        Err(error) => LockStatus::Indeterminate {
            reason: read_error_reason(error),
        },
    }
}

/// Outcome of trying to take the lock.
pub enum AcquireOutcome {
    Acquired,
    Busy(LockStatus),
}

/// Creates the lock file for `target` or reports the current lock status.
pub fn acquire<Io: SwitchIo>(io: &Io, target: &Path, process_name: &str) -> AcquireOutcome {
    let path = lock_path_for(target);
    let data = LockFileData {
        pid: Some(std::process::id()),
        process_name: Some(process_name.to_string()),
        acquired_at: Some(io.now_rfc3339()),
    };
    let content = serde_json::to_string(&data).expect("lock data serializes");
    match io.write_new_file(&path, &content) {
        Ok(()) => AcquireOutcome::Acquired,
        Err(_) => {
            let status = match read_lock_data(io, &path) {
                Ok(existing) => {
                    let liveness = existing.pid.map(pid_liveness).unwrap_or(PidLiveness::Dead);
                    asb_core::classify_lock(&existing, liveness)
                }
                Err(error) => LockStatus::Indeterminate {
                    reason: read_error_reason(error),
                },
            };
            AcquireOutcome::Busy(status)
        }
    }
}

/// Releases the lock if and only if it still belongs to this process.
pub fn release<Io: SwitchIo>(io: &Io, target: &Path) {
    let path = lock_path_for(target);
    if let Ok(data) = read_lock_data(io, &path) {
        if data.pid == Some(std::process::id()) {
            let _ = io.remove(&path);
        }
    }
}

/// A logged stale-lock recovery entry.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryEntry {
    pub lock_path: String,
    pub removed_holder_pid: Option<u32>,
    pub at: String,
}

/// Explicitly removes a lock that was classified as stale. Returns the
/// recovery entry for the audit trail; errors when the lock is anything
/// other than stale.
pub fn recover_stale<Io: SwitchIo>(io: &Io, target: &Path) -> Result<RecoveryEntry, LockStatus> {
    let path = lock_path_for(target);
    let status = probe_lock(io, target);
    match status {
        LockStatus::Stale(holder) => {
            io.remove(&path).map_err(|_| LockStatus::Indeterminate {
                reason: "lock 文件删除失败".to_string(),
            })?;
            Ok(RecoveryEntry {
                lock_path: path.to_string_lossy().to_string(),
                removed_holder_pid: holder.pid,
                at: io.now_rfc3339(),
            })
        }
        other => Err(other),
    }
}
