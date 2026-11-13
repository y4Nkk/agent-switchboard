//! Whole-configuration snapshot: the unit encrypted for cloud backup and
//! verified before it is enabled. The snapshot is data only — enabling it is
//! a staging write plus a directory swap owned by this module.

pub(crate) mod activation;
mod decode;


pub use activation::enable_snapshot;
pub(crate) use decode::decode_cloud_backup_snapshot;

use super::{codex_providers, history, providers, ConfigStore, ProfileStoreError};
use asb_core::contracts::{
    AppKind, CodexProviderFile, ConfigWriteRecord, ProviderFile, RouteMode, SettingsValues,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The only plaintext configuration snapshot format written to cloud backup.
/// The AES-GCM envelope has its own version and is intentionally independent.
pub const CLOUD_BACKUP_SNAPSHOT_SCHEMA_VERSION: u8 = 8;

/// The complete persisted configuration of both clients. Provider files are
/// grouped by the directory that owns their client association; secrets stay
/// inside the provider files exactly as on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigurationSnapshot {
    pub schema_version: u8,
    pub codex_providers: Vec<CodexProviderFile>,
    /// The Codex official-login records of the generic store, staged into
    /// `providers/codex/official`. Claude official records stay inside
    /// `claude_providers`.
    pub codex_official: Vec<ProviderFile>,
    pub claude_providers: Vec<ProviderFile>,
    pub client_settings: BTreeMap<AppKind, SettingsValues>,
    pub history: BTreeMap<AppKind, Vec<ConfigWriteRecord>>,
}

impl ConfigurationSnapshot {
    pub fn provider_count(&self) -> usize {
        self.codex_providers.len() + self.codex_official.len() + self.claude_providers.len()
    }
}

/// Reads the current on-disk configuration as a snapshot. Every file passes
/// the same strict validation a runtime read would apply.
pub fn read_configuration_snapshot(
    store: &ConfigStore,
) -> Result<ConfigurationSnapshot, ProfileStoreError> {
    let mut client_settings = BTreeMap::new();
    let mut history_map = BTreeMap::new();
    for app in [AppKind::Codex, AppKind::Claude] {
        client_settings.insert(app, store.get_client_settings(app)?.settings);
        history_map.insert(app, store.load_history(app)?);
    }
    Ok(ConfigurationSnapshot {
        schema_version: CLOUD_BACKUP_SNAPSHOT_SCHEMA_VERSION,
        codex_providers: codex_providers::load_codex_provider_files(store)?,
        codex_official: providers::load_provider_files(store, AppKind::Codex)?,
        claude_providers: providers::load_provider_files(store, AppKind::Claude)?,
        client_settings,
        history: history_map,
    })
}

/// Validates a snapshot without touching the filesystem: UUID identifiers,
/// provider contracts, client settings completeness, and history records.
pub fn validate_snapshot(snapshot: &ConfigurationSnapshot) -> Result<(), String> {
    if snapshot.schema_version != CLOUD_BACKUP_SNAPSHOT_SCHEMA_VERSION {
        return Err("云端备份配置快照版本不受支持".to_string());
    }
    let mut seen_ids = std::collections::HashSet::new();
    for app in [AppKind::Codex, AppKind::Claude] {
        let settings = snapshot
            .client_settings
            .get(&app)
            .ok_or_else(|| format!("快照缺少 {app:?} 客户端设置"))?;
        settings
            .validate_client_settings(app)
            .map_err(|error| error.to_string())?;
        for record in snapshot
            .history
            .get(&app)
            .ok_or_else(|| format!("快照缺少 {app:?} 写入历史"))?
        {
            history::validate_write_record(app, record)?;
        }
    }
    validate_codex_files(&snapshot.codex_providers, &mut seen_ids)?;
    validate_codex_official_files(&snapshot.codex_official, &mut seen_ids)?;
    validate_claude_files(&snapshot.claude_providers, &mut seen_ids)?;
    validate_claude_references(&snapshot.claude_providers)?;
    Ok(())
}

fn validate_claude_references(files: &[ProviderFile]) -> Result<(), String> {
    use asb_core::contracts::{ClaudeModelRole, ClaudeModelRoute};
    let profiles = files.iter().cloned().map(|file| file.into_profile(AppKind::Claude)).collect::<Vec<_>>();
    for owner in &profiles {
        for role in ClaudeModelRole::ALL {
            if let Some(ClaudeModelRoute::Provider { profile_id, model, .. }) = crate::gateway::claude_targets::settings(owner).and_then(|s| s.route(role)) {
                let target = profiles.iter().find(|profile| &profile.id == profile_id)
                    .ok_or_else(|| format!("{} 的 {} 路由目标不存在：{profile_id}", owner.name, role.name()))?;
                crate::gateway::claude_targets::validate_target(target, model)?;
            }
        }
    }
    Ok(())
}

fn validate_codex_files(
    files: &[CodexProviderFile],
    seen_ids: &mut std::collections::HashSet<String>,
) -> Result<(), String> {
    let mut previous_position = 0;
    for file in files {
        file.validate()?;
        let id = &file.profile.id;
        if uuid::Uuid::parse_str(id).is_err() || !seen_ids.insert(id.clone()) {
            return Err(format!("供应商标识无效或重复：{id}"));
        }
        if file.position <= previous_position {
            return Err(format!("供应商排序位置无效：{}", file.profile.name));
        }
        previous_position = file.position;
    }
    Ok(())
}

/// The Codex official-login directory holds at most one record, and it must
/// actually be an official-login record.
fn validate_codex_official_files(
    files: &[ProviderFile],
    seen_ids: &mut std::collections::HashSet<String>,
) -> Result<(), String> {
    for file in files {
        if uuid::Uuid::parse_str(&file.id).is_err() || !seen_ids.insert(file.id.clone()) {
            return Err(format!("供应商标识无效或重复：{}", file.id));
        }
        let profile = file.clone().into_profile(AppKind::Codex);
        if profile.route_mode != RouteMode::Official {
            return Err("Codex 官方登录目录只允许官方登录档案".to_string());
        }
        profile.validate().map_err(|error| error.to_string())?;
    }
    if files.len() > 1 {
        return Err("Codex 存在重复的官方登录入口".to_string());
    }
    Ok(())
}

fn validate_claude_files(
    files: &[ProviderFile],
    seen_ids: &mut std::collections::HashSet<String>,
) -> Result<(), String> {
    let mut previous_position = 0;
    let mut has_official_route = false;
    for file in files {
        if uuid::Uuid::parse_str(&file.id).is_err() || !seen_ids.insert(file.id.clone()) {
            return Err(format!("供应商标识无效或重复：{}", file.id));
        }
        if file.position <= previous_position {
            return Err(format!("供应商排序位置无效：{}", file.name));
        }
        previous_position = file.position;
        file.clone()
            .into_profile(AppKind::Claude)
            .validate()
            .map_err(|error| error.to_string())?;
        if file.route_mode == RouteMode::Official {
            if has_official_route {
                return Err("Claude 存在重复的官方登录入口".to_string());
            }
            has_official_route = true;
        }
    }
    Ok(())
}
