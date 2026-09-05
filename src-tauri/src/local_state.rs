//! Application-owned local state: desktop preferences, optional cloud-backup
//! connection coordinates, and the Codex reset-signal cache. Provider,
//! common-settings, and write-history storage live in [`crate::config_store`];
//! this struct only hands out its store and resolves the real client paths,
//! which are never created here.

use crate::codex_official_quota::CodexQuotaBaseline;
use crate::codex_reset::CodexResetStatus;
use crate::config_store::ConfigStore;
use crate::model_usage_cache::ModelUsageCache;
use crate::runtime_log::RuntimeLogLevel;
use crate::usage_cache::UsageCache;
use asb_core::contracts::AppKind;
use asb_core::discovery::DiscoveryReport;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Resolves the user home directory across supported platforms: `USERPROFILE`
/// on Windows, `HOME` elsewhere. The first non-empty value wins.
pub(crate) fn user_home_dir() -> Result<PathBuf, String> {
    for key in ["USERPROFILE", "HOME"] {
        if let Some(home) = std::env::var_os(key).filter(|value| !value.is_empty()) {
            return Ok(PathBuf::from(home));
        }
    }
    Err("无法确定用户主目录".to_string())
}

/// App-runtime preference: controls what a user-visible close request does.
/// It never belongs to Codex or Claude Code configuration files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloseBehavior {
    HideToTray,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MotionPreference {
    System,
    Reduce,
}

/// Bundled web font shipped with the app; also the interface-font default.
pub(crate) const DEFAULT_INTERFACE_FONT: &str = "Noto Sans SC";

/// Application-owned desktop preferences, stored separately from configuration
/// data. This is a strict complete current contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSettings {
    pub close_behavior: CloseBehavior,
    pub theme: ThemePreference,
    pub motion: MotionPreference,
    pub always_on_top: bool,
    pub launch_at_login: bool,
    pub hardware_acceleration: bool,
    pub interface_font: String,
    pub runtime_log_level: RuntimeLogLevel,
    /// Provider ids whose usage panel stays collapsed. A file missing this
    /// field reads as an empty set, so every panel stays expanded by default.
    #[serde(default)]
    pub collapsed_usage_ids: Vec<String>,
}

impl AppSettings {
    /// A font name is used verbatim as a CSS font-family value, so it must be
    /// a plain non-empty name without quotes or control characters.
    pub(crate) fn validate(&self) -> Result<(), String> {
        let valid = self.interface_font.trim().len() == self.interface_font.len()
            && !self.interface_font.is_empty()
            && self.interface_font.len() <= 64
            && !self
                .interface_font
                .chars()
                .any(|character| character.is_control() || matches!(character, '"' | '\'' | '\\'));
        if valid {
            Ok(())
        } else {
            Err("界面字体名称无效：须为非空字体名，且不含首尾空格、引号或控制字符".to_string())
        }
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            close_behavior: CloseBehavior::HideToTray,
            theme: ThemePreference::System,
            motion: MotionPreference::System,
            always_on_top: false,
            launch_at_login: false,
            hardware_acceleration: true,
            interface_font: DEFAULT_INTERFACE_FONT.to_string(),
            runtime_log_level: RuntimeLogLevel::Info,
            collapsed_usage_ids: Vec::new(),
        }
    }
}

/// Connection coordinates for a user-owned Supabase project. The project
/// publishable key identifies a public desktop client; authentication and the
/// separate cloud-backup password are intentionally never written to disk.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloudBackupSettings {
    pub project_url: String,
    pub publishable_key: String,
    pub email: String,
}

impl CloudBackupSettings {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let project_url = self.project_url.trim();
        let publishable_key = self.publishable_key.trim();
        let email = self.email.trim();
        let valid_url = project_url.starts_with("https://")
            && project_url.len() == self.project_url.len()
            && project_url.len() > "https://".len()
            && !project_url.contains(['?', '#', ' '])
            && !project_url.chars().any(char::is_control)
            && !project_url.ends_with('/');
        if !valid_url {
            return Err("Supabase 项目地址必须是无尾随斜杠的 https URL".to_string());
        }
        if publishable_key.is_empty()
            || publishable_key.len() != self.publishable_key.len()
            || publishable_key.len() > 2048
            || publishable_key.chars().any(char::is_control)
        {
            return Err("Supabase publishable key 无效".to_string());
        }
        if email.is_empty()
            || email.len() != self.email.len()
            || email.len() > 320
            || !email.contains('@')
            || email.chars().any(char::is_control)
        {
            return Err("项目 Auth 登录邮箱无效".to_string());
        }
        Ok(())
    }
}

pub struct LocalState {
    root: PathBuf,
}

impl LocalState {
    pub fn from_app(app: &tauri::AppHandle) -> Result<Self, String> {
        use tauri::Manager;

        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|_| "无法定位应用数据目录".to_string())?;
        Ok(Self::from_app_data_dir(app_data_dir))
    }

    /// Resolves the same app-data location Tauri uses before an AppHandle
    /// exists, so startup-only WebView preferences can be read in time.
    pub(crate) fn from_startup_identifier(identifier: &str) -> Result<Self, String> {
        let app_data_dir = dirs::data_dir()
            .ok_or_else(|| "无法定位应用数据目录".to_string())?
            .join(identifier);
        Ok(Self::from_app_data_dir(app_data_dir))
    }

    #[cfg(test)]
    pub(crate) fn from_root(root: PathBuf) -> Self {
        Self { root }
    }

    /// The application configuration store rooted at this state directory.
    /// Every provider, common-settings, and history operation goes through
    /// it; this struct keeps no second copy of that data.
    pub fn configuration(&self) -> ConfigStore {
        ConfigStore::new(self.root.clone())
    }

    pub fn target(&self, app: AppKind) -> Result<PathBuf, String> {
        Self::user_config_path(app)
    }

    /// Resolves the one global instruction document owned by each supported
    /// client. Codex honors an explicit CODEX_HOME when available; Claude
    /// follows its user-level .claude directory.
    pub fn global_prompt_target(&self, app: AppKind) -> Result<PathBuf, String> {
        Self::global_prompt_path(app)
    }

    pub fn user_config_path(app: AppKind) -> Result<PathBuf, String> {
        let home = user_home_dir()?;
        let codex_home = std::env::var_os("CODEX_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Ok(target_in_home(Path::new(&home), codex_home.as_deref(), app))
    }

    pub fn global_prompt_path(app: AppKind) -> Result<PathBuf, String> {
        let home = user_home_dir()?;
        let codex_home = std::env::var_os("CODEX_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Ok(global_prompt_target_in_home(
            Path::new(&home),
            codex_home.as_deref(),
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

    pub fn backup_dir(&self) -> PathBuf {
        self.root.join("backups")
    }

    /// Prompt-document backups stay in their own collection so configuration
    /// restore and undo never offer a document snapshot as a client config.
    pub fn prompt_backup_dir(&self) -> PathBuf {
        self.backup_dir().join("prompts")
    }

    pub fn get_app_settings(&self) -> Result<AppSettings, String> {
        match fs::read_to_string(self.settings_path()) {
            Ok(text) => {
                let settings = serde_json::from_str::<AppSettings>(&text)
                    .map_err(|_| "应用设置格式无效".to_string())?;
                settings
                    .validate()
                    .map_err(|_| "应用设置格式无效".to_string())?;
                Ok(settings)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(AppSettings::default())
            }
            Err(_) => Err("应用设置不可读".to_string()),
        }
    }

    pub fn set_app_settings(&self, settings: &AppSettings) -> Result<(), String> {
        settings.validate()?;
        let content =
            serde_json::to_string_pretty(settings).map_err(|_| "应用设置序列化失败".to_string())?;
        fs::create_dir_all(&self.root).map_err(|_| "无法创建应用数据目录".to_string())?;
        let temporary = self.root.join(format!("settings.{}.tmp", Uuid::new_v4()));
        fs::write(&temporary, content).map_err(|_| "无法写入应用设置临时文件".to_string())?;
        if fs::rename(&temporary, self.settings_path()).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err("无法原子保存应用设置".to_string());
        }
        Ok(())
    }

    /// One-click repair for an unreadable settings file: replaces it with
    /// validated defaults through the same atomic write as a normal save. A
    /// readable settings file is refused, so repair can never silently
    /// discard healthy preferences.
    pub fn repair_app_settings(&self) -> Result<AppSettings, String> {
        if self.get_app_settings().is_ok() {
            return Err("应用设置当前可读，无需修复".to_string());
        }
        let defaults = AppSettings::default();
        self.set_app_settings(&defaults)?;
        Ok(defaults)
    }

    /// The cloud destination is optional. Reading an absent file never
    /// creates state, matching the other app-owned optional snapshots.
    pub fn get_cloud_backup_settings(&self) -> Result<Option<CloudBackupSettings>, String> {
        let text = match fs::read_to_string(self.cloud_backup_settings_path()) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("云端备份设置不可读".to_string()),
        };
        let settings: CloudBackupSettings =
            serde_json::from_str(&text).map_err(|_| "云端备份设置格式无效".to_string())?;
        settings
            .validate()
            .map_err(|_| "云端备份设置格式无效".to_string())?;
        Ok(Some(settings))
    }

    pub fn set_cloud_backup_settings(&self, settings: &CloudBackupSettings) -> Result<(), String> {
        settings.validate()?;
        let content = serde_json::to_string_pretty(settings)
            .map_err(|_| "云端备份设置序列化失败".to_string())?;
        fs::create_dir_all(&self.root).map_err(|_| "无法创建应用数据目录".to_string())?;
        let temporary = self
            .root
            .join(format!("cloud-backup.{}.tmp", Uuid::new_v4()));
        fs::write(&temporary, content).map_err(|_| "无法写入云端备份设置临时文件".to_string())?;
        if fs::rename(&temporary, self.cloud_backup_settings_path()).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err("无法原子保存云端备份设置".to_string());
        }
        Ok(())
    }

    /// The last successfully normalized public signal. An absent file is a
    /// normal first-run state; invalid data is left untouched rather than
    /// silently being mistaken for a current result.
    pub fn load_codex_reset_cache(&self) -> Result<Option<CodexResetStatus>, String> {
        let text = match fs::read_to_string(self.codex_reset_cache_path()) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("Codex 重置信号缓存不可读".to_string()),
        };
        let status: CodexResetStatus =
            serde_json::from_str(&text).map_err(|_| "Codex 重置信号缓存格式无效".to_string())?;
        status
            .validate_cached()
            .map_err(|_| "Codex 重置信号缓存格式无效".to_string())?;
        Ok(Some(status))
    }

    /// Replaces the prior public snapshot atomically after a successful
    /// signal read. It never stores the upstream raw payload or credentials.
    pub fn save_codex_reset_cache(&self, status: &CodexResetStatus) -> Result<(), String> {
        let content = serde_json::to_string_pretty(status)
            .map_err(|_| "Codex 重置信号缓存序列化失败".to_string())?;
        fs::create_dir_all(&self.root).map_err(|_| "无法创建应用数据目录".to_string())?;
        let temporary = self
            .root
            .join(format!("codex-reset-cache.{}.tmp", Uuid::new_v4()));
        fs::write(&temporary, content)
            .map_err(|_| "无法写入 Codex 重置信号缓存临时文件".to_string())?;
        if fs::rename(&temporary, self.codex_reset_cache_path()).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err("无法原子保存 Codex 重置信号缓存".to_string());
        }
        Ok(())
    }

    /// The persisted comparison baseline for after-the-fact Codex quota-reset
    /// detection. An absent file is a normal first-run state; invalid data is
    /// left untouched rather than silently mistaken for a current baseline.
    pub fn load_codex_quota_baseline(&self) -> Result<Option<CodexQuotaBaseline>, String> {
        let text = match fs::read_to_string(self.codex_quota_baseline_path()) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("Codex 官方额度基线不可读".to_string()),
        };
        let baseline: CodexQuotaBaseline =
            serde_json::from_str(&text).map_err(|_| "Codex 官方额度基线格式无效".to_string())?;
        baseline
            .validate()
            .map_err(|_| "Codex 官方额度基线格式无效".to_string())?;
        Ok(Some(baseline))
    }

    /// Replaces the prior baseline atomically after a successful official
    /// read. It never stores credentials or raw upstream payloads.
    pub fn save_codex_quota_baseline(&self, baseline: &CodexQuotaBaseline) -> Result<(), String> {
        let content = serde_json::to_string_pretty(baseline)
            .map_err(|_| "Codex 官方额度基线序列化失败".to_string())?;
        fs::create_dir_all(&self.root).map_err(|_| "无法创建应用数据目录".to_string())?;
        let temporary = self
            .root
            .join(format!("codex-quota-baseline.{}.tmp", Uuid::new_v4()));
        fs::write(&temporary, content)
            .map_err(|_| "无法写入 Codex 官方额度基线临时文件".to_string())?;
        if fs::rename(&temporary, self.codex_quota_baseline_path()).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err("无法原子保存 Codex 官方额度基线".to_string());
        }
        Ok(())
    }

    /// The last successful provider-usage snapshots for the custom tray panel. The
    /// file contains normalized readings plus query digests only: no API key,
    /// endpoint, raw response, or usage-script source is persisted here.
    pub(crate) fn load_usage_cache(&self) -> Result<Option<UsageCache>, String> {
        let text = match fs::read_to_string(self.usage_cache_path()) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("托盘用量缓存不可读".to_string()),
        };
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|_| "托盘用量缓存格式无效".to_string())
    }

    pub(crate) fn save_usage_cache(&self, cache: &UsageCache) -> Result<(), String> {
        let content = serde_json::to_string_pretty(cache)
            .map_err(|_| "托盘用量缓存序列化失败".to_string())?;
        fs::create_dir_all(&self.root).map_err(|_| "无法创建应用数据目录".to_string())?;
        let temporary = self
            .root
            .join(format!("usage-cache.{}.tmp", Uuid::new_v4()));
        fs::write(&temporary, content).map_err(|_| "无法写入托盘用量缓存临时文件".to_string())?;
        if fs::rename(&temporary, self.usage_cache_path()).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err("无法原子保存托盘用量缓存".to_string());
        }
        Ok(())
    }

    pub(crate) fn clear_usage_cache(&self) -> Result<(), String> {
        match fs::remove_file(self.usage_cache_path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("无法清除托盘用量缓存".to_string()),
        }
    }

    /// The last successful local-session usage reports. It contains only
    /// normalized token totals and model labels, never a credential, endpoint,
    /// raw session record, or provider-quota reading.
    pub(crate) fn load_model_usage_cache(&self) -> Result<Option<ModelUsageCache>, String> {
        let text = match fs::read_to_string(self.model_usage_cache_path()) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("本地会话快照不可读".to_string()),
        };
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|_| "本地会话快照格式无效".to_string())
    }

    /// Replaces every range snapshot atomically after a local-session scan.
    pub(crate) fn save_model_usage_cache(&self, cache: &ModelUsageCache) -> Result<(), String> {
        let content = serde_json::to_string_pretty(cache)
            .map_err(|_| "本地会话快照序列化失败".to_string())?;
        fs::create_dir_all(&self.root).map_err(|_| "无法创建应用数据目录".to_string())?;
        let temporary = self
            .root
            .join(format!("model-usage-cache.{}.tmp", Uuid::new_v4()));
        fs::write(&temporary, content).map_err(|_| "无法写入本地会话快照临时文件".to_string())?;
        if fs::rename(&temporary, self.model_usage_cache_path()).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err("无法原子保存本地会话快照".to_string());
        }
        Ok(())
    }

    /// The last successful local discovery scan, for display on the 发现 page
    /// before the next scan runs. The stored copy never carries credentials:
    /// import re-derives the draft from the live files. An absent file is a
    /// normal first-run state; invalid data is left untouched rather than
    /// silently mistaken for a current result.
    pub fn load_discovery_cache(&self) -> Result<Option<DiscoveryReport>, String> {
        let text = match fs::read_to_string(self.discovery_cache_path()) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("发现扫描缓存不可读".to_string()),
        };
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|_| "发现扫描缓存格式无效".to_string())
    }

    pub fn save_discovery_cache(&self, report: &DiscoveryReport) -> Result<(), String> {
        let cached = report.cached_display();
        let content = serde_json::to_string_pretty(&cached)
            .map_err(|_| "发现扫描缓存序列化失败".to_string())?;
        fs::create_dir_all(&self.root).map_err(|_| "无法创建应用数据目录".to_string())?;
        let temporary = self
            .root
            .join(format!("discovery-cache.{}.tmp", Uuid::new_v4()));
        fs::write(&temporary, content).map_err(|_| "无法写入发现扫描缓存临时文件".to_string())?;
        if fs::rename(&temporary, self.discovery_cache_path()).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err("无法原子保存发现扫描缓存".to_string());
        }
        Ok(())
    }

    pub fn clear_discovery_cache(&self) -> Result<(), String> {
        match fs::remove_file(self.discovery_cache_path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("无法清除发现扫描缓存".to_string()),
        }
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

    fn model_usage_cache_path(&self) -> PathBuf {
        self.root.join("model-usage-cache.json")
    }

    /// The single credential-free usage-history ledger. Its schema, parsing,
    /// pruning, and atomic replacement are owned by `usage_history`.
    pub(crate) fn usage_history_path(&self) -> PathBuf {
        self.root.join("usage-history.json")
    }

    fn discovery_cache_path(&self) -> PathBuf {
        self.root.join("discovery-cache.json")
    }

    fn cloud_backup_settings_path(&self) -> PathBuf {
        self.root.join("cloud-backup.json")
    }

    fn from_app_data_dir(app_data_dir: PathBuf) -> Self {
        Self {
            root: app_data_dir.join("state"),
        }
    }
}

fn target_in_home(home: &Path, codex_home: Option<&Path>, app: AppKind) -> PathBuf {
    match app {
        AppKind::Codex => match codex_home {
            Some(directory) => directory.join("config.toml"),
            None => home.join(".codex").join("config.toml"),
        },
        AppKind::Claude => home.join(".claude").join("settings.json"),
    }
}

fn global_prompt_target_in_home(home: &Path, codex_home: Option<&Path>, app: AppKind) -> PathBuf {
    match app {
        AppKind::Codex => match codex_home {
            Some(directory) => directory.join(app.global_prompt_file_name()),
            None => home.join(".codex").join(app.global_prompt_file_name()),
        },
        AppKind::Claude => home.join(".claude").join(app.global_prompt_file_name()),
    }
}

fn codex_auth_path_in_home(home: &Path, codex_home: Option<&Path>) -> PathBuf {
    match codex_home {
        Some(directory) => directory.join("auth.json"),
        None => home.join(".codex").join("auth.json"),
    }
}

fn claude_credentials_path_in_home(home: &Path, config_dir: Option<&Path>) -> PathBuf {
    match config_dir {
        Some(directory) => directory.join(".credentials.json"),
        None => home.join(".claude").join(".credentials.json"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex_official_quota::BaselineRead;
    use crate::codex_reset::{CodexResetFeedStatus, ResetSignal, ResetType};
    use asb_core::contracts::{
        CodexOfficialQuotaReset, CodexOfficialQuotaResetKind, CodexOfficialQuotaWindow,
    };

    fn cached_reset_status() -> CodexResetStatus {
        CodexResetStatus {
            source_url: "https://www.codexrunway.com/api/status.json".to_string(),
            feed_status: CodexResetFeedStatus::Ok,
            generated_at: "2026-08-31T03:08:02.232Z".to_string(),
            last_successful_check_at: "2026-08-31T03:08:02.232Z".to_string(),
            checked_at: "2026-08-31T03:10:00.000Z".to_string(),
            latest_confirmed_signal: Some(ResetSignal {
                announced_at: "2026-08-31T02:34:27Z".to_string(),
                effective_at: None,
                schedule_precision: None,
                confidence: 0.98,
                reset_type: ResetType::Global,
            }),
            next_scheduled_reset: None,
            latest_relevant_tibo_post: None,
            source_warning: None,
        }
    }

    fn stored_baseline() -> CodexQuotaBaseline {
        CodexQuotaBaseline {
            account_marker: Some("account-marker".to_string()),
            last_read: Some(BaselineRead {
                at: "2026-09-01T08:00:00Z".to_string(),
                windows: vec![CodexOfficialQuotaWindow {
                    label: "7 天".to_string(),
                    used_percent: 42.5,
                    resets_at: Some("2026-09-04T00:00:00Z".to_string()),
                }],
            }),
            last_reset: Some(CodexOfficialQuotaReset {
                observed_at: "2026-08-31T02:34:27Z".to_string(),
                kind: CodexOfficialQuotaResetKind::Scheduled,
                resets_at: Some("2026-09-04T00:00:00Z".to_string()),
            }),
        }
    }

    #[test]
    fn new_state_is_empty_and_does_not_create_configuration_files() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));

        assert!(!state.configuration().legacy_store_path().exists());
        assert!(!state.configuration().configuration_dir().exists());
        let target = target_in_home(&directory.path().join("home"), None, AppKind::Codex);
        assert!(!target.exists());
    }

    #[test]
    fn global_prompt_targets_use_the_supported_client_document_names() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let home = directory.path().join("home");
        let codex_home = directory.path().join("custom-codex-home");

        assert_eq!(
            global_prompt_target_in_home(&home, None, AppKind::Codex),
            home.join(".codex").join("AGENTS.md")
        );
        assert_eq!(
            global_prompt_target_in_home(&home, Some(&codex_home), AppKind::Codex),
            codex_home.join("AGENTS.md")
        );
        assert_eq!(
            global_prompt_target_in_home(&home, None, AppKind::Claude),
            home.join(".claude").join("CLAUDE.md")
        );
    }

    #[test]
    fn codex_auth_target_uses_the_same_home_resolution_without_creating_it() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let home = directory.path().join("home");
        let codex_home = directory.path().join("alternate-codex-home");

        assert_eq!(
            codex_auth_path_in_home(&home, None),
            home.join(".codex").join("auth.json")
        );
        assert_eq!(
            codex_auth_path_in_home(&home, Some(&codex_home)),
            codex_home.join("auth.json")
        );
        assert!(!home.exists());
        assert!(!codex_home.exists());
    }

    #[test]
    fn codex_config_target_follows_codex_home() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let home = directory.path().join("home");
        let codex_home = directory.path().join("alternate-codex-home");

        assert_eq!(
            target_in_home(&home, Some(&codex_home), AppKind::Codex),
            codex_home.join("config.toml")
        );
    }

    #[test]
    fn claude_credentials_target_follows_the_config_dir_without_creating_it() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let home = directory.path().join("home");
        let config_dir = directory.path().join("alternate-claude-config");

        assert_eq!(
            claude_credentials_path_in_home(&home, None),
            home.join(".claude").join(".credentials.json")
        );
        assert_eq!(
            claude_credentials_path_in_home(&home, Some(&config_dir)),
            config_dir.join(".credentials.json")
        );
        assert!(!home.exists());
        assert!(!config_dir.exists());
    }

    #[test]
    fn codex_reset_cache_is_absent_without_creating_a_file() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));

        assert_eq!(state.load_codex_reset_cache().expect("empty cache"), None);
        assert!(!state.codex_reset_cache_path().exists());
    }

    #[test]
    fn cloud_backup_settings_are_optional_and_persist_without_passwords() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let settings = CloudBackupSettings {
            project_url: "https://example.supabase.co".to_string(),
            publishable_key: "sb_publishable_example".to_string(),
            email: "backup@example.com".to_string(),
        };

        assert_eq!(state.get_cloud_backup_settings().expect("absent"), None);
        state
            .set_cloud_backup_settings(&settings)
            .expect("save settings");

        assert_eq!(
            state.get_cloud_backup_settings().expect("reload settings"),
            Some(settings)
        );
        let stored = fs::read_to_string(state.cloud_backup_settings_path()).expect("stored text");
        assert!(!stored.contains("password"));
    }

    #[test]
    fn cloud_backup_settings_reject_a_non_https_or_noncanonical_project_url() {
        let invalid = CloudBackupSettings {
            project_url: "https://example.supabase.co/".to_string(),
            publishable_key: "sb_publishable_example".to_string(),
            email: "backup@example.com".to_string(),
        };

        assert_eq!(
            invalid.validate().expect_err("trailing slash"),
            "Supabase 项目地址必须是无尾随斜杠的 https URL"
        );
    }

    #[test]
    fn codex_reset_cache_replaces_and_persists_the_latest_successful_snapshot() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("state");
        let state = LocalState::from_root(root.clone());
        let first = cached_reset_status();
        let latest = CodexResetStatus {
            checked_at: "2026-08-31T04:10:00.000Z".to_string(),
            next_scheduled_reset: Some(ResetSignal {
                announced_at: "2026-08-31T04:00:00Z".to_string(),
                effective_at: Some("2026-08-31T09:00:00Z".to_string()),
                schedule_precision: Some("datetime".to_string()),
                confidence: 0.84,
                reset_type: ResetType::Global,
            }),
            ..first.clone()
        };

        state
            .save_codex_reset_cache(&first)
            .expect("save first cache");
        state
            .save_codex_reset_cache(&latest)
            .expect("replace cache with latest snapshot");

        let reopened = LocalState::from_root(root);
        assert_eq!(
            reopened.load_codex_reset_cache().expect("read cache"),
            Some(latest)
        );
    }

    #[test]
    fn legacy_codex_reset_cache_is_rejected_without_rewriting_it() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");
        let legacy = r#"{"sourceUrl":"https://www.codexrunway.com/api/status.json","feedStatus":"ok","generatedAt":"2026-08-31T03:08:02.232Z","lastSuccessfulCheckAt":"2026-08-31T03:08:02.232Z","checkedAt":"2026-08-31T03:10:00.000Z","latestConfirmedReset":null,"nextScheduledReset":null,"latestRelevantTiboPost":null,"sourceWarning":null}"#;
        fs::write(state.codex_reset_cache_path(), legacy).expect("write legacy cache");

        assert_eq!(
            state.load_codex_reset_cache().unwrap_err(),
            "Codex 重置信号缓存格式无效"
        );
        assert_eq!(
            fs::read_to_string(state.codex_reset_cache_path()).expect("read legacy cache"),
            legacy
        );
    }

    #[test]
    fn codex_quota_baseline_is_absent_without_creating_a_file() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));

        assert_eq!(
            state.load_codex_quota_baseline().expect("empty baseline"),
            None
        );
        assert!(!state.codex_quota_baseline_path().exists());
    }

    #[test]
    fn codex_quota_baseline_replaces_and_persists_the_latest_read() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("state");
        let state = LocalState::from_root(root.clone());
        let first = stored_baseline();
        let latest = CodexQuotaBaseline {
            account_marker: Some("replacement-marker".to_string()),
            last_read: None,
            last_reset: None,
        };

        state
            .save_codex_quota_baseline(&first)
            .expect("save first baseline");
        state
            .save_codex_quota_baseline(&latest)
            .expect("replace baseline with latest read");

        let reopened = LocalState::from_root(root);
        assert_eq!(
            reopened.load_codex_quota_baseline().expect("read baseline"),
            Some(latest)
        );
    }

    #[test]
    fn malformed_codex_quota_baseline_is_rejected_without_rewriting_it() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");
        let invalid = r#"{"accountMarker":"marker","lastRead":{"at":"not-a-time","windows":[]},"lastReset":null}"#;
        fs::write(state.codex_quota_baseline_path(), invalid).expect("write invalid baseline");

        assert_eq!(
            state.load_codex_quota_baseline().unwrap_err(),
            "Codex 官方额度基线格式无效"
        );
        assert_eq!(
            fs::read_to_string(state.codex_quota_baseline_path()).expect("read invalid baseline"),
            invalid
        );
    }

    #[test]
    fn discovery_cache_is_absent_without_creating_a_file() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));

        assert_eq!(state.load_discovery_cache().expect("empty cache"), None);
        assert!(!state.discovery_cache_path().exists());
    }

    #[test]
    fn discovery_cache_persists_display_facts_and_never_credentials() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("state");
        let state = LocalState::from_root(root.clone());
        let report = asb_core::discovery::discover(
            &asb_core::discovery::DiscoveryPaths {
                codex: "missing-codex.toml".into(),
                codex_auth: "missing-auth.json".into(),
                claude: "claude.json".into(),
            },
            |path| {
                if path == "claude.json" {
                    Ok(Some(
                        r#"{"env":{"ANTHROPIC_BASE_URL":"https://relay.internal","ANTHROPIC_AUTH_TOKEN":"TEST_CACHE_REDACTED_KEY","ANTHROPIC_MODEL":"claude-opus-4-1"}}"#
                            .to_string(),
                    ))
                } else {
                    Ok(None)
                }
            },
        );
        assert!(!report.codex.exists);
        assert_eq!(report.import_proposals.len(), 1);
        assert_eq!(
            report.import_proposals[0].draft.api_key,
            "TEST_CACHE_REDACTED_KEY"
        );

        state
            .save_discovery_cache(&report)
            .expect("save discovery cache");
        let stored = fs::read_to_string(state.discovery_cache_path()).expect("stored text");
        assert!(!stored.contains("TEST_CACHE_REDACTED_KEY"));

        let cached = LocalState::from_root(root)
            .load_discovery_cache()
            .expect("read cache");
        assert_eq!(cached, Some(report.cached_display()));
    }

    #[test]
    fn clear_discovery_cache_removes_the_stored_snapshot() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("state");
        let state = LocalState::from_root(root.clone());
        let report = asb_core::discovery::discover(
            &asb_core::discovery::DiscoveryPaths {
                codex: "c".into(),
                codex_auth: "a".into(),
                claude: "s".into(),
            },
            |_| Ok(Some(String::new())),
        );

        state
            .save_discovery_cache(&report)
            .expect("save discovery cache");
        state.clear_discovery_cache().expect("clear cache");
        state.clear_discovery_cache().expect("clear is idempotent");

        assert_eq!(
            LocalState::from_root(root)
                .load_discovery_cache()
                .expect("read cleared cache"),
            None
        );
    }

    #[test]
    fn app_settings_default_without_creating_a_file() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));

        assert_eq!(
            state.get_app_settings().expect("default settings"),
            AppSettings::default()
        );
        assert!(!state.settings_path().exists());
    }

    #[test]
    fn app_settings_persist_as_the_current_contract_only() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("state");
        let state = LocalState::from_root(root.clone());
        let expected = AppSettings {
            close_behavior: CloseBehavior::Exit,
            theme: ThemePreference::Dark,
            motion: MotionPreference::Reduce,
            always_on_top: true,
            launch_at_login: true,
            hardware_acceleration: false,
            interface_font: "MiSans".to_string(),
            runtime_log_level: RuntimeLogLevel::Warn,
            collapsed_usage_ids: vec!["codex-relay-a".to_string()],
        };

        state.set_app_settings(&expected).expect("save settings");
        let reopened = LocalState::from_root(root);
        assert_eq!(
            reopened.get_app_settings().expect("read settings"),
            expected
        );
        let written = fs::read_to_string(reopened.settings_path()).expect("read settings text");
        assert_eq!(
            written,
            "{\n  \"closeBehavior\": \"exit\",\n  \"theme\": \"dark\",\n  \"motion\": \"reduce\",\n  \"alwaysOnTop\": true,\n  \"launchAtLogin\": true,\n  \"hardwareAcceleration\": false,\n  \"interfaceFont\": \"MiSans\",\n  \"runtimeLogLevel\": \"warn\",\n  \"collapsedUsageIds\": [\n    \"codex-relay-a\"\n  ]\n}"
        );
        assert!(!reopened.backup_dir().exists());
    }

    #[test]
    fn a_settings_file_without_the_usage_collapse_set_reads_as_expanded() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");
        fs::write(
            state.settings_path(),
            "{\"closeBehavior\":\"hideToTray\",\"theme\":\"dark\",\"motion\":\"reduce\",\"alwaysOnTop\":true,\"launchAtLogin\":false,\"hardwareAcceleration\":true,\"interfaceFont\":\"Noto Sans SC\",\"runtimeLogLevel\":\"info\"}",
        )
        .expect("write settings without the collapse set");

        let settings = state.get_app_settings().expect("read settings");
        assert_eq!(settings.collapsed_usage_ids, Vec::<String>::new());
        // Every required preference survives the read untouched.
        assert_eq!(settings.theme, ThemePreference::Dark);
        assert!(settings.always_on_top);
    }

    #[test]
    fn app_settings_reject_incomplete_unknown_and_invalid_shapes() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");

        for invalid in [
            // Any previous or incomplete settings shape is rejected.
            "{\"closeBehavior\":\"hideToTray\"}",
            "{\"closeBehavior\":\"exit\",\"theme\":\"dark\",\"motion\":\"reduce\",\"alwaysOnTop\":true,\"hardwareAcceleration\":false,\"interfaceFont\":\"Noto Sans SC\",\"runtimeLogLevel\":\"info\"}",
            "{\"closeBehavior\":\"hideToTray\",\"theme\":\"system\",\"motion\":\"system\",\"alwaysOnTop\":false,\"launchAtLogin\":false,\"hardwareAcceleration\":true,\"interfaceFont\":\"Noto Sans SC\",\"runtimeLogLevel\":\"info\",\"legacy\":true}",
            "{\"closeBehavior\":\"hideToTray\",\"theme\":\"sepia\",\"motion\":\"system\",\"alwaysOnTop\":false,\"launchAtLogin\":false,\"hardwareAcceleration\":true,\"interfaceFont\":\"Noto Sans SC\",\"runtimeLogLevel\":\"info\"}",
            "{\"closeBehavior\":\"hideToTray\",\"theme\":\"system\",\"motion\":\"system\",\"alwaysOnTop\":false,\"launchAtLogin\":false,\"hardwareAcceleration\":\"false\",\"interfaceFont\":\"Noto Sans SC\",\"runtimeLogLevel\":\"info\"}",
            "{\"closeBehavior\":\"exit\",\"theme\":\"dark\",\"motion\":\"reduce\",\"alwaysOnTop\":true,\"hardwareAcceleration\":true}",
            "{\"closeBehavior\":\"exit\",\"theme\":\"dark\",\"motion\":\"reduce\",\"alwaysOnTop\":true,\"launchAtLogin\":false,\"hardwareAcceleration\":true,\"interfaceFont\":\"Noto Sans SC\",\"runtimeLogLevel\":\"verbose\"}",
            // A font name is consumed verbatim as a quoted CSS value.
            "{\"closeBehavior\":\"exit\",\"theme\":\"dark\",\"motion\":\"reduce\",\"alwaysOnTop\":true,\"launchAtLogin\":false,\"hardwareAcceleration\":true,\"interfaceFont\":\"\",\"runtimeLogLevel\":\"info\"}",
            "{\"closeBehavior\":\"exit\",\"theme\":\"dark\",\"motion\":\"reduce\",\"alwaysOnTop\":true,\"launchAtLogin\":false,\"hardwareAcceleration\":true,\"interfaceFont\":\"MiSans \",\"runtimeLogLevel\":\"info\"}",
        ] {
            fs::write(state.settings_path(), invalid).expect("write invalid settings");
            assert_eq!(state.get_app_settings().unwrap_err(), "应用设置格式无效");
        }
    }

    #[test]
    fn app_settings_repair_replaces_an_invalid_file_with_defaults() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");
        // A previous-contract shape without launchAtLogin is the repair case.
        fs::write(
            state.settings_path(),
            "{\"closeBehavior\":\"exit\",\"theme\":\"dark\",\"motion\":\"reduce\",\"alwaysOnTop\":true,\"hardwareAcceleration\":false,\"interfaceFont\":\"MiSans\",\"runtimeLogLevel\":\"warn\"}",
        )
        .expect("write stale settings");

        assert_eq!(
            state.repair_app_settings().expect("repair"),
            AppSettings::default()
        );
        assert_eq!(
            state.get_app_settings().expect("read repaired settings"),
            AppSettings::default()
        );
    }

    #[test]
    fn app_settings_repair_refuses_a_readable_file() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let saved = AppSettings {
            close_behavior: CloseBehavior::Exit,
            interface_font: "MiSans".to_string(),
            ..AppSettings::default()
        };
        state.set_app_settings(&saved).expect("save settings");

        assert_eq!(
            state.repair_app_settings().unwrap_err(),
            "应用设置当前可读，无需修复"
        );
        assert_eq!(state.get_app_settings().expect("read settings"), saved);
    }

    #[test]
    fn invalid_interface_font_names_are_rejected_before_any_write() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));

        let names = ["", "  ", "\"quoted\"", "back\\slash", "MiSans "]
            .into_iter()
            .map(str::to_string)
            .chain([core::iter::repeat('x').take(65).collect::<String>()]);
        for name in names {
            let settings = AppSettings {
                interface_font: name,
                ..AppSettings::default()
            };
            assert!(state.set_app_settings(&settings).is_err());
        }
        assert!(!state.settings_path().exists());
    }
}
