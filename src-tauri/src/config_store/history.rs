//! Configuration write history: one JSON file per client under
//! `history/{client}.json`. Only real client-file writes append here; saving
//! providers or common settings never does.

use super::{parse_strict, read_optional, write_json_atomic, ConfigStore, ProfileStoreError};
use asb_core::contracts::{AppKind, ConfigWriteRecord, WriteOperation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct HistoryFile {
    pub(crate) records: Vec<ConfigWriteRecord>,
}

pub(crate) fn validate_write_record(
    app: AppKind,
    record: &ConfigWriteRecord,
) -> Result<(), String> {
    if record.app != app {
        return Err("写入历史记录不属于该客户端文件".to_string());
    }
    if record.content_hash.len() != 64
        || !record
            .content_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("配置写入记录的内容哈希无效".to_string());
    }
    if record.backup_id.trim().is_empty() {
        return Err("配置写入记录缺少备份标识".to_string());
    }
    chrono::DateTime::parse_from_rfc3339(&record.at)
        .map_err(|_| "配置写入记录时间无效".to_string())?;
    let has_profile = record
        .profile_id
        .as_deref()
        .is_some_and(|id| !id.trim().is_empty())
        && record
            .profile_name
            .as_deref()
            .is_some_and(|name| !name.trim().is_empty());
    let has_any_profile = record.profile_id.is_some() || record.profile_name.is_some();
    match record.operation {
        WriteOperation::Projection if has_any_profile && !has_profile => {
            Err("投影记录的供应商标识和名称必须同时存在且非空".to_string())
        }
        WriteOperation::Restore if has_any_profile => {
            Err("非供应商投影记录不能包含供应商信息".to_string())
        }
        _ => Ok(()),
    }
}

impl ConfigStore {
    pub(crate) fn load_history(
        &self,
        app: AppKind,
    ) -> Result<Vec<ConfigWriteRecord>, ProfileStoreError> {
        self.ensure_layout()?;
        match read_optional(&self.history_path(app))? {
            None => Ok(vec![]),
            Some(text) => {
                let file: HistoryFile = parse_strict(&text)?;
                for record in &file.records {
                    validate_write_record(app, record)
                        .map_err(|_| ProfileStoreError::Unsupported)?;
                }
                Ok(file.records)
            }
        }
    }

    /// Appends one completed real client-file write. Provider and common
    /// settings saves deliberately never call this method.
    pub fn record_config_write(&self, entry: ConfigWriteRecord) -> Result<(), String> {
        let app = entry.app;
        validate_write_record(app, &entry)?;
        let mut records = self.load_history(app).map_err(|error| error.to_string())?;
        records.push(entry);
        let json = serde_json::to_string_pretty(&HistoryFile { records })
            .map_err(|_| "写入历史序列化失败".to_string())?;
        write_json_atomic(&self.history_path(app), &json)
    }

    /// The most recent client-file write for one client, if one exists.
    pub fn latest_config_write(
        &self,
        app: AppKind,
    ) -> Result<Option<ConfigWriteRecord>, ProfileStoreError> {
        Ok(self.load_history(app)?.pop())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(app: AppKind, profile: Option<&str>, at: &str) -> ConfigWriteRecord {
        ConfigWriteRecord {
            app,
            profile_id: profile.map(str::to_string),
            profile_name: profile.map(|_| "网关".to_string()),
            content_hash: "a".repeat(64),
            backup_id: "b1".to_string(),
            at: at.to_string(),
            operation: WriteOperation::Projection,
        }
    }

    #[test]
    fn records_split_per_client_and_latest_is_returned() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));

        store
            .record_config_write(record(AppKind::Codex, Some("p1"), "2026-09-01T08:00:00Z"))
            .expect("first");
        store
            .record_config_write(record(AppKind::Claude, Some("c1"), "2026-09-01T09:00:00Z"))
            .expect("claude");
        store
            .record_config_write(record(AppKind::Codex, Some("p2"), "2026-09-01T10:00:00Z"))
            .expect("second");

        let latest = store
            .latest_config_write(AppKind::Codex)
            .expect("latest")
            .expect("present");
        assert_eq!(latest.profile_id.as_deref(), Some("p2"));
        let persisted = std::fs::read_to_string(store.history_path(AppKind::Codex)).unwrap();
        assert!(persisted.contains("\"records\""));
        assert!(persisted.contains("\"profileId\": \"p1\""));

        let claude = store
            .latest_config_write(AppKind::Claude)
            .expect("claude latest")
            .expect("present");
        assert_eq!(claude.profile_id.as_deref(), Some("c1"));
    }

    #[test]
    fn malformed_records_are_rejected_without_any_write() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));

        let mut short_hash = record(AppKind::Codex, Some("p1"), "2026-09-01T08:00:00Z");
        short_hash.content_hash = "abc".to_string();
        assert!(store.record_config_write(short_hash).is_err());
        assert!(!store.history_path(AppKind::Codex).exists());

        std::fs::create_dir_all(
            store
                .history_path(AppKind::Codex)
                .parent()
                .unwrap()
                .to_path_buf(),
        )
        .unwrap();
        std::fs::write(
            store.history_path(AppKind::Codex),
            "{\"records\":[{\"app\":\"codex\",\"profileId\":null,\"profileName\":null,\"contentHash\":\"x\",\"backupId\":\"b\",\"at\":\"2026-09-01T08:00:00Z\",\"operation\":\"restore\"}]}",
        )
        .unwrap();
        assert_eq!(
            store
                .latest_config_write(AppKind::Codex)
                .expect_err("malformed record must fail"),
            ProfileStoreError::Unsupported
        );

        std::fs::write(
            store.history_path(AppKind::Codex),
            "{\"records\":[{\"app\":\"codex\",\"profileId\":null,\"profileName\":null,\"contentHash\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"backupId\":\"b\",\"at\":\"2026-09-01T08:00:00Z\",\"operation\":\"projection\",\"legacyField\":true}]}",
        )
        .unwrap();
        assert_eq!(
            store
                .latest_config_write(AppKind::Codex)
                .expect_err("unknown record members must fail"),
            ProfileStoreError::Unsupported
        );
    }
}
