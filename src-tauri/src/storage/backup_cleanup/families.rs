use crate::local_state::LocalState;
use asb_core::{AppKind, BackupRecord};
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) struct Family {
    pub(super) paths: Vec<PathBuf>,
    pub(super) stamp: DateTime<Utc>,
    pub(super) app: Option<AppKind>,
    pub(super) config: bool,
}

pub(super) fn modified(path: &Path) -> Result<DateTime<Utc>, String> {
    fs::symlink_metadata(path).and_then(|metadata| metadata.modified())
        .map(DateTime::from).map_err(|error| error.to_string())
}

fn walk(root: &Path) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    if !root.try_exists().map_err(|error| error.to_string())? {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut files = Vec::new();
    let mut directories = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
            if super::super::sizes::is_link(&metadata) {
                return Err(format!("备份目录包含链接，未清理：{}", path.display()));
            }
            if !metadata.is_dir() { files.push(path); continue; }
            let opaque = path.extension().is_some_and(|ext| ext == "bak")
                || dir.file_name().is_some_and(|name| name == "gateway-port");
            if opaque { directories.push(path); } else { pending.push(path); }
        }
    }
    Ok((files, directories))
}

fn records(files: &[PathBuf]) -> BTreeMap<PathBuf, BackupRecord> {
    files.iter().filter_map(|meta| {
        let name = meta.file_name()?.to_str()?.strip_suffix(".meta.json")?;
        let bak = meta.parent()?.join(name);
        let record: BackupRecord = serde_json::from_str(&fs::read_to_string(meta).ok()?).ok()?;
        // Invalid and incomplete restore records stay together on disk.
        if PathBuf::from(format!("{}.meta.json", record.backup_path)) != *meta || !bak.exists() {
            return None;
        }
        Some((bak, record))
    }).collect()
}

fn record_paths(bak: &Path) -> Vec<PathBuf> {
    vec![bak.to_path_buf(), PathBuf::from(format!("{}.meta.json", bak.display()))]
}

fn family(
    local: &LocalState, bak: &Path, record: &BackupRecord,
    records: &BTreeMap<PathBuf, BackupRecord>,
) -> Result<Family, String> {
    let config = record.linked_backup_id.is_none()
        && Path::new(&record.target_path) == local.target(record.app)?;
    let mut paths = record_paths(bak);
    if config {
        let snapshot = local.client_settings_backup_path(record)?;
        if snapshot.try_exists().map_err(|error| error.to_string())? { paths.push(snapshot); }
    }
    for (child, linked) in records {
        if linked.app == record.app && linked.linked_backup_id.as_deref() == Some(record.id.as_str()) {
            paths.extend(record_paths(child));
        }
    }
    let stamp = DateTime::parse_from_rfc3339(&record.created_at)
        .map(DateTime::from).map_err(|_| format!("备份创建时间无效：{}", bak.display()))?;
    Ok(Family { paths, stamp, app: Some(record.app), config })
}

pub(super) fn collect(local: &LocalState) -> Result<Vec<Family>, String> {
    let (files, directories) = walk(&local.backup_dir())?;
    let records = records(&files);
    let mut ids = BTreeSet::new();
    for record in records.values() {
        if !ids.insert(record.id.as_str()) { return Err("备份身份重复，未清理恢复材料".into()); }
    }
    let mut families = Vec::new();
    for (bak, record) in &records {
        if record.linked_backup_id.as_deref().is_some_and(|parent| ids.contains(parent)) { continue; }
        families.push(family(local, bak, record, &records)?);
    }
    let claimed: BTreeSet<_> = families.iter().flat_map(|family| family.paths.iter()).cloned().collect();
    for path in files.into_iter().chain(directories) {
        if claimed.contains(&path) { continue; }
        let name = path.file_name().and_then(|name| name.to_str()).ok_or("备份文件名无效")?;
        if name.ends_with(".meta.json") || (name.ends_with(".bak") && !path.is_dir()) { continue; }
        if path.is_dir() && name.ends_with(".bak")
            && PathBuf::from(format!("{}.meta.json", path.display())).try_exists()
                .map_err(|error| error.to_string())?
        { continue; }
        let stamp = modified(&path)?;
        families.push(Family { paths: vec![path], stamp, app: None, config: false });
    }
    families.sort_by(|left, right| left.paths.cmp(&right.paths));
    Ok(families)
}
