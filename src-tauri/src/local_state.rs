//! Application-owned local state: desktop preferences, optional cloud-backup
//! connection coordinates, and the Codex reset-signal cache. Provider,
//! common-settings, and write-history storage live in [`crate::config_store`];
//! this struct only hands out its store and resolves the real client paths,
//! which are never created here.

use crate::codex_reset::CodexResetStatus;
use crate::config_store::ConfigStore;
use crate::runtime_log::RuntimeLogLevel;
use asb_core::contracts::AppKind;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

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
            return Err("Supabase 登录邮箱无效".to_string());
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
        let home = std::env::var_os("USERPROFILE")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "无法确定 Windows 用户目录".to_string())?;
        Ok(target_in_home(Path::new(&home), app))
    }

    pub fn global_prompt_path(app: AppKind) -> Result<PathBuf, String> {
        let home = std::env::var_os("USERPROFILE")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "无法确定 Windows 用户目录".to_string())?;
        let codex_home = std::env::var_os("CODEX_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Ok(global_prompt_target_in_home(
            Path::new(&home),
            codex_home.as_deref(),
            app,
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

    fn settings_path(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    fn codex_reset_cache_path(&self) -> PathBuf {
        self.root.join("codex-reset-cache.json")
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

fn target_in_home(home: &Path, app: AppKind) -> PathBuf {
    match app {
        AppKind::Codex => home.join(".codex").join("config.toml"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex_reset::{CodexResetFeedStatus, ResetSignal};

    fn cached_reset_status() -> CodexResetStatus {
        CodexResetStatus {
            source_url: "https://www.codexrunway.com/api/status.json".to_string(),
            feed_status: CodexResetFeedStatus::Ok,
            generated_at: "2026-08-31T03:08:02.232Z".to_string(),
            last_successful_check_at: "2026-08-31T03:08:02.232Z".to_string(),
            checked_at: "2026-08-31T03:10:00.000Z".to_string(),
            latest_confirmed_reset: Some(ResetSignal {
                announced_at: "2026-08-31T02:34:27Z".to_string(),
                effective_at: None,
                schedule_precision: None,
                confidence: 0.98,
            }),
            next_scheduled_reset: None,
            latest_relevant_tibo_post: None,
            source_warning: None,
        }
    }

    #[test]
    fn new_state_is_empty_and_does_not_create_configuration_files() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));

        assert!(!state.configuration().legacy_store_path().exists());
        assert!(!state.configuration().configuration_dir().exists());
        let target = target_in_home(&directory.path().join("home"), AppKind::Codex);
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
    fn malformed_codex_reset_cache_is_rejected_without_rewriting_it() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");
        let invalid = r#"{"feedStatus":"ok","generatedAt":"2026-08-31T03:08:02.232Z","lastSuccessfulCheckAt":"2026-08-31T03:08:02.232Z","checkedAt":"bad-time","latestConfirmedReset":null,"nextScheduledReset":null,"latestRelevantTiboPost":null,"sourceWarning":null}"#;
        fs::write(state.codex_reset_cache_path(), invalid).expect("write invalid cache");

        assert_eq!(
            state.load_codex_reset_cache().unwrap_err(),
            "Codex 重置信号缓存格式无效"
        );
        assert_eq!(
            fs::read_to_string(state.codex_reset_cache_path()).expect("read invalid cache"),
            invalid
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
            "{\n  \"closeBehavior\": \"exit\",\n  \"theme\": \"dark\",\n  \"motion\": \"reduce\",\n  \"alwaysOnTop\": true,\n  \"launchAtLogin\": true,\n  \"hardwareAcceleration\": false,\n  \"interfaceFont\": \"MiSans\",\n  \"runtimeLogLevel\": \"warn\"\n}"
        );
        assert!(!reopened.backup_dir().exists());
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
