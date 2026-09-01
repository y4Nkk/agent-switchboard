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
mod prompt_documents;
mod restore;

pub use executor::{
    execute, read_preview, restore, sha256_hex, FilePreview, RecoveryOutcome, RestoreOutcome,
    SwitchError, SwitchOutcome, SwitchRequest,
};
pub use io::{FsIo, SwitchIo};
pub use lockfile::{
    acquire, lock_path_for, probe_lock, probe_lock_with, recover_stale, release, AcquireOutcome,
    RecoveryEntry,
};
pub use prompt_documents::{
    read_global_prompt_document, write_global_prompt_document, GlobalPromptDocumentOutcome,
    GlobalPromptDocumentRequest,
};
pub use restore::list_backups;
