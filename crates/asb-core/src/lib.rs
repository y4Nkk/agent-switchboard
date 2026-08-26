//! asb-core owns the typed contracts and all pure planning logic.
//!
//! Everything in this crate is side-effect free: it transforms configuration
//! text into previews and rendered candidates, and it never touches the
//! filesystem. Live-file mutation belongs exclusively to `asb-switch`.

pub mod adapter;
pub mod contracts;
pub mod discovery;
pub mod lock;
pub mod ownership;
pub mod redact;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod validate;

pub use discovery::{discover, inspect, DiscoveredFile, DiscoveredState, DiscoveryReport};

pub use adapter::{preview, render, route_state, validate_syntax, AdapterError};
pub use contracts::{
    AppKind, BackupRecord, ChangeKind, ClaudeModelSettings, CodexModelSettings, CommonConfigPatch,
    KeyChange, MatchStatus, ModelOptions, PatchEntry, PatchValue, ProviderDraft, ProviderProfile,
    RouteMode, RouteState, SwitchLog, SwitchPlan, SwitchPreview,
};
pub use lock::{classify_lock, LockFileData, LockHolder, LockStatus, PidLiveness};
pub use redact::{redact, REDACTED};
pub use validate::{validate_plan, ValidationError};
