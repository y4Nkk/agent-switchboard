//! Immutable saved client intent paired with a configuration-file backup.
use crate::local_state::LocalState;
use asb_core::contracts::{BackupRecord, SettingsValues};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Snapshot {
    backup: BackupRecord,
    settings: SettingsValues,
    settings_hash: String,
}

fn settings_hash(settings: &SettingsValues) -> Result<String, String> {
    serde_json::to_string(settings)
        .map(|text| asb_switch::sha256_hex(&text))
        .map_err(|error| error.to_string())
}

fn read(path: &Path, backup: &BackupRecord) -> Result<Option<SettingsValues>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("客户端设置备份不可读：{error}")),
    };
    let snapshot: Snapshot = serde_json::from_str(&text)
        .map_err(|error| format!("客户端设置备份格式无效：{error}"))?;
    snapshot.settings.validate_client_settings(backup.app).map_err(|error| error.to_string())?;
    if snapshot.backup != *backup || settings_hash(&snapshot.settings)? != snapshot.settings_hash {
        return Err("客户端设置备份身份或内容哈希不匹配".into());
    }
    Ok(Some(snapshot.settings))
}

pub(super) fn save(state: &LocalState, backup: &BackupRecord, settings: &SettingsValues) -> Result<(), String> {
    settings.validate_client_settings(backup.app).map_err(|error| error.to_string())?;
    let path = state.client_settings_backup_path(backup)?;
    if let Some(existing) = read(&path, backup)? {
        return if existing == *settings { Ok(()) } else { Err("拒绝覆盖已有客户端设置备份".into()) };
    }
    let snapshot = Snapshot {
        backup: backup.clone(), settings: settings.clone(), settings_hash: settings_hash(settings)?,
    };
    let text = serde_json::to_string_pretty(&snapshot).map_err(|error| error.to_string())?;
    crate::config_store::write_json_atomic(&path, &text)?;
    if read(&path, backup)?.as_ref() != Some(settings) {
        return Err("客户端设置备份回读校验失败".into());
    }
    Ok(())
}

pub(super) fn load(state: &LocalState, backup: &BackupRecord) -> Result<Option<SettingsValues>, String> {
    let settings = read(&state.client_settings_backup_path(backup)?, backup)?;
    if settings.is_none() && matches!(backup.reason.as_str(),
        "client-configuration-apply" | "client-configuration-native-defaults"
        | "client-configuration-native-defaults-unmanaged" | "client-configuration-clear-extra-configuration"
        | "manual-client-configuration" | "restore-precheck") {
        return Err("该备份缺少配套客户端设置快照，无法完整恢复；请保留备份并人工核对，不会猜测原设置".into());
    }
    Ok(settings)
}

pub(super) fn diff(state: &LocalState, backup: &BackupRecord) -> Result<Vec<asb_core::KeyChange>, String> {
    let Some(saved) = load(state, backup)? else { return Ok(Vec::new()); };
    let current = state.configuration().get_client_settings(backup.app).map_err(|error| error.to_string())?;
    let mut changes = asb_core::adapter::full_diff(
        asb_core::AppKind::Claude,
        &serde_json::to_string(&current.settings).map_err(|error| error.to_string())?,
        &serde_json::to_string(&saved).map_err(|error| error.to_string())?,
    ).map_err(|error| error.to_string())?;
    for change in &mut changes { change.key = format!("ASB 已保存配置.{}", change.key); }
    Ok(changes)
}

#[cfg(test)]
#[path = "client_settings_backup_tests.rs"]
mod tests;
