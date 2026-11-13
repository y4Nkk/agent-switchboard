//! Client-settings storage: one complete automatic-or-explicit intent file
//! per client under `client-settings/{client}.json`. Saving it never reads, previews,
//! or writes a real Codex or Claude Code configuration file.

use super::{
    content_revision, parse_file, read_optional, write_json_atomic, ConfigStore,
    ProfileStoreError, StoreOperationError,
};
use asb_core::contracts::{AppKind, ClientSettingsSnapshot, SettingsValues};
use asb_core::ownership::default_client_settings;
use std::sync::Mutex;

/// Serializes compare-and-save within this process so two simultaneous editor
/// requests cannot both accept the same optimistic revision.
static CLIENT_SAVE_LOCK: Mutex<()> = Mutex::new(());

impl ConfigStore {
    /// Reads one client's complete client settings and their storage
    /// revision. An absent file yields the directory defaults without
    /// creating anything; every present file must contain the complete,
    /// current automatic-or-explicit intent set.
    pub fn get_client_settings(
        &self,
        app: AppKind,
    ) -> Result<ClientSettingsSnapshot, ProfileStoreError> {
        self.ensure_layout()?;
        let snapshot = match read_optional(&self.client_settings_path(app))? {
            None => snapshot_of(default_client_settings(app)),
            Some(text) => {
                let settings: SettingsValues = parse_file(&text, &self.client_settings_path(app))?;
                settings
                    .validate_client_settings(app)
                    .map_err(|error| ProfileStoreError::invalid(&self.client_settings_path(app), error.to_string()))?;
                ClientSettingsSnapshot {
                    settings_hash: content_revision(text.as_bytes()),
                    settings,
                }
            }
        };
        Ok(snapshot)
    }

    /// Replaces one client's settings after the optimistic revision
    /// check. The saved shape is the complete intent set; this operation
    /// never touches the real client configuration.
    pub fn save_client_settings(
        &self,
        app: AppKind,
        settings: SettingsValues,
        expected_hash: &str,
    ) -> Result<ClientSettingsSnapshot, StoreOperationError> {
        settings
            .validate_client_settings(app)
            .map_err(|error| error.to_string())?;
        let _guard = CLIENT_SAVE_LOCK
            .lock()
            .map_err(|_| StoreOperationError::Invalid("客户端设置保存锁异常".to_string()))?;
        let current = self.get_client_settings(app)?;
        if current.settings_hash != expected_hash {
            return Err("客户端设置已更新，请重新加载后再保存".into());
        }
        let json = serde_json::to_string_pretty(&settings)
            .map_err(|_| StoreOperationError::Invalid("客户端设置序列化失败".to_string()))?;
        write_json_atomic(&self.client_settings_path(app), &json)?;
        Ok(ClientSettingsSnapshot {
            settings_hash: content_revision(json.as_bytes()),
            settings,
        })
    }
}

fn snapshot_of(settings: SettingsValues) -> ClientSettingsSnapshot {
    let json = serde_json::to_string_pretty(&settings).expect("client settings serialize");
    ClientSettingsSnapshot {
        settings_hash: content_revision(json.as_bytes()),
        settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::{ConfigValue, SettingValue};
    use std::fs;

    #[test]
    fn an_absent_file_yields_defaults_without_creating_one() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));

        let snapshot = store.get_client_settings(AppKind::Codex).expect("defaults");
        assert_eq!(snapshot.settings, default_client_settings(AppKind::Codex));
        assert!(!store.client_settings_path(AppKind::Codex).exists());
    }

    #[test]
    fn saving_persists_the_complete_value_set_and_rejects_stale_revisions() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));

        let initial = store.get_client_settings(AppKind::Codex).expect("initial");
        let mut changed = initial.settings.clone();
        changed.settings.insert(
            "approval_policy".to_string(),
            SettingValue::Explicit {
                value: ConfigValue::Str("on-request".into()),
            },
        );
        let saved = store
            .save_client_settings(AppKind::Codex, changed, &initial.settings_hash)
            .expect("save");
        assert_ne!(saved.settings_hash, initial.settings_hash);
        let persisted = fs::read_to_string(store.client_settings_path(AppKind::Codex)).unwrap();
        assert!(persisted.contains("\"mode\": \"explicit\""));
        assert!(persisted.contains("\"value\": \"on-request\""));
        assert!(!persisted.contains("disable_response_storage"));

        // The save response is itself a usable optimistic revision; it must
        // not require a reload merely because the file is pretty-printed.
        let saved_again = store
            .save_client_settings(AppKind::Codex, saved.settings.clone(), &saved.settings_hash)
            .expect("returned revision remains usable");
        assert_eq!(saved_again.settings_hash, saved.settings_hash);

        let error = store
            .save_client_settings(
                AppKind::Codex,
                saved.settings.clone(),
                &initial.settings_hash,
            )
            .expect_err("stale revision must fail");
        assert!(matches!(
            &error,
            StoreOperationError::Invalid(message) if message.contains("重新加载")
        ));
    }

    #[test]
    fn a_hand_edited_file_must_be_complete_and_contain_only_owned_keys() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));
        let path = store.client_settings_path(AppKind::Claude);
        std::fs::create_dir_all(path.parent().expect("client settings dir")).unwrap();
        std::fs::write(
            &path,
            "{\n  \"settings\": {\n    \"outputStyle\": { \"mode\": \"automatic\" }\n  }\n}",
        )
        .unwrap();

        assert!(matches!(
            store
                .get_client_settings(AppKind::Claude)
                .expect_err("partial file must fail"),
            ProfileStoreError::InvalidFile { .. }
        ));

        std::fs::write(
            &path,
            "{\n  \"settings\": {\n    \"permissions\": { \"mode\": \"explicit\", \"value\": true }\n  }\n}",
        )
        .unwrap();
        assert!(matches!(
            store
                .get_client_settings(AppKind::Claude)
                .expect_err("host key must fail"),
            ProfileStoreError::InvalidFile { .. }
        ));
    }
}
