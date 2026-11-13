//! Typed commands exposed to the UI.
//!
//! The UI passes only profile ids, drafts, app kinds, and preview hashes. The
//! command modules resolve the two supported local configuration paths and
//! are the sole backend entry point for preview, switch, restore, and
//! discovery. Split by domain:
//!
//! - [`error`]: the typed error and shared command guards
//! - [`window`]: title-bar window controls and the inspector toggle
//! - [`status`]: read-only configuration status and lock observation
//! - [`client_settings`]: general-configuration overlay commands
//! - [`codex_probe`]: degradation-probe catalog, batch start/cancel/save-retry, and history commands
//! - [`official_login`]: official client login start/poll/cancel commands
//! - [`prompt_management`]: global AGENTS.md / CLAUDE.md document commands
//! - [`subagent_settings`]: Codex `[agents]` default-setting commands
//! - [`switching`]: switch, backup, restore, and undo commands
//!
//! Profile store CRUD, application settings, probing, discovery, sessions,
//! and external-profile import live here as thin delegations to their owner modules.
//!
//! Filesystem and database work runs through [`error::blocking`]. Network
//! commands use that pool or cancellable async I/O, keeping the main thread
//! free while a provider responds.

mod app_settings;
pub(crate) mod claude_integration;
pub(crate) mod codex_project_plans;
mod discovery;
pub(crate) mod outbound_proxy_settings;
mod profiles;
mod query;
mod quota;
mod sessions;
pub(crate) mod session_range;
pub(crate) mod session_provider_transfer;
mod session_search;
pub use session_search::*;
pub(crate) mod storage;
pub(crate) mod quota_refresh;


pub(crate) mod client_settings;
pub(crate) mod client_software;
pub(crate) mod cloud_backup;
pub(crate) mod codex_probe;
pub(crate) mod error;
pub(crate) mod extensions;
pub(crate) mod gateway;
pub(crate) mod client_configuration_apply;
pub(crate) mod client_configuration_manual;
pub(crate) mod client_configuration_repair;
pub(crate) mod model_usage;
pub(crate) mod native_telemetry;
pub(crate) mod request_usage;
pub(crate) mod resource_usage;
pub(crate) mod official_login;
pub(crate) mod prompt_management;
pub(crate) mod provider_endpoints;
pub(crate) mod provider_request;
pub(crate) mod provider_diagnostics;
pub(crate) mod runtime_log;
pub(crate) mod status;
pub(crate) mod subagent_settings;
pub(crate) mod switching;
pub(crate) mod usage_history;
pub(crate) mod window;
pub(crate) mod workspace;

pub(crate) use status::config_status_report;
pub(crate) use status::ConfigFileStatus;
pub(crate) use window::apply_desktop_settings;

use std::sync::{Arc, Mutex, MutexGuard};

/// Serializes configuration snapshots and writers — switch,
/// save-and-apply, restore/undo, gateway port changes, and retry-driven
/// gateway recovery. Every writer also verifies an expected content hash,
/// but hash checks alone leave a read-then-write window; this gate closes it
/// so two confirmed transactions can never interleave their file writes.
#[derive(Clone, Default)]
pub(crate) struct ConfigWriteGate(Arc<Mutex<()>>);

impl ConfigWriteGate {
    /// Read-only IPCs and local store mutations must enter on the blocking pool,
    /// under the same gate as preparation's staged directory replacement.
    pub(crate) async fn run<T, F>(&self, task: F) -> Result<T, error::CommandError>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, error::CommandError> + Send + 'static,
    {
        let gate = self.clone();
        error::blocking(move || {
            let _guard = gate.lock().map_err(|error| error::CommandError::new("config-write-gate-unavailable", error))?;
            task()
        }).await
    }

    pub(crate) fn shared(&self) -> Arc<Mutex<()>> {
        Arc::clone(&self.0)
    }

    pub(crate) fn lock(&self) -> Result<MutexGuard<'_, ()>, String> {
        self.0
            .lock()
            .map_err(|_| "客户端配置写入闸门不可用".to_string())
    }
}

#[cfg(test)]
mod configuration_gate_tests;

pub use app_settings::*;
pub use discovery::*;
pub use outbound_proxy_settings::*;
pub use profiles::*;
pub use query::*;
pub use quota::*;
pub use sessions::*;
