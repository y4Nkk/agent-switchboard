use std::collections::BTreeSet;
use std::io::Read;

use asb_core::extensions::skill::ContentEntry;
use asb_core::extensions::validate::{ContentEntryKind, MAX_CONTENT_FILE_BYTES};

use super::github::{relative_to, validate_subpath};
use super::transport::MAX_ARCHIVE_DOWNLOAD;
use super::{SourceError, MAX_SOURCE_BYTES, MAX_SOURCE_ENTRIES};

const MAX_EXPANDED_ARCHIVE_BYTES: u64 =
    MAX_SOURCE_BYTES + MAX_SOURCE_ENTRIES as u64 * 1024 + 2 * 1024 * 1024;

#[derive(Default)]
struct ArchiveScan {
    root: Option<String>,
    seen: BTreeSet<String>,
    entries: usize,
    bytes: u64,
}

impl ArchiveScan {
    fn inspect<R: Read>(
        &mut self,
        entry: &tar::Entry<'_, R>,
    ) -> Result<Option<String>, SourceError> {
        self.entries += 1;
        if self.entries > MAX_SOURCE_ENTRIES {
            return Err(SourceError::Rejected(format!(
                "压缩包超过 {MAX_SOURCE_ENTRIES} 个条目"
            )));
        }
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err(SourceError::Rejected("压缩包包含链接或特殊条目".into()));
        }
        let path_bytes = entry.path_bytes();
        let path = std::str::from_utf8(&path_bytes)
            .map_err(|_| SourceError::Rejected("压缩包路径不是有效 UTF-8".into()))?;
        let path = if kind.is_dir() {
            path.strip_suffix('/').unwrap_or(path)
        } else {
            path
        };
        if path.is_empty() {
            return Err(SourceError::Rejected("压缩包包含空路径".into()));
        }
        let (root, relative) = path.split_once('/').unwrap_or((path, ""));
        if root.is_empty() {
            return Err(SourceError::Rejected("压缩包包含绝对路径".into()));
        }
        validate_subpath(root)?;
        validate_subpath(relative)?;
        let expected = self.root.get_or_insert_with(|| root.to_string());
        if expected.as_str() != root {
            return Err(SourceError::Rejected(
                "压缩包包含多个不一致的仓库根目录".into(),
            ));
        }
        if relative.is_empty() {
            if !kind.is_dir() {
                return Err(SourceError::Rejected("压缩包缺少仓库根目录".into()));
            }
            return Ok(None);
        }
        if !self.seen.insert(relative.to_lowercase()) {
            return Err(SourceError::Rejected(format!(
                "{relative} 存在重复或大小写冲突"
            )));
        }
        self.bytes = self
            .bytes
            .checked_add(entry.size())
            .ok_or_else(|| SourceError::Rejected("压缩包大小溢出".into()))?;
        if self.bytes > MAX_SOURCE_BYTES {
            return Err(SourceError::Rejected("压缩包内容超过扫描字节上限".into()));
        }
        if kind.is_dir() && entry.size() != 0 {
            return Err(SourceError::Rejected(format!("{relative} 的目录内容无效")));
        }
        Ok(Some(relative.into()))
    }
}

pub(super) fn extract_tar_gz_rooted(
    payload: &[u8],
    subpath: &str,
    commit: &str,
) -> Result<Vec<ContentEntry>, SourceError> {
    validate_subpath(subpath)?;
    if payload.len() as u64 > MAX_ARCHIVE_DOWNLOAD {
        return Err(SourceError::Rejected("压缩包超过下载上限".into()));
    }
    let decoder = flate2::read::GzDecoder::new(payload).take(MAX_EXPANDED_ARCHIVE_BYTES + 1);
    let mut archive = tar::Archive::new(decoder);
    let mut scan = ArchiveScan::default();
    let mut entries = Vec::new();
    for entry in archive.entries().map_err(archive_error)? {
        let mut entry = entry.map_err(archive_error)?;
        if scan.entries == 0 && entry.header().entry_type().is_pax_global_extensions() {
            verify_git_archive_comment(&mut entry, commit)?;
            scan.entries += 1;
            continue;
        }
        let Some(path) = scan.inspect(&entry)? else {
            continue;
        };
        let Some(relative) = relative_to(&path, subpath) else {
            continue;
        };
        entries.push(read_entry(&mut entry, relative)?);
    }
    // tar stops at its end marker before gzip reads the CRC/trailer. Drain the
    // bounded decoder so a truncated or corrupt payload cannot look complete.
    let mut decoder = archive.into_inner();
    std::io::copy(&mut decoder, &mut std::io::sink()).map_err(archive_error)?;
    if decoder.limit() == 0 {
        return Err(SourceError::Rejected(
            "压缩包展开大小超过上限；未接受截断内容".into(),
        ));
    }
    Ok(entries)
}

fn verify_git_archive_comment<R: Read>(
    entry: &mut tar::Entry<'_, R>,
    commit: &str,
) -> Result<(), SourceError> {
    // GitHub's leading global PAX entry records the pinned commit in a single Git comment.
    let mut comment = [0; 52];
    if entry.path_bytes().as_ref() == b"pax_global_header" && entry.size() == comment.len() as u64 {
        entry.read_exact(&mut comment).map_err(archive_error)?;
        let expected = format!("52 comment={}\n", commit.to_ascii_lowercase());
        if comment.as_slice() == expected.as_bytes() {
            return Ok(());
        }
    }
    Err(SourceError::Rejected("压缩包的 Git 提交元数据与请求不一致".into()))
}

fn read_entry<R: Read>(
    entry: &mut tar::Entry<'_, R>,
    relative: &str,
) -> Result<ContentEntry, SourceError> {
    let kind = if entry.header().entry_type().is_dir() {
        ContentEntryKind::Dir
    } else {
        ContentEntryKind::File
    };
    if entry.size() > MAX_CONTENT_FILE_BYTES {
        return Err(SourceError::Rejected(format!(
            "{relative} 超过单文件大小上限"
        )));
    }
    let mut bytes = Vec::new();
    if kind == ContentEntryKind::File {
        entry.read_to_end(&mut bytes).map_err(archive_error)?;
        if bytes.len() as u64 != entry.size() {
            return Err(SourceError::Rejected(format!("{relative} 内容被截断")));
        }
    }
    #[cfg(unix)]
    let mode = entry.header().mode().map_err(archive_error)? & 0o777;
    #[cfg(not(unix))]
    let mode = 0o644;
    Ok(ContentEntry {
        relative_path: relative.into(),
        kind,
        bytes,
        mode: if kind == ContentEntryKind::Dir {
            0o755
        } else {
            mode
        },
    })
}

fn archive_error(error: std::io::Error) -> SourceError {
    SourceError::Rejected(format!("压缩包无法完整读取：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};

    #[test]
    fn reads_git_archive_with_matching_commit_comment() {
        const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
        let comment = format!("52 comment={COMMIT}\n");
        let encoder = GzEncoder::new(Vec::new(), Compression::fast());
        let mut archive = tar::Builder::new(encoder);
        let mut header = tar::Header::new_ustar();
        header.set_entry_type(tar::EntryType::XGlobalHeader);
        header.set_size(comment.len() as u64);
        archive.append_data(&mut header, "pax_global_header", comment.as_bytes()).unwrap();

        let manifest = b"Skill";
        let mut header = tar::Header::new_ustar();
        header.set_mode(0o644);
        header.set_size(manifest.len() as u64);
        archive.append_data(&mut header, "repo/SKILL.md", &manifest[..]).unwrap();
        let bytes = archive.into_inner().unwrap().finish().unwrap();

        let entries = extract_tar_gz_rooted(&bytes, "", COMMIT).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].relative_path, "SKILL.md");
        assert!(extract_tar_gz_rooted(&bytes, "", &"f".repeat(40)).is_err());
    }
}
