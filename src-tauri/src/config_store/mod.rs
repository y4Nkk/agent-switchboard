//! The application configuration store: providers, common settings, and
//! write history under `state/configuration/`.
//!
//! Layout (the only persisted shape; the former `profiles.json` exists only
//! as the one-time migration source):
//!
//! ```text
//! state/
//! ├─ configuration/
//! │  ├─ common/{codex,claude}.json
//! │  ├─ providers/{codex,claude}/{uuid}.json
//! │  └─ history/{codex,claude}.json
//! └─ settings.json (app-runtime preferences, owned elsewhere)
//! ```
//!
//! Every mutation validates the typed contract before anything is written,
//! and every file lands through a temporary write plus atomic rename. None
//! of these files is a Codex or Claude Code configuration: switching remains
//! the only writer of real client files.

pub mod common;
pub mod history;
pub mod migration;
pub mod providers;
pub mod snapshot;

use asb_core::contracts::AppKind;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;
/// Read-side failures of the configuration store. `Migration` carries the
/// loud reason a one-time upgrade could not complete.
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileStoreError {
    Unreadable,
    Unsupported,
    Migration(String),
}

impl std::fmt::Display for ProfileStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable => formatter.write_str("配置存储不可读"),
            Self::Unsupported => formatter
                .write_str("配置存储格式无效或来自已不受支持的旧版本；请重置或重新创建供应商数据"),
            Self::Migration(reason) => {
                write!(formatter, "旧版配置数据迁移失败：{reason}；原文件未改动")
            }
        }
    }
}

/// The one configuration store for one app-data directory.
pub struct ConfigStore {
    /// The `state/` directory itself.
    state_root: PathBuf,
}

impl ConfigStore {
    pub fn new(state_root: PathBuf) -> Self {
        Self { state_root }
    }

    pub fn configuration_dir(&self) -> PathBuf {
        self.state_root.join("configuration")
    }

    pub fn legacy_store_path(&self) -> PathBuf {
        self.state_root.join("profiles.json")
    }

    pub(crate) fn migration_archive_dir(&self) -> PathBuf {
        self.state_root.join("migration-archive")
    }

    fn client_dir(&self, kind: &str) -> PathBuf {
        self.configuration_dir().join(kind)
    }

    pub fn common_path(&self, app: AppKind) -> PathBuf {
        self.client_dir("common")
            .join(format!("{}.json", app.dir_name()))
    }

    pub fn providers_dir(&self, app: AppKind) -> PathBuf {
        self.client_dir("providers").join(app.dir_name())
    }

    pub fn history_path(&self, app: AppKind) -> PathBuf {
        self.client_dir("history")
            .join(format!("{}.json", app.dir_name()))
    }

    /// Runs the only supported one-time store migrations, then confirms the
    /// current layout is usable. Runtime readers accept no legacy shape.
    pub fn ensure_layout(&self) -> Result<(), ProfileStoreError> {
        if self.legacy_store_path().exists() {
            if self.configuration_dir().exists() {
                return Err(ProfileStoreError::Unsupported);
            }
            migration::run(self).map_err(ProfileStoreError::Migration)?;
        }
        migration::migrate_provider_route_mode(self).map_err(ProfileStoreError::Migration)
    }

    /// Removes every persisted provider, common setting, and history record.
    /// The reset is the recovery path for unreadable legacy data.
    pub fn reset(&self) -> Result<(), String> {
        if let Err(error) = fs::remove_dir_all(self.configuration_dir()) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err("无法删除配置存储目录".to_string());
            }
        }
        if let Err(error) = fs::remove_file(self.legacy_store_path()) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err("无法删除旧版配置数据".to_string());
            }
        }
        if let Err(error) = fs::remove_dir_all(self.migration_archive_dir()) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err("无法删除迁移恢复数据".to_string());
            }
        }
        Ok(())
    }
}

/// Deterministic content revision for optimistic saves. Like the previous
/// base revision it is a content fingerprint, not a security primitive.
pub(crate) fn content_revision(bytes: &[u8]) -> String {
    let mut value = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{value:016x}")
}

/// Writes one JSON file through a sibling temporary file and an atomic
/// rename, creating parent directories on demand.
pub(crate) fn write_json_atomic(path: &Path, json: &str) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "无效的存储路径".to_string())?;
    fs::create_dir_all(parent).map_err(|_| "无法创建配置存储目录".to_string())?;
    let temporary = parent.join(format!(
        "{}.{}.tmp",
        safe_file_stem(path),
        Uuid::new_v4().simple()
    ));
    fs::write(&temporary, json).map_err(|_| "无法写入配置存储临时文件".to_string())?;
    if fs::rename(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("无法原子保存配置存储".to_string());
    }
    Ok(())
}

fn safe_file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "store".to_string())
}

/// Reads one JSON file, mapping absence to `None`.
pub(crate) fn read_optional(path: &Path) -> Result<Option<String>, ProfileStoreError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(ProfileStoreError::Unreadable),
    }
}

/// Strictly parses one JSON document; any shape drift is unsupported.
pub(crate) fn parse_strict<T: serde::de::DeserializeOwned>(
    text: &str,
) -> Result<T, ProfileStoreError> {
    serde_json::from_str(text).map_err(|_| ProfileStoreError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_revision_is_stable_and_order_sensitive() {
        let bytes = b"disable_response_storage: true";
        assert_eq!(content_revision(bytes), content_revision(bytes));
        assert_ne!(content_revision(bytes), content_revision(b"other"));
    }

    #[test]
    fn reset_removes_layout_and_legacy_file_without_requiring_them() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));
        fs::create_dir_all(store.providers_dir(AppKind::Codex)).expect("provider dir");
        fs::write(store.legacy_store_path(), b"{}").expect("legacy file");
        fs::create_dir_all(store.migration_archive_dir()).expect("migration archive");
        fs::write(store.migration_archive_dir().join("source.json"), b"{}").expect("archive file");

        store.reset().expect("reset");

        assert!(!store.configuration_dir().exists());
        assert!(!store.legacy_store_path().exists());
        assert!(!store.migration_archive_dir().exists());
        store.reset().expect("reset of an absent layout is fine");
    }

    #[test]
    fn ensure_layout_accepts_a_clean_state_and_rejects_both_present() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));
        store
            .ensure_layout()
            .expect("clean state needs no migration");

        fs::create_dir_all(store.configuration_dir()).expect("configuration dir");
        fs::write(store.legacy_store_path(), b"{}").expect("legacy file");
        assert_eq!(
            store.ensure_layout().expect_err("both present must fail"),
            ProfileStoreError::Unsupported
        );
    }
}
