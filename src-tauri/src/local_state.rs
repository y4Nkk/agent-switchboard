//! Application-owned local state.
//!
//! Supplier profiles and general overlays live in the app data directory.
//! They start empty and are never synthesized from example data. Codex and
//! Claude Code configuration paths are resolved separately and are not
//! created by this module.

use asb_core::ownership::is_profile_exclusive;
use asb_core::{
    AppKind, ClaudeModelSettings, CodexModelSettings, CommonConfigPatch, ModelOptions, PatchEntry,
    PatchValue, ProviderDraft, ProviderProfile, RouteMode, SwitchLog,
};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStore {
    pub profiles: Vec<ProviderProfile>,
    pub common: Vec<CommonConfigPatch>,
    #[serde(default)]
    pub switch_log: Vec<SwitchLog>,
}

/// Loose on-disk shape used only by the one-time store migration: profiles
/// written before explicit routing modes existed have no `mode`, and general
/// overlays may still carry keys that now belong to profiles.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredStore {
    #[serde(default)]
    profiles: Vec<StoredProfile>,
    #[serde(default)]
    common: Vec<StoredCommon>,
    #[serde(default)]
    switch_log: Vec<SwitchLog>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredProfile {
    id: String,
    app: AppKind,
    #[serde(default)]
    mode: Option<RouteMode>,
    name: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    env_key: Option<String>,
    #[serde(default)]
    model_options: Option<ModelOptions>,
    #[serde(default)]
    credential_ref: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredCommon {
    app: AppKind,
    #[serde(default)]
    entries: Vec<PatchEntry>,
}

/// The deprecated Claude model key, removed from general overlays for good.
const DEPRECATED_COMMON_KEY: &str = "env.ANTHROPIC_SMALL_FAST_MODEL";

fn empty_codex_settings() -> CodexModelSettings {
    CodexModelSettings {
        reasoning_effort: None,
        reasoning_summary: None,
        verbosity: None,
        context_window: None,
    }
}

fn empty_claude_settings() -> ClaudeModelSettings {
    ClaudeModelSettings {
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        available_models: None,
    }
}

fn required_string(entry: &PatchEntry) -> Result<String, String> {
    match &entry.value {
        PatchValue::Str(value) => Ok(value.clone()),
        _ => Err(format!(
            "旧通用配置的 {} 必须是文本，无法安全迁移",
            entry.key
        )),
    }
}

fn required_positive_integer(entry: &PatchEntry) -> Result<u64, String> {
    match entry.value {
        PatchValue::Number(value)
            if value.is_finite()
                && value > 0.0
                && value.fract() == 0.0
                && value <= u64::MAX as f64 =>
        {
            Ok(value as u64)
        }
        _ => Err(format!(
            "旧通用配置的 {} 必须是正整数，无法安全迁移",
            entry.key
        )),
    }
}

fn required_string_list(entry: &PatchEntry) -> Result<Vec<String>, String> {
    match &entry.value {
        PatchValue::Array(items) => items
            .iter()
            .map(|item| match item {
                PatchValue::Str(value) if !value.trim().is_empty() => Ok(value.clone()),
                _ => Err(format!(
                    "旧通用配置的 {} 必须是非空文本列表，无法安全迁移",
                    entry.key
                )),
            })
            .collect(),
        _ => Err(format!(
            "旧通用配置的 {} 必须是文本列表，无法安全迁移",
            entry.key
        )),
    }
}

fn codex_settings(profile: &mut ProviderProfile) -> Result<&mut CodexModelSettings, String> {
    if profile.model_options.is_none() {
        profile.model_options = Some(ModelOptions::Codex(empty_codex_settings()));
    }
    match profile.model_options.as_mut() {
        Some(ModelOptions::Codex(settings)) => Ok(settings),
        Some(ModelOptions::Claude(_)) => {
            Err("旧 Codex 档案的模型选项类型不一致，无法安全迁移".to_string())
        }
        None => unreachable!("Codex model options were initialized"),
    }
}

fn claude_settings(profile: &mut ProviderProfile) -> Result<&mut ClaudeModelSettings, String> {
    if profile.model_options.is_none() {
        profile.model_options = Some(ModelOptions::Claude(empty_claude_settings()));
    }
    match profile.model_options.as_mut() {
        Some(ModelOptions::Claude(settings)) => Ok(settings),
        Some(ModelOptions::Codex(_)) => {
            Err("旧 Claude 档案的模型选项类型不一致，无法安全迁移".to_string())
        }
        None => unreachable!("Claude model options were initialized"),
    }
}

fn migrate_codex_entries(
    profile: &mut ProviderProfile,
    entries: &[PatchEntry],
) -> Result<(), String> {
    for entry in entries {
        match entry.key.as_str() {
            "model" => profile.model = Some(required_string(entry)?),
            "model_provider" => match required_string(entry)?.as_str() {
                "openai" => {
                    profile.mode = RouteMode::Official;
                    profile.base_url = None;
                    profile.env_key = None;
                }
                "asb" if profile.mode == RouteMode::Custom => {}
                "asb" => {
                    return Err(
                        "旧通用配置指定了自定义 Codex 服务，但档案缺少服务地址，无法安全迁移"
                            .to_string(),
                    )
                }
                _ => {
                    return Err(
                        "旧通用配置指定了当前版本无法表示的 Codex 服务，无法安全迁移".to_string(),
                    )
                }
            },
            "model_reasoning_effort" => {
                codex_settings(profile)?.reasoning_effort = Some(required_string(entry)?);
            }
            "model_reasoning_summary" => {
                codex_settings(profile)?.reasoning_summary = Some(required_string(entry)?);
            }
            "model_verbosity" => {
                codex_settings(profile)?.verbosity = Some(required_string(entry)?);
            }
            "model_context_window" => {
                codex_settings(profile)?.context_window = Some(required_positive_integer(entry)?);
            }
            "model_providers.asb"
            | "model_providers.asb.base_url"
            | "model_providers.asb.env_key"
            | "model_providers.asb.wire_api"
            | "model_providers.asb.name" => {
                return Err(
                    "旧通用配置包含 Codex 服务表，无法确认其与哪个档案对应，已停止迁移以保留原数据"
                        .to_string(),
                )
            }
            key if key.starts_with("model_providers.asb.") => {
                return Err(
                    "旧通用配置包含 Codex 服务表，无法确认其与哪个档案对应，已停止迁移以保留原数据"
                        .to_string(),
                )
            }
            _ => return Err(format!("旧 Codex 通用配置包含无法迁移的键 {}", entry.key)),
        }
    }
    Ok(())
}

fn migrate_claude_entries(
    profile: &mut ProviderProfile,
    entries: &[PatchEntry],
) -> Result<(), String> {
    let mut top_level_model = None;
    let mut environment_model = None;

    for entry in entries {
        match entry.key.as_str() {
            "model" => top_level_model = Some(required_string(entry)?),
            "env.ANTHROPIC_MODEL" => environment_model = Some(required_string(entry)?),
            "env.ANTHROPIC_BASE_URL" => {
                profile.mode = RouteMode::Custom;
                profile.base_url = Some(required_string(entry)?);
                profile.env_key = None;
            }
            "env.ANTHROPIC_DEFAULT_HAIKU_MODEL" | DEPRECATED_COMMON_KEY => {
                claude_settings(profile)?.haiku_model = Some(required_string(entry)?);
            }
            "env.ANTHROPIC_DEFAULT_SONNET_MODEL" => {
                claude_settings(profile)?.sonnet_model = Some(required_string(entry)?);
            }
            "env.ANTHROPIC_DEFAULT_OPUS_MODEL" => {
                claude_settings(profile)?.opus_model = Some(required_string(entry)?);
            }
            "availableModels" => {
                claude_settings(profile)?.available_models = Some(required_string_list(entry)?);
            }
            _ => return Err(format!("旧 Claude 通用配置包含无法迁移的键 {}", entry.key)),
        }
    }

    // Claude Code gives env.ANTHROPIC_MODEL precedence over model. Preserve
    // the effective route while moving it to the profile-owned primary model.
    if let Some(model) = environment_model.or(top_level_model) {
        profile.model = Some(model);
    }
    Ok(())
}

fn validate_store(store: &ProfileStore) -> Result<(), String> {
    let mut profile_ids = HashSet::new();
    for profile in &store.profiles {
        if !profile_ids.insert(&profile.id) {
            return Err("供应商存储包含重复档案标识".to_string());
        }
        profile
            .validate()
            .map_err(|error| format!("供应商存储包含无效档案：{error}"))?;
    }

    let mut common_apps = HashSet::new();
    for patch in &store.common {
        if !common_apps.insert(patch.app) {
            return Err("供应商存储包含同一客户端的重复通用配置".to_string());
        }
        patch
            .validate()
            .map_err(|error| format!("供应商存储包含无效通用配置：{error}"))?;
    }
    Ok(())
}

fn migrate_store(stored: StoredStore) -> Result<(ProfileStore, bool), String> {
    let mut changed = false;

    let mut profiles = Vec::with_capacity(stored.profiles.len());
    for profile in stored.profiles {
        if profile
            .credential_ref
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Err(
                "旧供应商档案包含已弃用的凭据引用，应用拒绝自动迁移以避免丢失数据".to_string(),
            );
        }
        let (mode, env_key) = match profile.mode {
            Some(mode) => (mode, profile.env_key),
            None => {
                changed = true;
                // Before explicit modes, an empty address meant the official
                // endpoint; anything else was custom.
                match profile.base_url.is_some() {
                    true => (RouteMode::Custom, profile.env_key),
                    false => (RouteMode::Official, None),
                }
            }
        };
        profiles.push(ProviderProfile {
            id: profile.id,
            app: profile.app,
            mode,
            name: profile.name,
            model: profile.model,
            base_url: profile.base_url,
            env_key,
            model_options: profile.model_options,
        });
    }

    let mut common = Vec::with_capacity(stored.common.len());
    for patch in stored.common {
        let (profile_entries, entries): (Vec<_>, Vec<_>) =
            patch.entries.into_iter().partition(|entry| {
                entry.key == DEPRECATED_COMMON_KEY || is_profile_exclusive(&entry.key)
            });

        if !profile_entries.is_empty() {
            let mut matching_profiles = profiles
                .iter_mut()
                .filter(|profile| profile.app == patch.app);
            let Some(first) = matching_profiles.next() else {
                return Err(
                    "旧通用配置包含档案级设置，但没有对应供应商档案，已停止迁移以保留原数据"
                        .to_string(),
                );
            };
            match patch.app {
                AppKind::Codex => migrate_codex_entries(first, &profile_entries)?,
                AppKind::Claude => migrate_claude_entries(first, &profile_entries)?,
            }
            for profile in matching_profiles {
                match patch.app {
                    AppKind::Codex => migrate_codex_entries(profile, &profile_entries)?,
                    AppKind::Claude => migrate_claude_entries(profile, &profile_entries)?,
                }
            }
            changed = true;
        }

        let migrated_patch = CommonConfigPatch {
            app: patch.app,
            entries,
        };
        migrated_patch
            .validate()
            .map_err(|error| format!("旧通用配置无法迁移：{error}"))?;
        common.push(migrated_patch);
    }

    let store = ProfileStore {
        profiles,
        common,
        switch_log: stored.switch_log,
    };
    validate_store(&store)?;
    Ok((store, changed))
}

pub struct LocalState {
    root: PathBuf,
}

impl LocalState {
    pub fn from_app(app: &tauri::AppHandle) -> Result<Self, String> {
        use tauri::Manager;

        let root = app
            .path()
            .app_data_dir()
            .map_err(|_| "无法定位应用数据目录".to_string())?
            .join("state");
        Ok(Self { root })
    }

    #[cfg(test)]
    fn from_root(root: PathBuf) -> Self {
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

    pub fn load_store(&self) -> Result<ProfileStore, String> {
        let path = self.store_path();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ProfileStore::default());
            }
            Err(_) => return Err("供应商存储不可读".to_string()),
        };
        let stored: StoredStore =
            serde_json::from_str(&text).map_err(|_| "供应商存储格式无效".to_string())?;
        let (store, changed) = migrate_store(stored)?;
        // The migration is one-way: the migrated shape is written back the
        // first time it is seen so the old keys never return.
        if changed {
            self.save_store(&store)?;
        }
        Ok(store)
    }

    pub fn list_profiles(&self) -> Result<Vec<ProviderProfile>, String> {
        Ok(self.load_store()?.profiles)
    }

    pub fn create_profile(&self, draft: ProviderDraft) -> Result<ProviderProfile, String> {
        draft.validate().map_err(|error| error.to_string())?;
        let mut store = self.load_store()?;
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
        let mut store = self.load_store()?;
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
        let mut store = self.load_store()?;
        let original_len = store.profiles.len();
        store.profiles.retain(|profile| profile.id != profile_id);
        if store.profiles.len() == original_len {
            return Err("供应商不存在".to_string());
        }
        self.save_store(&store)
    }

    pub fn import_profile(&self, draft: ProviderDraft) -> Result<ProviderProfile, String> {
        draft.validate().map_err(|error| error.to_string())?;
        let mut store = self.load_store()?;
        if let Some(existing) = store.profiles.iter().find(|profile| {
            profile.app == draft.app
                && profile.mode == draft.mode
                && profile.name == draft.name
                && profile.model == draft.model
                && profile.base_url == draft.base_url
                && profile.env_key == draft.env_key
                && profile.model_options == draft.model_options
        }) {
            return Ok(existing.clone());
        }
        let profile = ProviderProfile::from_draft(Uuid::new_v4().to_string(), draft);
        store.profiles.push(profile.clone());
        self.save_store(&store)?;
        Ok(profile)
    }

    /// Appends one completed switch or restore to the log and keeps the file
    /// in sync.
    pub fn record_switch(&self, entry: SwitchLog) -> Result<(), String> {
        let mut store = self.load_store()?;
        store.switch_log.push(entry);
        self.save_store(&store)
    }

    /// The most recent log entry for one client, if the app ever completed a
    /// switch or restore for it.
    pub fn latest_switch(&self, app: AppKind) -> Result<Option<SwitchLog>, String> {
        Ok(self
            .load_store()?
            .switch_log
            .into_iter()
            .rev()
            .find(|entry| entry.app == app))
    }

    pub fn find_profile(&self, profile_id: &str) -> Result<ProviderProfile, String> {
        self.load_store()?
            .profiles
            .into_iter()
            .find(|profile| profile.id == profile_id)
            .ok_or_else(|| "供应商不存在".to_string())
    }

    pub fn get_common(&self, app: AppKind) -> Result<CommonConfigPatch, String> {
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
        let mut store = self.load_store()?;
        store.common.retain(|current| current.app != app);
        store.common.push(patch);
        self.save_store(&store)
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

fn target_in_home(home: &Path, app: AppKind) -> PathBuf {
    match app {
        AppKind::Codex => home.join(".codex").join("config.toml"),
        AppKind::Claude => home.join(".claude").join("settings.json"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::PatchValue;

    fn codex_draft(name: &str) -> ProviderDraft {
        ProviderDraft {
            app: AppKind::Codex,
            mode: RouteMode::Custom,
            name: name.to_string(),
            model: Some("gpt-5.3-codex".to_string()),
            base_url: Some("https://gateway.example/v1".to_string()),
            env_key: Some("OPENAI_API_KEY".to_string()),
            model_options: None,
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
                        value: PatchValue::Bool(true),
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
                value: PatchValue::Bool(true),
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

    #[test]
    fn legacy_store_moves_profile_settings_without_losing_effective_values() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("state");
        fs::create_dir_all(&root).expect("create state dir");
        // Store shape from before explicit routing modes: no `mode`, a
        // deprecated Claude model key, and general keys that now belong to
        // profiles.
        let legacy = r#"{
  "profiles": [
    { "id": "p1", "app": "codex", "name": "旧网关", "model": "gpt-5", "baseUrl": "https://old.internal/v1", "envKey": "OLD_KEY" },
    { "id": "p3", "app": "codex", "name": "第二网关", "model": "gpt-4", "baseUrl": "https://second.internal/v1", "envKey": "SECOND_KEY" },
    { "id": "p2", "app": "claude", "name": "旧 Claude", "model": "claude-sonnet-4", "baseUrl": null, "envKey": null }
  ],
  "common": [
    { "app": "claude", "entries": [
      { "key": "env.ANTHROPIC_SMALL_FAST_MODEL", "value": "claude-3-5-haiku-latest" },
      { "key": "model", "value": "claude-sonnet-4" },
      { "key": "env.ANTHROPIC_MODEL", "value": "claude-opus-4" },
      { "key": "env.ANTHROPIC_DEFAULT_SONNET_MODEL", "value": "claude-sonnet-4" },
      { "key": "availableModels", "value": ["claude-haiku-4", "claude-opus-4"] }
    ] },
    { "app": "codex", "entries": [
      { "key": "model", "value": "gpt-5.4" },
      { "key": "model_reasoning_effort", "value": "high" },
      { "key": "model_reasoning_summary", "value": "concise" },
      { "key": "model_verbosity", "value": "low" },
      { "key": "model_context_window", "value": 272000 },
      { "key": "disable_response_storage", "value": true }
    ] }
  ]
}"#;
        fs::write(root.join("profiles.json"), legacy).expect("write legacy store");

        let state = LocalState::from_root(root.clone());
        let store = state.load_store().expect("migrated store");

        let codex = store
            .profiles
            .iter()
            .find(|p| p.id == "p1")
            .expect("codex profile");
        assert_eq!(codex.mode, RouteMode::Custom);
        assert_eq!(codex.model.as_deref(), Some("gpt-5.4"));
        assert_eq!(
            codex.model_options,
            Some(ModelOptions::Codex(CodexModelSettings {
                reasoning_effort: Some("high".to_string()),
                reasoning_summary: Some("concise".to_string()),
                verbosity: Some("low".to_string()),
                context_window: Some(272_000),
            }))
        );
        let second_codex = store
            .profiles
            .iter()
            .find(|p| p.id == "p3")
            .expect("second codex profile");
        assert_eq!(second_codex.model, codex.model);
        assert_eq!(second_codex.model_options, codex.model_options);
        let claude = store
            .profiles
            .iter()
            .find(|p| p.id == "p2")
            .expect("claude profile");
        assert_eq!(claude.mode, RouteMode::Official);
        // ANTHROPIC_MODEL was the actual old runtime override, so it wins
        // over the old top-level `model` during migration.
        assert_eq!(claude.model.as_deref(), Some("claude-opus-4"));
        assert_eq!(
            claude.model_options,
            Some(ModelOptions::Claude(ClaudeModelSettings {
                haiku_model: Some("claude-3-5-haiku-latest".to_string()),
                sonnet_model: Some("claude-sonnet-4".to_string()),
                opus_model: None,
                available_models: Some(vec![
                    "claude-haiku-4".to_string(),
                    "claude-opus-4".to_string(),
                ]),
            }))
        );

        // Model keys no longer belong in general overlays; their old values
        // now live in every corresponding supplier profile.
        let claude_common = store
            .common
            .iter()
            .find(|c| c.app == AppKind::Claude)
            .expect("claude common");
        assert!(claude_common.entries.is_empty());
        let codex_common = store
            .common
            .iter()
            .find(|c| c.app == AppKind::Codex)
            .expect("codex common");
        assert_eq!(
            codex_common.entries,
            vec![PatchEntry {
                key: "disable_response_storage".to_string(),
                value: PatchValue::Bool(true),
            }]
        );

        // The migrated shape was written back; a second load changes nothing.
        let rewritten = fs::read_to_string(root.join("profiles.json")).expect("read back");
        assert!(rewritten.contains("\"mode\": \"custom\""));
        assert!(!rewritten.contains("ANTHROPIC_SMALL_FAST_MODEL"));
        let again = state.load_store().expect("second load");
        assert_eq!(again, store);
    }

    #[test]
    fn legacy_credential_reference_stops_migration_without_rewriting_the_store() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("state");
        fs::create_dir_all(&root).expect("create state dir");
        let legacy = r#"{
  "profiles": [
    {
      "id": "p1",
      "app": "codex",
      "name": "旧网关",
      "baseUrl": "https://old.internal/v1",
      "envKey": "OLD_KEY",
      "credentialRef": "legacy-reference"
    }
  ]
}"#;
        let path = root.join("profiles.json");
        fs::write(&path, legacy).expect("write legacy store");

        let state = LocalState::from_root(root);
        let error = state
            .load_store()
            .expect_err("must not strip credential reference");
        assert!(error.contains("凭据引用"));
        assert!(fs::read_to_string(path)
            .expect("read original store")
            .contains("credentialRef"));
    }
}
