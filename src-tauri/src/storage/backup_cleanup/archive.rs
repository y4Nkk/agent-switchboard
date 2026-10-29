use super::{families::Family, BackupCleanupPlan};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

pub(super) struct Material {
    source: PathBuf,
    name: String,
    pub(super) directory: bool,
    pub(super) bytes: u64,
    hash: String,
}

impl Material {
    pub(super) fn descriptor(&self) -> Value {
        json!({"path":self.name, "directory":self.directory, "bytes":self.bytes, "sha256":self.hash})
    }
}

fn digest(mut input: impl Read) -> Result<(u64, String), String> {
    let mut hash = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0; 64 * 1024];
    loop {
        let size = input.read(&mut buffer).map_err(|error| error.to_string())?;
        if size == 0 { break; }
        bytes = bytes.saturating_add(size as u64);
        hash.update(&buffer[..size]);
    }
    Ok((bytes, format!("{:x}", hash.finalize())))
}

pub(super) fn inventory(root: &Path, families: &[Family]) -> Result<Vec<Material>, String> {
    let mut pending: Vec<_> = families.iter().flat_map(|family| family.paths.clone()).collect();
    let mut entries = BTreeMap::new();
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if super::super::sizes::is_link(&metadata) || (!metadata.is_dir() && !metadata.is_file()) {
            return Err(format!("备份材料类型不受支持，未清理：{}", path.display()));
        }
        let relative = path.strip_prefix(root).map_err(|_| "备份材料不属于备份目录")?;
        let relative = relative.to_str().ok_or("备份材料文件名不是有效 Unicode，未清理")?
            .replace(std::path::MAIN_SEPARATOR, "/");
        let name = format!("agent-switchboard-backups/{relative}");
        let (bytes, hash) = if metadata.is_dir() { (0, String::new()) }
            else { digest(File::open(&path).map_err(|error| error.to_string())?)? };
        if metadata.is_dir() {
            for child in fs::read_dir(&path).map_err(|error| error.to_string())? {
                pending.push(child.map_err(|error| error.to_string())?.path());
            }
        }
        if entries.insert(name.clone(), Material {
            source: path, name, directory: metadata.is_dir(), bytes, hash,
        }).is_some() { return Err("备份材料属于重复的恢复组，未清理".into()); }
    }
    Ok(entries.into_values().collect())
}

fn manifest(entries: &[Material], plan: &BackupCleanupPlan) -> Value {
    json!({"format":"agent-switchboard-backup-cleanup", "version":1, "plan":plan,
        "entries":entries.iter().map(Material::descriptor).collect::<Vec<_>>()})
}

pub(super) fn write(path: &Path, entries: &[Material], plan: &BackupCleanupPlan) -> Result<(), String> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let file = options.open(path).map_err(|error| error.to_string())?;
    let output = GzEncoder::new(BufWriter::new(&file), Compression::default());
    let mut archive = tar::Builder::new(output);
    let content = serde_json::to_vec(&manifest(entries, plan)).map_err(|error| error.to_string())?;
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(0o600);
    header.set_cksum();
    archive.append_data(&mut header, "manifest.json", content.as_slice()).map_err(|error| error.to_string())?;
    for entry in entries {
        if entry.directory {
            archive.append_dir(&entry.name, &entry.source).map_err(|error| error.to_string())?;
        } else {
            let mut input = File::open(&entry.source).map_err(|error| error.to_string())?;
            archive.append_file(&entry.name, &mut input).map_err(|error| error.to_string())?;
        }
    }
    archive.into_inner().and_then(|output| output.finish())
        .and_then(|mut output| output.flush()).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())
}

pub(super) fn verify(path: &Path, entries: &[Material], plan: &BackupCleanupPlan) -> Result<(), String> {
    let input = GzDecoder::new(File::open(path).map_err(|error| error.to_string())?);
    let mut archive = tar::Archive::new(input);
    let expected: BTreeMap<_, _> = entries.iter().map(|entry| (entry.name.as_str(), entry)).collect();
    let mut seen = BTreeSet::new();
    let mut got_manifest = false;
    for entry in archive.entries().map_err(|error| error.to_string())? {
        let mut entry = entry.map_err(|error| error.to_string())?;
        let name = entry.path().map_err(|error| error.to_string())?.to_str()
            .ok_or("备份归档文件名不是有效 Unicode，原备份未清理")?
            .replace(std::path::MAIN_SEPARATOR, "/");
        if name == "manifest.json" {
            if !entry.header().entry_type().is_file() { return Err("备份归档清单类型无效，原备份未清理".into()); }
            let value: Value = serde_json::from_reader(&mut entry).map_err(|error| error.to_string())?;
            if got_manifest || value != manifest(entries, plan) { return Err("备份归档清单回读不匹配，原备份未清理".into()); }
            got_manifest = true;
            continue;
        }
        let name = name.trim_end_matches('/');
        let material = expected.get(name).ok_or("备份归档包含计划外材料，原备份未清理")?;
        if !seen.insert(name.to_string()) { return Err("备份归档包含重复材料，原备份未清理".into()); }
        let kind = entry.header().entry_type();
        if material.directory {
            if !kind.is_dir() || entry.header().size().map_err(|error| error.to_string())? != 0 {
                return Err("备份归档目录类型或长度回读不匹配，原备份未清理".into());
            }
        } else if !kind.is_file() || digest(&mut entry)? != (material.bytes, material.hash.clone()) {
            return Err("备份归档内容回读不匹配，原备份未清理".into());
        }
    }
    if !got_manifest || seen.len() != expected.len() { return Err("备份归档材料不完整，原备份未清理".into()); }
    let mut input = archive.into_inner();
    std::io::copy(&mut input, &mut std::io::sink()).map_err(|error| error.to_string())?;
    Ok(())
}
