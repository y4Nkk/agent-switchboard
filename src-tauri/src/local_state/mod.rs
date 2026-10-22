//! Application-owned local state: desktop preferences, optional cloud-backup
//! connection coordinates, and the Codex reset-signal cache. Provider,
//! client-settings, and write-history storage live in [`crate::config_store`];
//! this struct only hands out its store and resolves the real client paths,
//! which are never created here.

mod caches;
pub(crate) mod codex_paths;
mod paths;
mod settings;
mod settings_upgrade;
mod workspace;


pub(crate) use paths::user_home_dir;
pub(crate) use settings::{DesktopSettingsError, ThemePreference};
#[allow(unused_imports)] // public contract mirror of the frontend settings type
pub use settings::{
    AppSettings, CloseBehavior, CloudBackupSettings, LanguagePreference, StartupPage, WorkspacePage,
};

use crate::config_store::ConfigStore;
use asb_core::contracts::AppKind;
use paths::{
    claude_credentials_path_in_home, codex_auth_path_in_home, global_prompt_target_in_home,
    target_in_home,
};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct LocalState {
    root: PathBuf,
}

impl LocalState {
    pub fn from_app(app: &tauri::AppHandle) -> Result<Self, String> {
        Self::from_identifier(&app.config().identifier)
    }

    /// Uses the same path before and after an AppHandle exists, including
    /// process-level Windows directory overrides.
    pub(crate) fn from_identifier(identifier: &str) -> Result<Self, String> {
        let app_data_dir = crate::app_paths::data_directory(identifier)?;
        Ok(Self::from_app_data_dir(app_data_dir))
    }

    /// Runs after the single-instance guard and before commands or the
    /// gateway are published. Runtime state lookup never migrates a store.
    #[cfg(test)]
    #[allow(dead_code)] // verification harness
    pub(crate) fn initialize_schemas(&self) -> Result<(), String> {
        self.initialize_configuration_store()?;
        self.initialize_extension_schema()
    }

    pub(crate) fn initialize_configuration_store(&self) -> Result<(), String> {
        self.configuration()
            .initialize_current_layout()
            .map_err(|error| error.to_string())?;
        crate::session_manager::initialize_storage(self.root())?;
        if let Err(error) = crate::session_manager::history::initialize(self.root()) {
            log::warn!("Claude 会话保留暂不可用，原库已保留：{error}");
        }
        if let Err(error) = crate::usage_history::initialize(self) {
            log::warn!("用量历史初始化失败，该历史暂不可读写，其他功能继续可用: {error}");
        }
        Ok(())
    }

    pub(crate) fn initialize_extension_schema(&self) -> Result<(), String> {
        self.ensure_extension_schema()
    }

    /// Brings the persisted extension library to the current schema once at
    /// startup: an interrupted swap is recovered, then the one-shot offline
    /// migration runs. Failure blocks startup instead of risking a second
    /// writer against a rejected schema.
    fn ensure_extension_schema(&self) -> Result<(), String> {
        let root = self.root.join("extensions");
        crate::extensions::migrate::recover_interrupted_migration(&root)?;
        crate::extensions::migrate::migrate_library_if_needed(&root).map(|_| ())
    }

    fn from_app_data_dir(app_data_dir: PathBuf) -> Self {
        Self {
            root: app_data_dir.join("state"),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_root(root: PathBuf) -> Self {
        Self { root }
    }

    /// The application configuration store rooted at this state directory.
    /// Every provider, client-settings, and history operation goes through
    /// it; this struct keeps no second copy of that data.
    pub fn configuration(&self) -> ConfigStore {
        ConfigStore::new(self.root.clone())
    }

    /// The `state/` directory itself; extension storage lives beside the
    /// provider store under `state/extensions`.
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    pub fn target(&self, app: AppKind) -> Result<PathBuf, String> {
        Self::user_config_path(app)
    }

    /// Resolves the one global instruction document owned by each supported
    /// client. Both clients honor their explicit configuration directories.
    pub fn global_prompt_target(&self, app: AppKind) -> Result<PathBuf, String> {
        Self::global_prompt_path(app)
    }

    pub fn user_config_path(app: AppKind) -> Result<PathBuf, String> {
        let home = user_home_dir()?;
        let codex_home = std::env::var_os("CODEX_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let claude_dir = std::env::var_os("CLAUDE_CONFIG_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Ok(target_in_home(
            &home,
            codex_home.as_deref(),
            claude_dir.as_deref(),
            app,
        ))
    }

    pub fn global_prompt_path(app: AppKind) -> Result<PathBuf, String> {
        let home = user_home_dir()?;
        let codex_home = std::env::var_os("CODEX_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let claude_dir = std::env::var_os("CLAUDE_CONFIG_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Ok(global_prompt_target_in_home(
            Path::new(&home),
            codex_home.as_deref(),
            claude_dir.as_deref(),
            app,
        ))
    }

    /// Resolves the existing Codex login cache without reading, creating, or
    /// modifying it. Codex honors an explicit CODEX_HOME for this user-owned
    /// state just as it does for its global instruction document.
    pub fn codex_auth_path() -> Result<PathBuf, String> {
        let home = user_home_dir()?;
        let codex_home = std::env::var_os("CODEX_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Ok(codex_auth_path_in_home(
            Path::new(&home),
            codex_home.as_deref(),
        ))
    }

    /// Resolves the Claude login cache without reading, creating, or modifying
    /// it. Claude honors an explicit CLAUDE_CONFIG_DIR for this user-owned
    /// state, mirroring how the Codex resolver honors CODEX_HOME.
    pub fn claude_credentials_path() -> Result<PathBuf, String> {
        let home = user_home_dir()?;
        let config_dir = std::env::var_os("CLAUDE_CONFIG_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Ok(claude_credentials_path_in_home(
            Path::new(&home),
            config_dir.as_deref(),
        ))
    }

    /// The Claude CLI's `config.json` beside its login cache. Only the
    /// plugin-integration marker is ever written there, by `claude_integration`.
    pub fn claude_plugin_config_path() -> Result<PathBuf, String> {
        Ok(Self::claude_credentials_path()?.with_file_name("config.json"))
    }

    /// The Claude user document (`~/.claude.json`), resolved exactly as the
    /// extensions workspace resolves it so both writers share one lock path.
    pub fn claude_user_document_path() -> Result<PathBuf, String> {
        let home = user_home_dir()?;
        let config_dir = std::env::var_os("CLAUDE_CONFIG_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Ok(crate::extensions::paths::claude_user_json_path(
            &home,
            config_dir.as_deref(),
        ))
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.root.join("backups")
    }

    /// Private state for the loopback protocol gateway. It contains its
    /// listener identity and the active profile fingerprints, never a copied
    /// upstream API key or a client configuration document.
    pub(crate) fn gateway_state_path(&self) -> PathBuf {
        self.root.join("gateway.json")
    }

    /// Durable, credential-free request history for Claude gateway traffic.
    pub(crate) fn claude_request_ledger_path(&self) -> PathBuf {
        self.root.join("claude-request-ledger.json")
    }

    /// Prompt-document backups stay in their own collection so configuration
    /// restore and undo never offer a document snapshot as a client config.
    pub fn prompt_backup_dir(&self) -> PathBuf {
        self.backup_dir().join("prompts")
    }

    fn settings_path(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    fn codex_reset_cache_path(&self) -> PathBuf {
        self.root.join("codex-reset-cache.json")
    }

    fn codex_quota_baseline_path(&self) -> PathBuf {
        self.root.join("codex-quota-baseline.json")
    }

    fn usage_cache_path(&self) -> PathBuf {
        self.root.join("usage-cache.json")
    }

    fn discovery_cache_path(&self) -> PathBuf {
        self.root.join("discovery-cache.json")
    }

    fn cloud_backup_settings_path(&self) -> PathBuf {
        self.root.join("cloud-backup.json")
    }
}
