//! Observable lock contract.
//!
//! A lock is state, not a boolean. Planning and classification never delete
//! or recover a lock; recovery is an explicit executor action that must be
//! logged.

use serde::{Deserialize, Serialize};

/// Whether the operating system reports a pid as running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PidLiveness {
    Alive,
    Dead,
    Unknown,
}

/// Parsed contents of a lock file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockFileData {
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub acquired_at: Option<String>,
}

/// Who holds a lock, as far as observable evidence goes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockHolder {
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub acquired_at: Option<String>,
}

/// The four lock states. `Indeterminate` carries the reason we could not
/// decide; the caller must not treat it as free.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "state")]
pub enum LockStatus {
    Free,
    Held(LockHolder),
    Stale(LockHolder),
    Indeterminate { reason: String },
}

/// Classifies a lock from parsed data plus an OS liveness probe.
///
/// - unparseable or missing pid data → `Indeterminate`
/// - pid alive → `Held`
/// - pid dead → `Stale` (a dead process cannot own a lock)
/// - liveness undecidable (for example access denied) → `Indeterminate`
///
/// This function is pure: it never removes anything.
pub fn classify_lock(parsed: &LockFileData, liveness: PidLiveness) -> LockStatus {
    let Some(pid) = parsed.pid else {
        return LockStatus::Indeterminate {
            reason: "锁文件中没有可用的进程标识".to_string(),
        };
    };
    let holder = LockHolder {
        pid: Some(pid),
        process_name: parsed.process_name.clone(),
        acquired_at: parsed.acquired_at.clone(),
    };
    match liveness {
        PidLiveness::Alive => LockStatus::Held(holder),
        PidLiveness::Dead => LockStatus::Stale(holder),
        PidLiveness::Unknown => LockStatus::Indeterminate {
            reason: format!("无法确定进程 {pid} 是否仍在运行"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(pid: Option<u32>) -> LockFileData {
        LockFileData {
            pid,
            process_name: Some("agent-switchboard".to_string()),
            acquired_at: Some("2026-08-26T00:00:00Z".to_string()),
        }
    }

    #[test]
    fn lock_without_pid_is_indeterminate_not_free() {
        assert_eq!(classify_lock(&data(None), PidLiveness::Dead), {
            LockStatus::Indeterminate {
                reason: "锁文件中没有可用的进程标识".to_string(),
            }
        });
    }

    #[test]
    fn live_pid_is_held() {
        assert!(matches!(
            classify_lock(&data(Some(4242)), PidLiveness::Alive),
            LockStatus::Held(_)
        ));
    }

    #[test]
    fn dead_pid_is_stale_not_free() {
        assert!(matches!(
            classify_lock(&data(Some(4242)), PidLiveness::Dead),
            LockStatus::Stale(_)
        ));
    }

    #[test]
    fn unknown_liveness_is_indeterminate() {
        assert!(matches!(
            classify_lock(&data(Some(4242)), PidLiveness::Unknown),
            LockStatus::Indeterminate { .. }
        ));
    }
}
