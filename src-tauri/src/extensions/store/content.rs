//! Immutable skill content versions: staging, verification, and the
//! per-definition version listing.

use std::fs;
use std::path::Path;

use asb_core::extensions::contracts::EXTENSIONS_SCHEMA_VERSION;
use asb_core::extensions::skill::ContentEntry;
use asb_core::extensions::validate::{validate_content_paths, ContentEntryKind};

use crate::config_store::write_json_atomic;

use super::ExtensionStore;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredContentVersion {
    schema_version: u8,
    digest: String,
    files: Vec<StoredFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredFile {
    relative_path: String,
    /// Explicit entry kind; an empty file is a file, never a directory.
    is_dir: bool,
    mode: u32,
    size: u64,
}

/// Reads one stored version directory back into canon entries, driven by
/// the manifest's explicit kinds.
fn walk_staged(
    version_dir: &Path,
    version: &StoredContentVersion,
) -> Result<Vec<ContentEntry>, ExtensionStoreError> {
    if version.schema_version != EXTENSIONS_SCHEMA_VERSION {
        return Err(ExtensionStoreError::Unsupported(format!(
            "Skill 内容版本 {} 不是当前支持的版本 {}",
            version.schema_version, EXTENSIONS_SCHEMA_VERSION
        )));
    }
    let paths: Vec<(String, ContentEntryKind, u64)> = version
        .files
        .iter()
        .map(|file| {
            (
                file.relative_path.clone(),
                if file.is_dir {
                    ContentEntryKind::Dir
                } else {
                    ContentEntryKind::File
                },
                file.size,
            )
        })
        .collect();
    validate_content_paths(&paths).map_err(|errors| {
        ExtensionStoreError::Unsupported(format!("Skill 内容清单无效：{}", errors.join("；")))
    })?;
    let mut entries = Vec::new();
    for file in &version.files {
        let target = version_dir.join(&file.relative_path);
        if file.is_dir {
            let metadata = fs::symlink_metadata(&target).map_err(|error| {
                ExtensionStoreError::Unreadable(format!("{}：{error}", target.display()))
            })?;
            if !metadata.file_type().is_dir() || file.size != 0 {
                return Err(ExtensionStoreError::Unsupported(format!(
                    "Skill 内容条目 {} 的类型或大小与清单不一致",
                    file.relative_path
                )));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o7777 != file.mode & 0o7777 {
                    return Err(ExtensionStoreError::Unsupported(format!(
                        "Skill 内容条目 {} 的权限与清单不一致",
                        file.relative_path
                    )));
                }
            }
            entries.push(ContentEntry {
                relative_path: file.relative_path.clone(),
                kind: asb_core::extensions::validate::ContentEntryKind::Dir,
                bytes: Vec::new(),
                mode: file.mode,
            });
        } else {
            let metadata = fs::symlink_metadata(&target).map_err(|error| {
                ExtensionStoreError::Unreadable(format!("{}：{error}", target.display()))
            })?;
            if !metadata.file_type().is_file() {
                return Err(ExtensionStoreError::Unsupported(format!(
                    "Skill 内容条目 {} 不是普通文件",
                    file.relative_path
                )));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o7777 != file.mode & 0o7777 {
                    return Err(ExtensionStoreError::Unsupported(format!(
                        "Skill 内容条目 {} 的权限与清单不一致",
                        file.relative_path
                    )));
                }
            }
            let bytes = fs::read(&target).map_err(|error| {
                ExtensionStoreError::Unreadable(format!("{}：{error}", target.display()))
            })?;
            if bytes.len() as u64 != file.size {
                return Err(ExtensionStoreError::Unsupported(format!(
                    "Skill 内容条目 {} 的大小与清单不一致",
                    file.relative_path
                )));
            }
            entries.push(ContentEntry {
                relative_path: file.relative_path.clone(),
                kind: asb_core::extensions::validate::ContentEntryKind::File,
                bytes,
                mode: file.mode,
            });
        }
    }
    if asb_core::extensions::skill::content_digest(&entries) != version.digest {
        return Err(ExtensionStoreError::Unsupported(
            "Skill 内容与其不可变摘要不一致".to_string(),
        ));
    }
    Ok(entries)
}

/// Errors from the extension library.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExtensionStoreError {
    #[error("扩展库不可读：{0}")]
    Unreadable(String),
    #[error("扩展库数据不受支持：{0}")]
    Unsupported(String),
    #[error("{0}")]
    Conflict(String),
    /// A compensating write failed. The operation journal remains the
    /// authoritative recovery source and overlapping writes must stop.
    #[error("扩展库需要恢复：{0}")]
    RecoveryRequired(String),
}

/// Summary of one stored content version, for version history listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredVersionSummary {
    pub digest: String,
    pub file_count: u64,
    pub total_bytes: u64,
}

const CONTENT_MANIFEST_NAME: &str = ".asb-content.json";

impl ExtensionStore {
    /// Publishes one immutable content version. The digest must match the
    /// entries exactly; an already-published version is verified where it
    /// sits, never rewritten, and a mismatched digest never publishes.
    pub fn save_skill_version(
        &self,
        definition_id: &str,
        digest: &str,
        entries: &[ContentEntry],
    ) -> Result<(), ExtensionStoreError> {
        if asb_core::extensions::skill::content_digest(entries) != digest {
            return Err(ExtensionStoreError::Conflict(
                "Skill 内容与其不可变摘要不一致".to_string(),
            ));
        }
        let paths: Vec<(String, ContentEntryKind, u64)> = entries
            .iter()
            .map(|entry| {
                (
                    entry.relative_path.clone(),
                    entry.kind,
                    entry.bytes.len() as u64,
                )
            })
            .collect();
        validate_content_paths(&paths).map_err(|errors| {
            ExtensionStoreError::Unsupported(format!("Skill 内容清单无效：{}", errors.join("；")))
        })?;
        let version = StoredContentVersion {
            schema_version: EXTENSIONS_SCHEMA_VERSION,
            digest: digest.to_string(),
            files: entries
                .iter()
                .map(|entry| StoredFile {
                    relative_path: entry.relative_path.clone(),
                    is_dir: entry.kind == ContentEntryKind::Dir,
                    mode: entry.mode,
                    size: entry.bytes.len() as u64,
                })
                .collect(),
        };
        let version_dir = self.path(&["library", definition_id, digest]);
        if version_dir.join(CONTENT_MANIFEST_NAME).is_file() {
            // Immutability: republishing verifies the stored bytes instead
            // of rewriting them.
            let stored = self.load_skill_version(definition_id, digest)?;
            if asb_core::extensions::skill::content_digest(&stored) != digest {
                return Err(ExtensionStoreError::Conflict(
                    "库中的 Skill 内容与记录摘要不一致".to_string(),
                ));
            }
            return Ok(());
        }
        let parent = self.path(&["library", definition_id]);
        fs::create_dir_all(&parent).map_err(|error| {
            ExtensionStoreError::Unreadable(format!("{}：{error}", parent.display()))
        })?;
        let staging = parent.join(format!(".staging-{}", uuid::Uuid::new_v4().simple()));
        let _ = fs::remove_dir_all(&staging);
        let write_result = (|| -> Result<(), ExtensionStoreError> {
            for entry in entries {
                let target = staging.join(&entry.relative_path);
                if entry.kind == ContentEntryKind::Dir {
                    fs::create_dir_all(&target).map_err(|error| {
                        ExtensionStoreError::Unreadable(format!("{}：{error}", target.display()))
                    })?;
                } else {
                    if let Some(dir) = target.parent() {
                        fs::create_dir_all(dir).map_err(|error| {
                            ExtensionStoreError::Unreadable(format!("{}：{error}", dir.display()))
                        })?;
                    }
                    fs::write(&target, &entry.bytes).map_err(|error| {
                        ExtensionStoreError::Unreadable(format!("{}：{error}", target.display()))
                    })?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        fs::set_permissions(&target, std::fs::Permissions::from_mode(entry.mode))
                            .map_err(|error| {
                            ExtensionStoreError::Unreadable(format!(
                                "{}：{error}",
                                target.display()
                            ))
                        })?;
                    }
                }
            }
            let manifest = serde_json::to_string_pretty(&version)
                .map_err(|error| ExtensionStoreError::Unreadable(error.to_string()))?;
            write_json_atomic(&staging.join(CONTENT_MANIFEST_NAME), &manifest)
                .map_err(ExtensionStoreError::Unreadable)?;
            // The staged tree must read back exactly as the final one will.
            let staged = walk_staged(&staging, &version)?;
            if asb_core::extensions::skill::content_digest(&staged) != digest {
                return Err(ExtensionStoreError::Unsupported(
                    "Skill 内容写入后与摘要不一致".to_string(),
                ));
            }
            Ok(())
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
        fs::rename(&staging, &version_dir).map_err(|error| {
            let _ = fs::remove_dir_all(&staging);
            ExtensionStoreError::Unreadable(format!("{}：{error}", version_dir.display()))
        })?;
        Ok(())
    }

    pub fn load_skill_version(
        &self,
        definition_id: &str,
        digest: &str,
    ) -> Result<Vec<ContentEntry>, ExtensionStoreError> {
        let version_dir = self.path(&["library", definition_id, digest]);
        let manifest_path = version_dir.join(CONTENT_MANIFEST_NAME);
        let version: StoredContentVersion = self.read_json(&manifest_path).map_err(|error| {
            ExtensionStoreError::Unreadable(format!(
                "Skill 内容版本不存在或不可读：{}",
                error.to_string().trim_start_matches("扩展库不可读：")
            ))
        })?;
        if version.digest != digest {
            return Err(ExtensionStoreError::Unsupported(format!(
                "Skill 内容目录 {} 与清单摘要 {} 不一致",
                digest, version.digest
            )));
        }
        walk_staged(&version_dir, &version)
    }

    pub fn list_skill_versions(
        &self,
        definition_id: &str,
    ) -> Result<Vec<StoredVersionSummary>, ExtensionStoreError> {
        let dir = self.path(&["library", definition_id]);
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(ExtensionStoreError::Unreadable(format!(
                    "{}：{error}",
                    dir.display()
                )))
            }
        };
        let mut summaries = Vec::new();
        for entry in entries {
            let path = entry
                .map_err(|error| ExtensionStoreError::Unreadable(error.to_string()))?
                .path();
            if !path.is_dir() {
                continue;
            }
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            if name.starts_with('.') || name.is_empty() {
                continue;
            }
            let manifest_path = path.join(CONTENT_MANIFEST_NAME);
            if !manifest_path.is_file() {
                continue;
            }
            let version: StoredContentVersion = self.read_json(&manifest_path)?;
            if version.digest != name {
                return Err(ExtensionStoreError::Unsupported(format!(
                    "Skill 内容目录 {} 与清单摘要 {} 不一致",
                    name, version.digest
                )));
            }
            let entries = walk_staged(&path, &version)?;
            summaries.push(StoredVersionSummary {
                digest: version.digest,
                file_count: entries
                    .iter()
                    .filter(|entry| entry.kind == ContentEntryKind::File)
                    .count() as u64,
                total_bytes: entries.iter().map(|entry| entry.bytes.len() as u64).sum(),
            });
        }
        summaries.sort_by(|a, b| a.digest.cmp(&b.digest));
        Ok(summaries)
    }
}
