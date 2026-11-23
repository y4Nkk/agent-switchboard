use super::{validate_snapshot, ConfigurationSnapshot, CLOUD_BACKUP_SNAPSHOT_SCHEMA_VERSION};
use serde_json::Value;

pub(super) const UNSUPPORTED_CLOUD_BACKUP_SNAPSHOT: &str = "云端备份不是当前支持的配置数据格式";

pub(crate) fn decode_cloud_backup_snapshot(
    cleartext: &[u8],
) -> Result<ConfigurationSnapshot, String> {
    let mut value: Value = serde_json::from_slice(cleartext)
        .map_err(|_| UNSUPPORTED_CLOUD_BACKUP_SNAPSHOT.to_string())?;
    match value.get("schemaVersion").and_then(Value::as_u64) {
        Some(7) => upgrade_version_seven(&mut value)?,
        Some(version) if version == u64::from(CLOUD_BACKUP_SNAPSHOT_SCHEMA_VERSION) => {}
        _ => return Err("云端备份配置快照版本不受支持".to_string()),
    }
    let snapshot =
        serde_json::from_value(value).map_err(|_| UNSUPPORTED_CLOUD_BACKUP_SNAPSHOT.to_string())?;
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

/// Decode the known previous format before entering current strict readers.
fn upgrade_version_seven(value: &mut Value) -> Result<(), String> {
    let providers = value
        .get_mut("claudeProviders")
        .and_then(Value::as_array_mut)
        .ok_or(UNSUPPORTED_CLOUD_BACKUP_SNAPSHOT)?;
    for provider in providers {
        if let Some(options) = provider.get_mut("modelOptions") {
            asb_core::contracts::upgrade_claude_model_options(options)?;
        }
    }
    value["schemaVersion"] = Value::from(CLOUD_BACKUP_SNAPSHOT_SCHEMA_VERSION);
    Ok(())
}

#[cfg(test)]
mod tests;
