use crate::{app_paths, local_state::LocalState};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) const CACHE_BUDGET_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StorageUsage {
    pub data_bytes: u64,
    pub backup_bytes: u64,
    pub history_bytes: u64,
    pub cache_bytes: u64,
    pub cache_budget_bytes: u64,
    pub cache_maintenance: bool,
    pub cache_clearing: bool,
}

pub(super) fn cache_paths(identifier: &str) -> Result<[PathBuf; 2], String> {
    let root = app_paths::local_data_directory(identifier)?.join("EBWebView/Default");
    Ok([root.join("Cache"), root.join("Code Cache")])
}

pub(super) fn cache_bytes(identifier: &str) -> Result<u64, String> {
    cache_paths(identifier)?.iter().try_fold(0_u64, |total, path| Ok(total.saturating_add(size(path)?)))
}

pub(crate) fn usage(local: &LocalState, identifier: &str) -> Result<StorageUsage, String> {
    let root = local.root();
    let history_bytes = [root.join("configuration/history"), root.join("codex/requests.sqlite3"), root.join("codex/probes.sqlite3"),
        root.join("sessions")]
        .iter().try_fold(0_u64, |total, path| size(path).map(|bytes| total.saturating_add(bytes)))?;
    Ok(StorageUsage { data_bytes: size(root)?, backup_bytes: size(&local.backup_dir())?, history_bytes,
        cache_bytes: cache_bytes(identifier)?, cache_budget_bytes: CACHE_BUDGET_BYTES,
        cache_maintenance: cfg!(windows), cache_clearing: super::cache::is_clearing() })
}

pub(super) fn size(path: &Path) -> Result<u64, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(format!("无法统计 {}：{error}", path.display())),
    };
    if is_link(&metadata) { return Err(format!("容量统计拒绝重解析或符号链接：{}", path.display())); }
    if metadata.is_file() { return Ok(metadata.len()); }
    let mut bytes = 0_u64;
    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
        bytes = bytes.saturating_add(size(&entry.map_err(|error| error.to_string())?.path())?);
    }
    Ok(bytes)
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    { use std::os::windows::fs::MetadataExt; metadata.file_attributes() & 0x400 != 0 }
    #[cfg(not(windows))]
    { metadata.file_type().is_symlink() }
}
