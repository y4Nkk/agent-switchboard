//! Whole-configuration snapshot: the unit encrypted for cloud backup and the
//! shape verified before the one-time legacy migration enables the new
//! layout. The snapshot is data only — enabling it is a staging write plus a
//! directory swap owned by this module.

use super::{history, providers, write_json_atomic, ConfigStore, ProfileStoreError};
use asb_core::contracts::{AppKind, CommonSettings, ConfigWriteRecord, ProviderFile, RouteMode};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// The complete persisted configuration of both clients. Provider files are
/// grouped by the directory that owns their client association; secrets stay
/// inside the provider files exactly as on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigurationSnapshot {
    pub providers: BTreeMap<AppKind, Vec<ProviderFile>>,
    pub common: BTreeMap<AppKind, CommonSettings>,
    pub history: BTreeMap<AppKind, Vec<ConfigWriteRecord>>,
}

impl ConfigurationSnapshot {
    pub fn provider_count(&self) -> usize {
        self.providers.values().map(Vec::len).sum()
    }
}

/// Reads the current on-disk configuration as a snapshot. Every file passes
/// the same strict validation a runtime read would apply.
pub fn read_configuration_snapshot(
    store: &ConfigStore,
) -> Result<ConfigurationSnapshot, ProfileStoreError> {
    let mut providers = BTreeMap::new();
    let mut common = BTreeMap::new();
    let mut history_map = BTreeMap::new();
    for app in [AppKind::Codex, AppKind::Claude] {
        providers.insert(app, providers::load_provider_files(store, app)?);
        common.insert(app, store.get_common_settings(app)?.settings);
        history_map.insert(app, store.load_history(app)?);
    }
    Ok(ConfigurationSnapshot {
        providers,
        common,
        history: history_map,
    })
}

/// Validates a snapshot without touching the filesystem: UUID identifiers,
/// provider contracts, common-settings completeness, and history records.
pub fn validate_snapshot(snapshot: &ConfigurationSnapshot) -> Result<(), String> {
    let mut seen_ids = std::collections::HashSet::new();
    for app in [AppKind::Codex, AppKind::Claude] {
        let files = snapshot
            .providers
            .get(&app)
            .ok_or_else(|| format!("快照缺少 {app:?} 供应商分组"))?;
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
                .into_profile(app)
                .validate()
                .map_err(|error| error.to_string())?;
            if file.route_mode == RouteMode::Official {
                if has_official_route {
                    return Err(format!("{app:?} 存在重复的官方登录入口"));
                }
                has_official_route = true;
            }
        }
        let settings = snapshot
            .common
            .get(&app)
            .ok_or_else(|| format!("快照缺少 {app:?} 通用设置"))?;
        settings
            .validate_for(app)
            .map_err(|error| error.to_string())?;
        for record in snapshot
            .history
            .get(&app)
            .ok_or_else(|| format!("快照缺少 {app:?} 写入历史"))?
        {
            history::validate_write_record(app, record)?;
        }
    }
    Ok(())
}

fn snapshot_history_file(records: &[ConfigWriteRecord]) -> Result<String, String> {
    serde_json::to_string_pretty(&history::HistoryFile {
        records: records.to_vec(),
    })
    .map_err(|_| "写入历史序列化失败".to_string())
}

/// Writes a validated snapshot into `target`, which must not exist yet, then
/// reads every file back and checks the result equals the snapshot.
fn stage_and_verify(target: &Path, snapshot: &ConfigurationSnapshot) -> Result<(), String> {
    validate_snapshot(snapshot)?;
    for app in [AppKind::Codex, AppKind::Claude] {
        for file in &snapshot.providers[&app] {
            let json = serde_json::to_string_pretty(file)
                .map_err(|_| "供应商文件序列化失败".to_string())?;
            write_json_atomic(
                &target
                    .join("providers")
                    .join(app.dir_name())
                    .join(format!("{}.json", file.id)),
                &json,
            )?;
        }
        let common_json = serde_json::to_string_pretty(&snapshot.common[&app])
            .map_err(|_| "通用设置序列化失败".to_string())?;
        write_json_atomic(
            &target
                .join("common")
                .join(format!("{}.json", app.dir_name())),
            &common_json,
        )?;
        let history_json = snapshot_history_file(&snapshot.history[&app])?;
        write_json_atomic(
            &target
                .join("history")
                .join(format!("{}.json", app.dir_name())),
            &history_json,
        )?;
    }

    // Read back through the strict runtime readers and compare. The staged
    // directory is itself a configuration directory, so its parent is the
    // probe's state root.
    let probe = ConfigStore::new(
        target
            .parent()
            .ok_or_else(|| "无效的暂存路径".to_string())?
            .to_path_buf(),
    );
    let read_back = read_configuration_snapshot(&probe).map_err(|error| error.to_string())?;
    if read_back != *snapshot {
        return Err("暂存配置回读校验不一致".to_string());
    }
    Ok(())
}

enum Activation {
    Enabled { retired: Option<PathBuf> },
    Restored(String),
    RecoveryRequired(String),
}

/// Activates a staged directory without deleting either side before the live
/// directory has been restored or replaced. The injectable rename operation
/// keeps the failure boundary testable without a second filesystem contract.
fn activate_staged<F>(live: &Path, staged: &Path, state_root: &Path, rename: &mut F) -> Activation
where
    F: FnMut(&Path, &Path) -> std::io::Result<()>,
{
    if !live.exists() {
        return match rename(staged, live) {
            Ok(()) => Activation::Enabled { retired: None },
            Err(error) => Activation::Restored(format!("无法启用新配置目录：{error}")),
        };
    }

    let retired = state_root.join(format!("retired-{}", Uuid::new_v4().simple()));
    if let Err(error) = rename(live, &retired) {
        return Activation::Restored(format!("无法移出旧配置目录：{error}"));
    }
    match rename(staged, live) {
        Ok(()) => Activation::Enabled {
            retired: Some(retired),
        },
        Err(error) => match rename(&retired, live) {
            Ok(()) => Activation::Restored(format!(
                "无法启用新配置目录：{error}；已恢复原配置目录"
            )),
            Err(restore_error) => Activation::RecoveryRequired(format!(
                "无法启用新配置目录：{error}；自动恢复也失败：{restore_error}。原配置保留在 {}，新暂存配置保留在 {}",
                retired.display(),
                staged.display()
            )),
        },
    }
}

/// Replaces the live `configuration` directory with a validated snapshot.
/// The snapshot is written into a private staging root, read back through
/// the strict runtime readers, and only then swapped into place. A failed
/// swap restores the old directory; an unrecoverable filesystem failure keeps
/// both directories in place and returns their exact recovery paths.
pub fn enable_snapshot(
    store: &ConfigStore,
    snapshot: &ConfigurationSnapshot,
) -> Result<(), String> {
    let staging_root = store
        .state_root
        .join(format!("staging-{}", Uuid::new_v4().simple()));
    let staged = staging_root.join("configuration");
    let mut preserve_staging = false;
    let result = stage_and_verify(&staged, snapshot).and_then(|()| {
        let live = store.configuration_dir();
        let mut rename = |from: &Path, to: &Path| fs::rename(from, to);
        match activate_staged(&live, &staged, &store.state_root, &mut rename) {
            Activation::Enabled { retired } => {
                if let Some(retired) = retired {
                    let _ = fs::remove_dir_all(retired);
                }
                Ok(())
            }
            Activation::Restored(error) => Err(error),
            Activation::RecoveryRequired(error) => {
                preserve_staging = true;
                Err(error)
            }
        }
    });
    if !preserve_staging {
        let _ = fs::remove_dir_all(&staging_root);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::{CommonSettingValue, ConfigValue, ProviderDraft};

    fn draft(app: AppKind, name: &str) -> ProviderDraft {
        ProviderDraft {
            app,
            route_mode: RouteMode::Custom,
            name: name.to_string(),
            model: None,
            base_url: Some("https://relay.example".to_string()),
            api_key: "test-api-key".to_string(),
            model_options: None,
            notes: None,
            website_url: None,
            usage_query: None,
        }
    }

    fn live_snapshot() -> (tempfile::TempDir, ConfigStore, ConfigurationSnapshot) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));
        store
            .create_provider(draft(AppKind::Codex, "网关"))
            .expect("provider");
        let snapshot = read_configuration_snapshot(&store).expect("snapshot");
        (directory, store, snapshot)
    }

    #[test]
    fn snapshot_round_trips_through_enable() {
        let (directory, store, mut snapshot) = live_snapshot();
        snapshot.common.insert(AppKind::Codex, {
            let mut settings = snapshot.common[&AppKind::Codex].clone();
            settings.settings.insert(
                "model_reasoning_effort".into(),
                CommonSettingValue::Explicit {
                    value: ConfigValue::Str("high".into()),
                },
            );
            settings
        });

        enable_snapshot(&store, &snapshot).expect("enable");
        let read_back = read_configuration_snapshot(&store).expect("read back");
        assert_eq!(read_back, snapshot);
        assert_eq!(read_back.provider_count(), 1);
        assert!(!directory
            .path()
            .join("state")
            .join("settings.json")
            .exists());
    }

    #[test]
    fn an_invalid_snapshot_is_rejected_without_touching_the_live_layout() {
        let (_directory, store, mut snapshot) = live_snapshot();
        snapshot
            .providers
            .get_mut(&AppKind::Codex)
            .expect("codex group")[0]
            .id = "not-a-uuid".into();

        assert!(enable_snapshot(&store, &snapshot).is_err());
        let read_back = read_configuration_snapshot(&store).expect("live layout intact");
        assert_eq!(read_back.provider_count(), 1);
        assert!(read_back.providers[&AppKind::Codex][0].id != "not-a-uuid");
    }

    #[test]
    fn history_positions_must_stay_ordered() {
        let (_directory, store, mut snapshot) = live_snapshot();
        let mut file = snapshot.providers[&AppKind::Codex][0].clone();
        file.id = uuid::Uuid::new_v4().to_string();
        file.name = "第二网关".into();
        file.position = 50; // lower than the first provider's 100
        snapshot
            .providers
            .get_mut(&AppKind::Codex)
            .expect("codex group")
            .push(file);
        assert!(enable_snapshot(&store, &snapshot).is_err());
    }

    #[test]
    fn duplicate_uuid_across_clients_is_rejected_before_any_snapshot_write() {
        let (_directory, store, mut snapshot) = live_snapshot();
        let mut claude = snapshot.providers[&AppKind::Codex][0].clone();
        claude.position = 100;
        snapshot
            .providers
            .get_mut(&AppKind::Claude)
            .expect("claude group")
            .push(claude);

        let error = enable_snapshot(&store, &snapshot).expect_err("duplicate must fail");
        assert!(error.contains("重复"));
        assert_eq!(
            read_configuration_snapshot(&store)
                .unwrap()
                .provider_count(),
            1
        );
    }

    #[test]
    fn failed_swap_restores_the_original_live_directory() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state_root = directory.path().join("state");
        let live = state_root.join("configuration");
        let staged = state_root.join("staging").join("configuration");
        fs::create_dir_all(&live).unwrap();
        fs::create_dir_all(&staged).unwrap();
        fs::write(live.join("marker"), "old").unwrap();
        fs::write(staged.join("marker"), "new").unwrap();

        let mut calls = 0;
        let mut rename = |from: &Path, to: &Path| {
            calls += 1;
            if calls == 2 {
                return Err(std::io::Error::other("injected staged activation failure"));
            }
            fs::rename(from, to)
        };
        let result = activate_staged(&live, &staged, &state_root, &mut rename);

        assert!(matches!(result, Activation::Restored(_)));
        assert_eq!(fs::read_to_string(live.join("marker")).unwrap(), "old");
    }

    #[test]
    fn failed_rollback_reports_the_preserved_recovery_paths() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state_root = directory.path().join("state");
        let live = state_root.join("configuration");
        let staged = state_root.join("staging").join("configuration");
        fs::create_dir_all(&live).unwrap();
        fs::create_dir_all(&staged).unwrap();
        fs::write(live.join("marker"), "old").unwrap();
        fs::write(staged.join("marker"), "new").unwrap();

        let mut calls = 0;
        let mut rename = |from: &Path, to: &Path| {
            calls += 1;
            if calls == 2 || calls == 3 {
                return Err(std::io::Error::other("injected swap failure"));
            }
            fs::rename(from, to)
        };
        let result = activate_staged(&live, &staged, &state_root, &mut rename);

        let Activation::RecoveryRequired(error) = result else {
            panic!("rollback failure must remain observable")
        };
        assert!(error.contains("原配置保留在"));
        let retired = fs::read_dir(&state_root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("retired-"))
            })
            .expect("retired original remains");
        assert_eq!(fs::read_to_string(retired.join("marker")).unwrap(), "old");
        assert_eq!(fs::read_to_string(staged.join("marker")).unwrap(), "new");
    }
}
