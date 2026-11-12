//! File primitives shared by the application-owned recovery transactions.
use sha2::{Digest, Sha256};
use std::{fs::{self, OpenOptions}, io::{Read, Write}, path::Path};

pub(crate) fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
    read_bounded(path, u64::MAX)
}

pub(crate) fn read_bounded(path: &Path, maximum: u64) -> Result<Option<Vec<u8>>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}：{error}", path.display())),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(format!("不是可安全处理的普通文件：{}", path.display()));
    }
    if metadata.len() > maximum { return Err(format!("文件超过读取上限：{}", path.display())); }
    let file = fs::File::open(path).map_err(|error| format!("{}：{error}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(maximum.saturating_add(1)).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > maximum { return Err(format!("文件超过读取上限：{}", path.display())); }
    Ok(Some(bytes))
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|error| format!("无法创建 {}：{error}", path.display()))?;
    file.write_all(bytes).and_then(|_| file.sync_all())
        .map_err(|error| format!("无法完整保存 {}：{error}", path.display()))?;
    if read(path)?.as_deref() != Some(bytes) {
        return Err(format!("保存后回读不一致：{}", path.display()));
    }
    Ok(())
}

pub(crate) fn replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    write_new(&temporary, bytes)?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("无法启用 {}：{error}", path.display()));
    }
    if read(path)?.as_deref() != Some(bytes) {
        return Err(format!("启用后回读不一致：{}", path.display()));
    }
    Ok(())
}

pub(crate) fn preserve(path: &Path, bytes: &[u8]) -> Result<(), String> {
    match read(path)? {
        Some(current) if current == bytes => Ok(()),
        Some(_) => Err(format!("恢复材料已变化，保留全部文件：{}", path.display())),
        None => write_new(path, bytes),
    }
}
