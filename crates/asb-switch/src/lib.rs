//! asb-switch is the switch executor and the only crate allowed to write
//! live configuration files.
//!
//! Planning lives in `asb-core` and is side-effect free; everything here that
//! touches a file is observable: locks are classified (free, held, stale,
//! indeterminate), backups carry hashes, and every failure reports whether
//! and how the live file was restored.

mod display;
pub mod executor;
pub mod io;
pub mod lockfile;
pub mod pids;
mod restore;

pub use executor::{
    execute, execute_common, read_common_preview, read_preview, sha256_hex, CommonRequest,
    FilePreview, RecoveryOutcome, SwitchError, SwitchOutcome, SwitchRequest,
};
pub use io::{FsIo, SwitchIo};
pub use lockfile::{
    acquire, lock_path_for, probe_lock, probe_lock_with, recover_stale, release, AcquireOutcome,
    RecoveryEntry,
};
pub use restore::{list_backups, restore, RestoreOutcome};
