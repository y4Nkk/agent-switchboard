//! Application-owned local state.
//!
//! Supplier profiles and general overlays live in the app data directory.
//! They start empty and are never synthesized from example data. Codex and
//! Claude Code configuration paths are resolved separately and are not
//! created by this module.

use crate::codex_reset::CodexResetStatus;
use crate::runtime_log::RuntimeLogLevel;
use asb_core::{AppKind, CommonConfigPatch, ProviderDraft, ProviderProfile, SwitchLog};
use std::collections::{HashSet, VecDeque};
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

/// Application-owned desktop preferences, stored separately from profile data.
/// This is a strict complete contract. The only migration is the exact
/// immediately previous complete shape, which is atomically rewritten with
/// the bundled default runtime-log level.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSettings {
    pub close_behavior: CloseBehavior,
    pub theme: ThemePreference,
    pub motion: MotionPreference,
    pub always_on_top: bool,
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

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileStore {
    pub profiles: Vec<ProviderProfile>,
    pub common: Vec<CommonConfigPatch>,
    pub switch_log: Vec<SwitchLog>,
}

pub struct LocalState {
    root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileStoreError {
    Unreadable,
    Unsupported,
}

impl std::fmt::Display for ProfileStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable => formatter.write_str("供应商存储不可读"),
            Self::Unsupported => formatter
                .write_str("供应商存储格式无效或来自已不受支持的旧版本；请重新创建供应商档案"),
        }
    }
}

fn validate_store(store: &ProfileStore) -> Result<(), ProfileStoreError> {
    validate_store_for_write(store).map_err(|_| ProfileStoreError::Unsupported)
}

/// The write boundary validates both the shared serializable contract and
/// the desktop-only JavaScript loading contract. Persistence therefore never
/// accepts a script that could only fail later when a user clicks test.
fn validate_store_for_write(store: &ProfileStore) -> Result<(), String> {
    for profile in &store.profiles {
        profile.validate().map_err(|error| error.to_string())?;
        if let Some(query) = &profile.usage_query {
            crate::usage_query::validate_persisted(query)?;
        }
    }
    for patch in &store.common {
        patch.validate().map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Parses the one current profile-store contract, with the only permitted
/// migration performed before typed deserialization: a complete old bare
/// declarative `usageQuery` object gains `kind: "declarative"`. This is a
/// migration boundary, not an old runtime read path.
fn parse_profile_store(text: &str) -> Result<(ProfileStore, bool), ProfileStoreError> {
    let mut raw: serde_json::Value =
        serde_json::from_str(text).map_err(|_| ProfileStoreError::Unsupported)?;
    let migrated = migrate_legacy_usage_queries(&mut raw)?;
    let store: ProfileStore =
        serde_json::from_value(raw).map_err(|_| ProfileStoreError::Unsupported)?;
    validate_store(&store)?;
    Ok((store, migrated))
}

/// Accepts exactly the former declarative field set and no other untagged
/// shapes. Any unknown key, missing URL, or wrong value type rejects the
/// whole store before it can be written back.
fn migrate_legacy_usage_queries(raw: &mut serde_json::Value) -> Result<bool, ProfileStoreError> {
    let root = raw.as_object_mut().ok_or(ProfileStoreError::Unsupported)?;
    let profiles = root
        .get_mut("profiles")
        .ok_or(ProfileStoreError::Unsupported)?
        .as_array_mut()
        .ok_or(ProfileStoreError::Unsupported)?;
    let mut migrated = false;

    for profile in profiles {
        let profile = profile
            .as_object_mut()
            .ok_or(ProfileStoreError::Unsupported)?;
        let Some(query) = profile.get_mut("usageQuery") else {
            continue;
        };
        if query.is_null() {
            continue;
        }
        let query = query
            .as_object_mut()
            .ok_or(ProfileStoreError::Unsupported)?;
        if query.contains_key("kind") {
            continue;
        }
        let allowed = ["url", "remainingPath", "usedPath", "totalPath", "unit"];
        if query.keys().any(|key| !allowed.contains(&key.as_str()))
            || !matches!(query.get("url"), Some(serde_json::Value::String(_)))
            || query.iter().any(|(key, value)| {
                key != "url"
                    && !matches!(
                        value,
                        serde_json::Value::String(_) | serde_json::Value::Null
                    )
            })
        {
            return Err(ProfileStoreError::Unsupported);
        }
        query.insert(
            "kind".to_string(),
            serde_json::Value::String("declarative".to_string()),
        );
        migrated = true;
    }
    Ok(migrated)
}

fn same_profile(profile: &ProviderProfile, draft: &ProviderDraft) -> bool {
    profile.app == draft.app
        && profile.name == draft.name
        && profile.model == draft.model
        && profile.base_url == draft.base_url
        && profile.api_key == draft.api_key
        && profile.model_options == draft.model_options
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

    pub fn load_store(&self) -> Result<ProfileStore, ProfileStoreError> {
        let path = self.store_path();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ProfileStore::default());
            }
            Err(_) => return Err(ProfileStoreError::Unreadable),
        };
        let (store, migrated) = parse_profile_store(&text)?;
        if migrated {
            self.save_store(&store)
                .map_err(|_| ProfileStoreError::Unreadable)?;
        }
        Ok(store)
    }

    pub fn reset_profile_store(&self) -> Result<(), String> {
        self.save_store(&ProfileStore::default())
    }

    pub fn list_profiles(&self) -> Result<Vec<ProviderProfile>, ProfileStoreError> {
        Ok(self.load_store()?.profiles)
    }

    pub fn create_profile(&self, draft: ProviderDraft) -> Result<ProviderProfile, String> {
        draft.validate().map_err(|error| error.to_string())?;
        let mut store = self.load_store().map_err(|error| error.to_string())?;
        let profile = ProviderProfile::from_draft(Uuid::new_v4().to_string(), draft);
        store.profiles.push(profile.clone());
        self.save_store(&store)?;
        Ok(profile)
    }

    pub fn update_profile(
        &self,
        profile_id: &str,
        draft: ProviderDraft,
    ) -> Result<ProviderProfile, String> {
        draft.validate().map_err(|error| error.to_string())?;
        let mut store = self.load_store().map_err(|error| error.to_string())?;
        let index = store
            .profiles
            .iter()
            .position(|profile| profile.id == profile_id)
            .ok_or_else(|| "供应商不存在".to_string())?;
        if store.profiles[index].app != draft.app {
            return Err("供应商不能变更所属客户端".to_string());
        }
        let profile = ProviderProfile::from_draft(profile_id.to_string(), draft);
        store.profiles[index] = profile.clone();
        self.save_store(&store)?;
        Ok(profile)
    }

    pub fn delete_profile(&self, profile_id: &str) -> Result<(), String> {
        let mut store = self.load_store().map_err(|error| error.to_string())?;
        let original_len = store.profiles.len();
        store.profiles.retain(|profile| profile.id != profile_id);
        if store.profiles.len() == original_len {
            return Err("供应商不存在".to_string());
        }
        self.save_store(&store)
    }

    /// Rewrites one client's slice of the profiles vector to match
    /// `ordered_ids`. The vector is the single owner of display order;
    /// profiles of the other client keep their absolute positions.
    pub fn reorder_profiles(
        &self,
        app: AppKind,
        ordered_ids: &[String],
    ) -> Result<Vec<ProviderProfile>, String> {
        let mut store = self.load_store().map_err(|error| error.to_string())?;
        let ordered: Vec<ProviderProfile> = ordered_ids
            .iter()
            .map(|id| {
                store
                    .profiles
                    .iter()
                    .find(|profile| profile.id == *id && profile.app == app)
                    .cloned()
                    .ok_or_else(|| format!("排序清单包含不属于该客户端的供应商：{id}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let unique: HashSet<&str> = ordered_ids.iter().map(String::as_str).collect();
        let app_count = store
            .profiles
            .iter()
            .filter(|profile| profile.app == app)
            .count();
        if unique.len() != ordered_ids.len() || ordered.len() != app_count {
            return Err("排序清单必须覆盖该客户端的全部供应商且不得重复".to_string());
        }
        let mut queue: VecDeque<ProviderProfile> = ordered.into();
        let mut reordered = Vec::with_capacity(store.profiles.len());
        for profile in store.profiles.iter() {
            if profile.app == app {
                if let Some(moved) = queue.pop_front() {
                    reordered.push(moved);
                }
            } else {
                reordered.push(profile.clone());
            }
        }
        store.profiles = reordered;
        self.save_store(&store)?;
        Ok(store.profiles)
    }

    pub fn import_profile(&self, draft: ProviderDraft) -> Result<ProviderProfile, String> {
        draft.validate().map_err(|error| error.to_string())?;
        let mut store = self.load_store().map_err(|error| error.to_string())?;
        if let Some(existing) = store
            .profiles
            .iter()
            .find(|profile| same_profile(profile, &draft))
        {
            return Ok(existing.clone());
        }
        let profile = ProviderProfile::from_draft(Uuid::new_v4().to_string(), draft);
        store.profiles.push(profile.clone());
        self.save_store(&store)?;
        Ok(profile)
    }

    /// Whether an exactly equal profile already exists (scan-side view of the
    /// import dedup rule).
    pub fn profile_exists(&self, draft: &ProviderDraft) -> bool {
        match self.load_store() {
            Ok(store) => store.profiles.iter().any(|p| same_profile(p, draft)),
            Err(_) => false,
        }
    }

    /// Appends one completed switch or restore to the log and keeps the file
    /// in sync.
    pub fn record_switch(&self, entry: SwitchLog) -> Result<(), String> {
        let mut store = self.load_store().map_err(|error| error.to_string())?;
        store.switch_log.push(entry);
        self.save_store(&store)
    }

    /// The most recent log entry for one client, if the app ever completed a
    /// switch or restore for it.
    pub fn latest_switch(&self, app: AppKind) -> Result<Option<SwitchLog>, ProfileStoreError> {
        Ok(self
            .load_store()?
            .switch_log
            .into_iter()
            .rev()
            .find(|entry| entry.app == app))
    }

    pub fn find_profile(&self, profile_id: &str) -> Result<ProviderProfile, String> {
        self.load_store()
            .map_err(|error| error.to_string())?
            .profiles
            .into_iter()
            .find(|profile| profile.id == profile_id)
            .ok_or_else(|| "供应商不存在".to_string())
    }

    pub fn get_common(&self, app: AppKind) -> Result<CommonConfigPatch, ProfileStoreError> {
        Ok(self
            .load_store()?
            .common
            .into_iter()
            .find(|patch| patch.app == app)
            .unwrap_or(CommonConfigPatch {
                app,
                entries: vec![],
            }))
    }

    pub fn set_common(&self, app: AppKind, patch: CommonConfigPatch) -> Result<(), String> {
        if patch.app != app {
            return Err("通用配置所属客户端不一致".to_string());
        }
        patch.validate().map_err(|error| error.to_string())?;
        let mut store = self.load_store().map_err(|error| error.to_string())?;
        store.common.retain(|current| current.app != app);
        store.common.push(patch);
        self.save_store(&store)
    }

    pub fn get_app_settings(&self) -> Result<AppSettings, String> {
        match fs::read_to_string(self.settings_path()) {
            Ok(text) => match serde_json::from_str::<AppSettings>(&text) {
                Ok(settings) => match settings.validate() {
                    Ok(()) => Ok(settings),
                    Err(_) => Err("应用设置格式无效".to_string()),
                },
                Err(_) => {
                    let settings = migrate_previous_app_settings(&text)?;
                    self.set_app_settings(&settings)
                        .map_err(|_| "应用设置升级失败".to_string())?;
                    Ok(settings)
                }
            },
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

    /// Restores cloud plaintext only through the same strict parser and
    /// one-time usage-query migration as the on-disk profile store.
    pub(crate) fn restore_cloud_backup_store_bytes(
        &self,
        bytes: &[u8],
    ) -> Result<ProfileStore, String> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| "云端备份不是当前支持的供应商数据格式".to_string())?;
        let (store, _) = parse_profile_store(text).map_err(|error| error.to_string())?;
        self.save_store(&store)?;
        Ok(store)
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

    fn store_path(&self) -> PathBuf {
        self.root.join("profiles.json")
    }

    fn save_store(&self, store: &ProfileStore) -> Result<(), String> {
        validate_store_for_write(store)?;
        let content =
            serde_json::to_string_pretty(store).map_err(|_| "供应商存储序列化失败".to_string())?;
        fs::create_dir_all(&self.root).map_err(|_| "无法创建应用数据目录".to_string())?;
        let path = self.store_path();
        let temporary = self.root.join(format!("profiles.{}.tmp", Uuid::new_v4()));
        fs::write(&temporary, content).map_err(|_| "无法写入供应商存储临时文件".to_string())?;
        if fs::rename(&temporary, &path).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err("无法原子保存供应商存储".to_string());
        }
        Ok(())
    }
}

fn migrate_previous_app_settings(text: &str) -> Result<AppSettings, String> {
    let mut fields = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(text)
        .map_err(|_| "应用设置格式无效".to_string())?;
    let previous_fields = [
        "closeBehavior",
        "theme",
        "motion",
        "alwaysOnTop",
        "hardwareAcceleration",
        "interfaceFont",
    ];
    if fields.len() != previous_fields.len()
        || !previous_fields
            .iter()
            .all(|field| fields.contains_key(*field))
    {
        return Err("应用设置格式无效".to_string());
    }

    fields.insert(
        "runtimeLogLevel".to_string(),
        serde_json::Value::String("info".to_string()),
    );
    serde_json::from_value(serde_json::Value::Object(fields))
        .map_err(|_| "应用设置格式无效".to_string())
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
    use asb_core::contracts::UsageQuery;
    use asb_core::{PatchEntry, PatchValue};

    fn codex_draft(name: &str) -> ProviderDraft {
        ProviderDraft {
            app: AppKind::Codex,
            name: name.to_string(),
            model: Some("gpt-5.3-codex".to_string()),
            base_url: Some("https://gateway.example/v1".to_string()),
            api_key: "OPENAI_API_KEY".to_string(),
            model_options: None,
            notes: None,
            website_url: None,
            usage_query: None,
        }
    }

    fn legacy_usage_store(query: &str) -> String {
        r#"{"profiles":[{"id":"p1","app":"codex","name":"旧用量档案","model":null,"baseUrl":"https://relay.example/v1","apiKey":"test-api-key","notes":null,"websiteUrl":null,"usageQuery":__QUERY__}],"common":[],"switchLog":[]}"#
            .replace("__QUERY__", query)
    }

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

        assert_eq!(
            state.load_store().expect("empty store"),
            ProfileStore::default()
        );
        assert!(!state.store_path().exists());
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
    fn legacy_env_key_store_is_rejected_without_rewriting_it() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");
        let legacy = r#"{"profiles":[{"id":"old","app":"codex","name":"旧档案","baseUrl":"https://relay.example/v1","envKey":"OPENAI_API_KEY","model":null,"modelOptions":null}],"common":[],"switchLog":[]}"#;
        fs::write(state.store_path(), legacy).expect("write legacy store");

        let error = state.load_store().expect_err("old schema must fail");
        assert_eq!(error, ProfileStoreError::Unsupported);
        assert_eq!(fs::read_to_string(state.store_path()).unwrap(), legacy);
    }

    #[test]
    fn legacy_claude_one_m_model_strings_are_rejected_without_rewriting_them() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");
        let legacy = r#"{"profiles":[{"id":"old","app":"claude","name":"旧档案","baseUrl":"https://relay.example/v1","apiKey":"<placeholder>","model":"claude-opus-4-1[1m]","modelOptions":{"kind":"claude","primaryOneM":false,"haikuModel":null,"sonnetModel":null,"sonnetOneM":false,"opusModel":null,"opusOneM":false,"availableModels":null}}],"common":[],"switchLog":[]}"#;
        fs::write(state.store_path(), legacy).expect("write legacy store");

        assert_eq!(
            state
                .load_store()
                .expect_err("old Claude model form must fail"),
            ProfileStoreError::Unsupported
        );
        assert_eq!(fs::read_to_string(state.store_path()).unwrap(), legacy);
    }

    #[test]
    fn legacy_claude_model_options_without_context_flags_are_rejected_without_rewriting() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");
        let legacy = r#"{"profiles":[{"id":"old","app":"claude","name":"旧档案","baseUrl":"https://relay.example/v1","apiKey":"<placeholder>","model":"claude-opus-4-1","modelOptions":{"kind":"claude","haikuModel":null,"sonnetModel":"claude-sonnet-4-6","opusModel":null,"availableModels":null}}],"common":[],"switchLog":[]}"#;
        fs::write(state.store_path(), legacy).expect("write legacy store");

        assert_eq!(
            state
                .load_store()
                .expect_err("old Claude option form must fail"),
            ProfileStoreError::Unsupported
        );
        assert_eq!(fs::read_to_string(state.store_path()).unwrap(), legacy);
    }

    #[test]
    fn exact_legacy_declarative_usage_query_migrates_once_and_rewrites_atomically() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");
        let legacy = legacy_usage_store(
            r#"{"url":"{{baseUrl}}/balance","remainingPath":"data/balance","unit":"USD"}"#,
        );
        fs::write(state.store_path(), &legacy).expect("write legacy store");

        let store = state.load_store().expect("migrate store");
        assert!(matches!(
            store.profiles[0].usage_query,
            Some(UsageQuery::Declarative {
                ref remaining_path,
                ..
            }) if remaining_path.as_deref() == Some("data/balance")
        ));
        let migrated = fs::read_to_string(state.store_path()).expect("read migrated store");
        assert!(migrated.contains("\"kind\": \"declarative\""));

        state.load_store().expect("second current read");
        assert_eq!(
            fs::read_to_string(state.store_path()).expect("read stable store"),
            migrated
        );
    }

    #[test]
    fn malformed_legacy_usage_query_is_rejected_without_any_write_back() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");

        for query in [
            r#"{"remainingPath":"data/balance"}"#,
            r#"{"url":"{{baseUrl}}/balance","remainingPath":"data/balance","unknown":true}"#,
            r#"{"url":"{{baseUrl}}/balance","remainingPath":1}"#,
        ] {
            let invalid = legacy_usage_store(query);
            fs::write(state.store_path(), &invalid).expect("write invalid legacy store");
            assert_eq!(
                state
                    .load_store()
                    .expect_err("invalid legacy store must fail"),
                ProfileStoreError::Unsupported
            );
            assert_eq!(
                fs::read_to_string(state.store_path()).expect("read unchanged invalid store"),
                invalid
            );
        }
    }

    #[test]
    fn cloud_restore_uses_the_same_legacy_usage_query_migration() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let legacy = legacy_usage_store(r#"{"url":"{{baseUrl}}/balance","usedPath":"data/used"}"#);

        let store = state
            .restore_cloud_backup_store_bytes(legacy.as_bytes())
            .expect("restore legacy cloud backup");
        assert!(matches!(
            store.profiles[0].usage_query,
            Some(UsageQuery::Declarative {
                ref used_path,
                ..
            }) if used_path.as_deref() == Some("data/used")
        ));
        assert!(fs::read_to_string(state.store_path())
            .expect("read restored store")
            .contains("\"kind\": \"declarative\""));
    }

    #[test]
    fn invalid_usage_script_never_enters_the_profile_store() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let mut draft = codex_draft("坏脚本");
        draft.usage_query = Some(UsageQuery::Script {
            source: "({ request() {} })".to_string(),
        });

        assert!(state.create_profile(draft).is_err());
        assert!(!state.store_path().exists());
    }

    #[test]
    fn reset_profile_store_replaces_unsupported_data_with_current_empty_store() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");
        fs::write(
            state.store_path(),
            br#"{"profiles":[{"envKey":"OLD_KEY"}]}"#,
        )
        .expect("write unsupported store");
        state.reset_profile_store().expect("reset store");

        assert_eq!(
            state.load_store().expect("current store"),
            ProfileStore::default()
        );
        assert!(fs::read_to_string(state.store_path())
            .expect("read reset store")
            .contains("\"profiles\": []"));
    }

    #[test]
    fn incomplete_store_shapes_are_unsupported_not_silently_defaulted() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");

        fs::write(state.store_path(), br#"{"profiles":[],"common":[]}"#)
            .expect("write store without a switch log");
        assert_eq!(
            state.load_store().expect_err("missing switchLog must fail"),
            ProfileStoreError::Unsupported
        );

        fs::write(
            state.store_path(),
            br#"{"profiles":[],"common":[],"switchLog":[{"app":"codex","profileId":null,"profileName":null,"contentHash":"h","backupId":"b","at":"2026-08-28T08:00:00Z"}]}"#,
        )
        .expect("write log entry without an operation");
        assert_eq!(
            state.load_store().expect_err("missing operation must fail"),
            ProfileStoreError::Unsupported
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
            "{\n  \"closeBehavior\": \"exit\",\n  \"theme\": \"dark\",\n  \"motion\": \"reduce\",\n  \"alwaysOnTop\": true,\n  \"hardwareAcceleration\": false,\n  \"interfaceFont\": \"MiSans\",\n  \"runtimeLogLevel\": \"warn\"\n}"
        );
        assert!(!reopened.store_path().exists());
        assert!(!reopened.backup_dir().exists());
    }

    #[test]
    fn previous_complete_app_settings_are_migrated_once_to_the_current_contract() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");
        fs::write(
            state.settings_path(),
            "{\n  \"closeBehavior\": \"exit\",\n  \"theme\": \"dark\",\n  \"motion\": \"reduce\",\n  \"alwaysOnTop\": true,\n  \"hardwareAcceleration\": false,\n  \"interfaceFont\": \"Noto Sans SC\"\n}",
        )
        .expect("write previous settings");

        assert_eq!(
            state.get_app_settings().expect("migrate settings"),
            AppSettings {
                close_behavior: CloseBehavior::Exit,
                theme: ThemePreference::Dark,
                motion: MotionPreference::Reduce,
                always_on_top: true,
                hardware_acceleration: false,
                interface_font: DEFAULT_INTERFACE_FONT.to_string(),
                runtime_log_level: RuntimeLogLevel::Info,
            }
        );
        assert_eq!(
            fs::read_to_string(state.settings_path()).expect("read migrated settings"),
            "{\n  \"closeBehavior\": \"exit\",\n  \"theme\": \"dark\",\n  \"motion\": \"reduce\",\n  \"alwaysOnTop\": true,\n  \"hardwareAcceleration\": false,\n  \"interfaceFont\": \"Noto Sans SC\",\n  \"runtimeLogLevel\": \"info\"\n}"
        );
    }

    #[test]
    fn app_settings_reject_incomplete_unknown_and_invalid_shapes() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        fs::create_dir_all(&state.root).expect("create state directory");

        for invalid in [
            // Previous incomplete settings shape: no compatibility path.
            "{\"closeBehavior\":\"hideToTray\"}",
            "{\"closeBehavior\":\"hideToTray\",\"theme\":\"system\",\"motion\":\"system\",\"alwaysOnTop\":false,\"hardwareAcceleration\":true,\"interfaceFont\":\"Noto Sans SC\",\"runtimeLogLevel\":\"info\",\"legacy\":true}",
            "{\"closeBehavior\":\"hideToTray\",\"theme\":\"sepia\",\"motion\":\"system\",\"alwaysOnTop\":false,\"hardwareAcceleration\":true,\"interfaceFont\":\"Noto Sans SC\",\"runtimeLogLevel\":\"info\"}",
            "{\"closeBehavior\":\"hideToTray\",\"theme\":\"system\",\"motion\":\"system\",\"alwaysOnTop\":false,\"hardwareAcceleration\":\"false\",\"interfaceFont\":\"Noto Sans SC\",\"runtimeLogLevel\":\"info\"}",
            // Two-versions-old complete shape: only one migration step exists.
            "{\"closeBehavior\":\"exit\",\"theme\":\"dark\",\"motion\":\"reduce\",\"alwaysOnTop\":true,\"hardwareAcceleration\":true}",
            "{\"closeBehavior\":\"exit\",\"theme\":\"dark\",\"motion\":\"reduce\",\"alwaysOnTop\":true,\"hardwareAcceleration\":true,\"interfaceFont\":\"Noto Sans SC\",\"runtimeLogLevel\":\"verbose\"}",
            // A font name is consumed verbatim as a quoted CSS value.
            "{\"closeBehavior\":\"exit\",\"theme\":\"dark\",\"motion\":\"reduce\",\"alwaysOnTop\":true,\"hardwareAcceleration\":true,\"interfaceFont\":\"\",\"runtimeLogLevel\":\"info\"}",
            "{\"closeBehavior\":\"exit\",\"theme\":\"dark\",\"motion\":\"reduce\",\"alwaysOnTop\":true,\"hardwareAcceleration\":true,\"interfaceFont\":\"MiSans \",\"runtimeLogLevel\":\"info\"}",
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

    #[test]
    fn profiles_and_common_configuration_persist_without_seed_data() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let created = state
            .create_profile(codex_draft("本机网关"))
            .expect("create profile");
        let updated = state
            .update_profile(&created.id, codex_draft("已更新网关"))
            .expect("update profile");
        state
            .set_common(
                AppKind::Codex,
                CommonConfigPatch {
                    app: AppKind::Codex,
                    entries: vec![PatchEntry {
                        key: "disable_response_storage".to_string(),
                        value: Some(PatchValue::Bool(true)),
                    }],
                },
            )
            .expect("save common");

        let reopened = LocalState::from_root(directory.path().join("state"));
        assert_eq!(
            reopened.list_profiles().expect("list profiles"),
            vec![updated]
        );
        assert_eq!(
            reopened
                .get_common(AppKind::Codex)
                .expect("get common")
                .entries,
            vec![PatchEntry {
                key: "disable_response_storage".to_string(),
                value: Some(PatchValue::Bool(true)),
            }]
        );
    }

    #[test]
    fn import_is_idempotent_and_delete_removes_the_only_profile() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let first = state
            .import_profile(codex_draft("导入网关"))
            .expect("first import");
        let second = state
            .import_profile(codex_draft("导入网关"))
            .expect("second import");

        assert_eq!(first, second);
        state.delete_profile(&first.id).expect("delete profile");
        assert!(state.list_profiles().expect("list profiles").is_empty());
    }

    #[test]
    fn reorder_moves_one_clients_profiles_and_keeps_the_other_in_place() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let codex_a = state
            .create_profile(codex_draft("网关 A"))
            .expect("create codex A");
        let claude = state
            .create_profile(ProviderDraft {
                app: AppKind::Claude,
                name: "Claude 中继".to_string(),
                model: None,
                base_url: Some("https://claude-relay.example/v1".to_string()),
                api_key: "test-api-key".into(),
                model_options: None,
                notes: None,
                website_url: None,
                usage_query: None,
            })
            .expect("create claude");
        let codex_b = state
            .create_profile(codex_draft("网关 B"))
            .expect("create codex B");
        let codex_c = state
            .create_profile(codex_draft("网关 C"))
            .expect("create codex C");

        let reordered = state
            .reorder_profiles(
                AppKind::Codex,
                &[codex_b.id.clone(), codex_a.id.clone(), codex_c.id.clone()],
            )
            .expect("reorder codex");

        let ids: Vec<&str> = reordered.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                codex_b.id.as_str(),
                claude.id.as_str(),
                codex_a.id.as_str(),
                codex_c.id.as_str()
            ]
        );
        assert_eq!(
            LocalState::from_root(directory.path().join("state"))
                .list_profiles()
                .expect("reopen"),
            reordered
        );
    }

    #[test]
    fn reorder_rejects_foreign_missing_and_duplicate_ids() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        let codex_a = state
            .create_profile(codex_draft("网关 A"))
            .expect("create codex A");
        let codex_b = state
            .create_profile(codex_draft("网关 B"))
            .expect("create codex B");
        let claude = state
            .create_profile(ProviderDraft {
                app: AppKind::Claude,
                name: "Claude 中继".to_string(),
                model: None,
                base_url: Some("https://claude-relay.example/v1".to_string()),
                api_key: "test-api-key".into(),
                model_options: None,
                notes: None,
                website_url: None,
                usage_query: None,
            })
            .expect("create claude");

        let foreign = state
            .reorder_profiles(AppKind::Codex, &[claude.id.clone()])
            .expect_err("foreign id must be rejected");
        assert!(foreign.contains("不属于该客户端"));
        let missing = state
            .reorder_profiles(AppKind::Codex, &[codex_a.id.clone()])
            .expect_err("incomplete list must be rejected");
        assert!(missing.contains("一一对应") || missing.contains("不得重复"));
        let duplicate = state
            .reorder_profiles(AppKind::Codex, &[codex_a.id.clone(), codex_a.id.clone()])
            .expect_err("duplicate id must be rejected");
        assert!(duplicate.contains("不得重复"));

        let unchanged = state.list_profiles().expect("list profiles");
        let ids: Vec<&str> = unchanged.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![codex_a.id.as_str(), codex_b.id.as_str(), claude.id.as_str()]
        );
    }

    #[test]
    fn switch_log_records_and_returns_the_latest_entry_per_app() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = LocalState::from_root(directory.path().join("state"));
        state
            .record_switch(SwitchLog {
                app: AppKind::Codex,
                profile_id: Some("p1".into()),
                profile_name: Some("网关 A".into()),
                content_hash: "hash-a".into(),
                backup_id: "b1".into(),
                at: "2026-08-26T08:00:00Z".into(),
                operation: asb_core::SwitchOp::Switch,
            })
            .expect("record first");
        state
            .record_switch(SwitchLog {
                app: AppKind::Codex,
                profile_id: Some("p2".into()),
                profile_name: Some("网关 B".into()),
                content_hash: "hash-b".into(),
                backup_id: "b2".into(),
                at: "2026-08-26T09:00:00Z".into(),
                operation: asb_core::SwitchOp::Switch,
            })
            .expect("record second");
        let latest = state
            .latest_switch(AppKind::Codex)
            .expect("latest switch")
            .expect("present");
        assert_eq!(latest.profile_name.as_deref(), Some("网关 B"));
        assert!(state
            .latest_switch(AppKind::Claude)
            .expect("latest switch")
            .is_none());
    }
}
