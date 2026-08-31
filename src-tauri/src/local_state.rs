//! Application-owned local state.
//!
//! Supplier profiles and general overlays live in the app data directory.
//! They start empty and are never synthesized from example data. Codex and
//! Claude Code configuration paths are resolved separately and are not
//! created by this module.

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

/// Application-owned desktop preferences, stored separately from profile data.
/// This is a strict complete contract. The only migration is the exact
/// immediately previous complete shape, which is atomically rewritten with
/// hardware acceleration enabled to preserve the prior runtime behavior.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSettings {
    pub close_behavior: CloseBehavior,
    pub theme: ThemePreference,
    pub motion: MotionPreference,
    pub always_on_top: bool,
    pub hardware_acceleration: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            close_behavior: CloseBehavior::HideToTray,
            theme: ThemePreference::System,
            motion: MotionPreference::System,
            always_on_top: false,
            hardware_acceleration: true,
        }
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
    for profile in &store.profiles {
        profile
            .validate()
            .map_err(|_| ProfileStoreError::Unsupported)?;
    }
    for patch in &store.common {
        patch
            .validate()
            .map_err(|_| ProfileStoreError::Unsupported)?;
    }
    Ok(())
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

    pub fn user_config_path(app: AppKind) -> Result<PathBuf, String> {
        let home = std::env::var_os("USERPROFILE")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "无法确定 Windows 用户目录".to_string())?;
        Ok(target_in_home(Path::new(&home), app))
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.root.join("backups")
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
        let store: ProfileStore =
            serde_json::from_str(&text).map_err(|_| ProfileStoreError::Unsupported)?;
        validate_store(&store)?;
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
            Ok(text) => match serde_json::from_str(&text) {
                Ok(settings) => Ok(settings),
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

    fn settings_path(&self) -> PathBuf {
        self.root.join("settings.json")
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
    let previous_fields = ["closeBehavior", "theme", "motion", "alwaysOnTop"];
    if fields.len() != previous_fields.len()
        || !previous_fields
            .iter()
            .all(|field| fields.contains_key(*field))
    {
        return Err("应用设置格式无效".to_string());
    }

    fields.insert(
        "hardwareAcceleration".to_string(),
        serde_json::Value::Bool(true),
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

#[cfg(test)]
mod tests {
    use super::*;
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
            "{\n  \"closeBehavior\": \"exit\",\n  \"theme\": \"dark\",\n  \"motion\": \"reduce\",\n  \"alwaysOnTop\": true,\n  \"hardwareAcceleration\": false\n}"
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
            "{\n  \"closeBehavior\": \"exit\",\n  \"theme\": \"dark\",\n  \"motion\": \"reduce\",\n  \"alwaysOnTop\": true\n}",
        )
        .expect("write previous settings");

        assert_eq!(
            state.get_app_settings().expect("migrate settings"),
            AppSettings {
                close_behavior: CloseBehavior::Exit,
                theme: ThemePreference::Dark,
                motion: MotionPreference::Reduce,
                always_on_top: true,
                hardware_acceleration: true,
            }
        );
        assert_eq!(
            fs::read_to_string(state.settings_path()).expect("read migrated settings"),
            "{\n  \"closeBehavior\": \"exit\",\n  \"theme\": \"dark\",\n  \"motion\": \"reduce\",\n  \"alwaysOnTop\": true,\n  \"hardwareAcceleration\": true\n}"
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
            "{\"closeBehavior\":\"hideToTray\",\"theme\":\"system\",\"motion\":\"system\",\"alwaysOnTop\":false,\"legacy\":true}",
            "{\"closeBehavior\":\"hideToTray\",\"theme\":\"sepia\",\"motion\":\"system\",\"alwaysOnTop\":false}",
            "{\"closeBehavior\":\"hideToTray\",\"theme\":\"system\",\"motion\":\"system\",\"alwaysOnTop\":false,\"hardwareAcceleration\":\"false\"}",
        ] {
            fs::write(state.settings_path(), invalid).expect("write invalid settings");
            assert_eq!(state.get_app_settings().unwrap_err(), "应用设置格式无效");
        }
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
