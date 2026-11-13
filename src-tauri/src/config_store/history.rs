//! Configuration write history: one JSON file per client under
//! `history/{client}.json`. Only real client-file writes append here; saving
//! providers or client settings never does.

mod read;
mod write;
mod archive;
use super::{ConfigStore, ProfileStoreError, StoreOperationError};
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
        WriteOperation::GatewayPortChange if !has_profile => {
            Err("网关端口修改记录必须关联完整的供应商标识和名称".to_string())
        }
        WriteOperation::Projection if has_any_profile && !has_profile => {
            Err("供应商关联写入的标识和名称必须同时存在且非空".to_string())
        }
        WriteOperation::Restore if has_any_profile => {
            Err("非供应商投影记录不能包含供应商信息".to_string())
        }
        _ => Ok(()),
    }
}

impl ConfigStore {
    pub(crate) fn visit_history(&self, app: AppKind,
        mut visitor: impl FnMut(ConfigWriteRecord) -> Result<(), String>) -> Result<(), StoreOperationError> {
        self.ensure_layout()?;
        read::visit(&self.history_path(app), app, &mut visitor)
    }

    pub(crate) fn load_history(
        &self,
        app: AppKind,
    ) -> Result<Vec<ConfigWriteRecord>, ProfileStoreError> {
        let mut records = Vec::new();
        self.visit_history(app, |record| { records.push(record); Ok(()) }).map_err(|error| store_error(error, &self.history_path(app)))?;
        Ok(records)
    }

    /// Appends one completed real client-file write. Provider and client
    /// settings saves deliberately never call this method.
    pub fn record_config_write(&self, entry: ConfigWriteRecord) -> Result<(), StoreOperationError> {
        let app = entry.app;
        validate_write_record(app, &entry)?;
        self.rewrite_history(app, |_| true, Some(&entry))
    }

    /// Removes one transaction-owned write fact only when it is still the
    /// latest fact for that client. Port-change recovery uses this after it
    /// has restored the matching client file; refusing to remove a newer
    /// record prevents it from corrupting undo history after another write.
    pub(crate) fn remove_config_write_if_last(
        &self,
        entry: &ConfigWriteRecord,
    ) -> Result<bool, StoreOperationError> {
        validate_write_record(entry.app, entry)?;
        let mut count = 0;
        let mut latest = None;
        self.visit_history(entry.app, |record| { count += 1; latest = Some(record); Ok(()) })?;
        let Some(last) = latest.as_ref() else {
            return Ok(false);
        };
        if last != entry {
            return Err("配置写入历史已被后续操作更新，拒绝删除恢复记录".into());
        }
        let mut index = 0;
        self.rewrite_history(entry.app, |_| { index += 1; index < count }, None)?;
        Ok(true)
    }

    /// The most recent client-file write for one client, if one exists.
    pub fn latest_config_write(
        &self,
        app: AppKind,
    ) -> Result<Option<ConfigWriteRecord>, ProfileStoreError> {
        let mut latest = None;
        self.visit_history(app, |record| { latest = Some(record); Ok(()) }).map_err(|error| store_error(error, &self.history_path(app)))?;
        Ok(latest)
    }
}

fn store_error(error: StoreOperationError, path: &std::path::Path) -> ProfileStoreError {
    match error {
        StoreOperationError::Store(error) => error,
        StoreOperationError::Invalid(reason) => ProfileStoreError::invalid(path, reason),
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
        let stored: serde_json::Value = serde_json::from_str(&persisted).unwrap();
        assert_eq!(stored["records"][0]["profileId"], "p1");

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

        let mut unowned_gateway_change = record(AppKind::Codex, None, "2026-09-01T08:00:00Z");
        unowned_gateway_change.operation = WriteOperation::GatewayPortChange;
        assert!(store.record_config_write(unowned_gateway_change).is_err());

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
            ProfileStoreError::invalid(&store.history_path(AppKind::Codex), "配置写入记录的内容哈希无效")
        );

        std::fs::write(
            store.history_path(AppKind::Codex),
            "{\"records\":[{\"app\":\"codex\",\"profileId\":null,\"profileName\":null,\"contentHash\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"backupId\":\"b\",\"at\":\"2026-09-01T08:00:00Z\",\"operation\":\"projection\",\"legacyField\":true}]}",
        )
        .unwrap();
        assert!(matches!(store.latest_config_write(AppKind::Codex),
            Err(ProfileStoreError::InvalidFile { path, .. })
                if path == store.history_path(AppKind::Codex)));
    }

    #[test]
    fn removes_only_the_matching_latest_record() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));
        let first = record(AppKind::Claude, Some("p1"), "2026-09-01T08:00:00Z");
        let second = record(AppKind::Claude, Some("p2"), "2026-09-01T09:00:00Z");
        store.record_config_write(first.clone()).unwrap();
        store.record_config_write(second.clone()).unwrap();

        assert!(store.remove_config_write_if_last(&second).unwrap());
        assert_eq!(
            store.latest_config_write(AppKind::Claude).unwrap(),
            Some(first)
        );
        assert!(store.remove_config_write_if_last(&second).is_err());
    }
}
