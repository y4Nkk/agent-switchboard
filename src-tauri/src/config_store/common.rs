//! Common-settings storage: one complete plain-value file per client under
//! `common/{client}.json`. Saving these values never reads, previews, or
//! writes a real Codex or Claude Code configuration file.

use super::{
    content_revision, parse_strict, read_optional, write_json_atomic, ConfigStore,
    ProfileStoreError,
};
use asb_core::contracts::{AppKind, CommonSettings, CommonSettingsSnapshot};
use asb_core::ownership::default_common_settings;
use std::sync::Mutex;

/// Serializes compare-and-save within this process so two simultaneous editor
/// requests cannot both accept the same optimistic revision.
static COMMON_SAVE_LOCK: Mutex<()> = Mutex::new(());

impl ConfigStore {
    /// Reads one client's complete common settings and their storage
    /// revision. An absent file yields the directory defaults without
    /// creating anything; every present file must contain the complete,
    /// current value set.
    pub fn get_common_settings(
        &self,
        app: AppKind,
    ) -> Result<CommonSettingsSnapshot, ProfileStoreError> {
        self.ensure_layout()?;
        let snapshot = match read_optional(&self.common_path(app))? {
            None => snapshot_of(default_common_settings(app)),
            Some(text) => {
                let settings: CommonSettings = parse_strict(&text)?;
                settings
                    .validate_for(app)
                    .map_err(|_| ProfileStoreError::Unsupported)?;
                CommonSettingsSnapshot {
                    settings_hash: content_revision(text.as_bytes()),
                    settings,
                }
            }
        };
        Ok(snapshot)
    }

    /// Replaces one client's common settings after the optimistic revision
    /// check. The saved shape is the complete value set; this operation never
    /// touches the real client configuration.
    pub fn save_common_settings(
        &self,
        app: AppKind,
        settings: CommonSettings,
        expected_hash: &str,
    ) -> Result<CommonSettingsSnapshot, String> {
        settings
            .validate_for(app)
            .map_err(|error| error.to_string())?;
        let _guard = COMMON_SAVE_LOCK
            .lock()
            .map_err(|_| "通用设置保存锁异常".to_string())?;
        let current = self
            .get_common_settings(app)
            .map_err(|error| error.to_string())?;
        if current.settings_hash != expected_hash {
            return Err("通用设置已更新，请重新加载后再保存".to_string());
        }
        let json = serde_json::to_string_pretty(&settings)
            .map_err(|_| "通用设置序列化失败".to_string())?;
        write_json_atomic(&self.common_path(app), &json)?;
        Ok(CommonSettingsSnapshot {
            settings_hash: content_revision(json.as_bytes()),
            settings,
        })
    }
}

fn snapshot_of(settings: CommonSettings) -> CommonSettingsSnapshot {
    let json = serde_json::to_string_pretty(&settings).expect("common settings serialize");
    CommonSettingsSnapshot {
        settings_hash: content_revision(json.as_bytes()),
        settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::ConfigValue;
    use std::fs;

    #[test]
    fn an_absent_file_yields_defaults_without_creating_one() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));

        let snapshot = store.get_common_settings(AppKind::Codex).expect("defaults");
        assert_eq!(snapshot.settings, default_common_settings(AppKind::Codex));
        assert!(!store.common_path(AppKind::Codex).exists());
    }

    #[test]
    fn saving_persists_the_complete_value_set_and_rejects_stale_revisions() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));

        let initial = store.get_common_settings(AppKind::Codex).expect("initial");
        let mut changed = initial.settings.clone();
        changed.settings.insert(
            "model_reasoning_effort".to_string(),
            ConfigValue::Str("xhigh".into()),
        );
        let saved = store
            .save_common_settings(AppKind::Codex, changed, &initial.settings_hash)
            .expect("save");
        assert_ne!(saved.settings_hash, initial.settings_hash);
        let persisted = fs::read_to_string(store.common_path(AppKind::Codex)).unwrap();
        assert!(persisted.contains("\"model_reasoning_effort\": \"xhigh\""));
        assert!(persisted.contains("\"disable_response_storage\""));

        // The save response is itself a usable optimistic revision; it must
        // not require a reload merely because the file is pretty-printed.
        let saved_again = store
            .save_common_settings(AppKind::Codex, saved.settings.clone(), &saved.settings_hash)
            .expect("returned revision remains usable");
        assert_eq!(saved_again.settings_hash, saved.settings_hash);

        let error = store
            .save_common_settings(
                AppKind::Codex,
                saved.settings.clone(),
                &initial.settings_hash,
            )
            .expect_err("stale revision must fail");
        assert!(error.contains("重新加载"));
    }

    #[test]
    fn a_hand_edited_file_must_be_complete_and_contain_only_owned_keys() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));
        let path = store.common_path(AppKind::Claude);
        std::fs::create_dir_all(path.parent().expect("common dir")).unwrap();
        std::fs::write(
            &path,
            "{\n  \"settings\": {\n    \"outputStyle\": \"learning\"\n  }\n}",
        )
        .unwrap();

        assert_eq!(
            store
                .get_common_settings(AppKind::Claude)
                .expect_err("partial file must fail"),
            ProfileStoreError::Unsupported
        );

        std::fs::write(
            &path,
            "{\n  \"settings\": {\n    \"permissions\": true\n  }\n}",
        )
        .unwrap();
        assert_eq!(
            store
                .get_common_settings(AppKind::Claude)
                .expect_err("host key must fail"),
            ProfileStoreError::Unsupported
        );
    }
}
