use super::{capture, db_error, store, Transcript};
use crate::session_manager::{images, SessionMeta};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

pub(super) struct Inventory { pub files: BTreeMap<String, PathBuf>, pub stamp: String, pub rewritten: bool }

pub(super) fn inventory(connection: &Connection, id: &str, transcript: &Path, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<Inventory, String> {
    let mut files = BTreeMap::new();
    let sidecar = transcript.with_extension("");
    if sidecar.try_exists().map_err(io_error)? { collect(&sidecar, transcript.parent().ok_or("会话目录无效")?, &mut files, checkpoint)?; }
    let project: Option<String> = connection.query_row("SELECT meta FROM transcripts WHERE id=?1", [id], |row| row.get(0))
        .optional().map_err(db_error)?;
    let project = project.map(|text| serde_json::from_str::<SessionMeta>(&text).map_err(io_error)).transpose()?;
    let mut statement = connection.prepare("SELECT reference,relative_path FROM attachments WHERE source=?1
        UNION SELECT reference,relative_path FROM image_references WHERE source=?1").map_err(db_error)?;
    let values = statement.query_map([id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))).map_err(db_error)?;
    for value in values {
        checkpoint()?;
        let (reference, relative) = value.map_err(db_error)?;
        let file = reference_path(&reference, project.as_ref().and_then(|meta| meta.project_dir.as_deref()))?;
        if file.try_exists().map_err(io_error)? { files.insert(relative, file); }
    }
    Ok(Inventory { stamp: inventory_stamp(&files, checkpoint)?, rewritten: changed_assets(connection, id, &files, checkpoint)?, files })
}

fn changed_assets(connection: &Connection, source: &str, files: &BTreeMap<String, PathBuf>, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<bool, String> {
    let mut statement = connection.prepare("SELECT stamp FROM attachments WHERE source=?1 AND relative_path=?2 LIMIT 1").map_err(db_error)?;
    for (relative, file) in files {
        checkpoint()?;
        let previous: Option<String> = statement.query_row(params![source,relative], |row| row.get(0)).optional().map_err(db_error)?;
        if let Some(previous) = previous {
            if capture::stamp(&file.metadata().map_err(io_error)?)? != previous { return Ok(true); }
        }
    }
    Ok(false)
}

fn inventory_stamp(files: &BTreeMap<String, PathBuf>, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<String, String> {
    let mut hash = Sha256::new();
    for (relative, file) in files {
        checkpoint()?;
        hash.update(relative.as_bytes());
        hash.update(capture::stamp(&file.metadata().map_err(io_error)?)?.as_bytes());
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn collect(directory: &Path, parent: &Path, files: &mut BTreeMap<String, PathBuf>, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<(), String> {
    let mut pending = vec![directory.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).map_err(io_error)? {
            checkpoint()?;
            let entry = entry.map_err(io_error)?;
            let kind = entry.file_type().map_err(io_error)?;
            if capture::is_link(&std::fs::symlink_metadata(entry.path()).map_err(io_error)?) {
                return Err(format!("会话附件是链接，未跨出来源目录读取：{}", entry.path().display()));
            }
            if kind.is_dir() { pending.push(entry.path()); }
            else if kind.is_file() {
                let path = entry.path();
                let subagent = path.parent().and_then(Path::file_name).is_some_and(|name| name == "subagents");
                if subagent && path.extension().is_some_and(|extension| extension == "jsonl") { continue; }
                let relative = capture::text(path.strip_prefix(parent).map_err(io_error)?)?.to_string();
                files.insert(relative, path);
            }
        }
    }
    Ok(())
}

pub(super) fn save(connection: &Connection, id: &str, transcript: &Path,
    inventory: &Inventory, references: &BTreeSet<String>, checkpoint: &dyn Fn() -> Result<(), String>,
) -> Result<(Vec<String>, String), String> {
    let mut files = inventory.files.clone();
    let mut selected = BTreeMap::new();
    let mut issues = Vec::new();
    let meta: String = connection.query_row("SELECT meta FROM transcripts WHERE id=?1", [id], |row| row.get(0)).map_err(db_error)?;
    let meta: SessionMeta = serde_json::from_str(&meta).map_err(io_error)?;
    connection.execute("DELETE FROM image_references WHERE source=?1", [id]).map_err(db_error)?;
    for reference in references {
        checkpoint()?;
        let file = match reference_path(reference, meta.project_dir.as_deref()) {
            Ok(file) => file,
            Err(error) => { issues.push(error); continue; }
        };
        let relative = if file.starts_with(transcript.parent().ok_or("会话目录无效")?) {
            capture::text(file.strip_prefix(transcript.parent().ok_or("会话目录无效")?).map_err(io_error)?)?.to_string()
        } else {
            format!("images/{}-{}", asb_switch::sha256_hex(reference), file.file_name().and_then(|name| name.to_str()).ok_or("附件文件名无效")?)
        };
        connection.execute("INSERT INTO image_references(source,reference,relative_path) VALUES(?1,?2,?3)",
            params![id,reference,relative]).map_err(db_error)?;
        if !file.try_exists().map_err(io_error)? {
            let retained: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM attachments WHERE source=?1 AND reference=?2)",
                params![id,reference], |row| row.get(0)).map_err(db_error)?;
            if retained { continue; }
            issues.push(format!("原图片已不存在，正文已保留，附件不可恢复：{reference}"));
            continue;
        }
        let mut header = [0u8; 12];
        let count = File::open(&file).map_err(io_error)?.read(&mut header).map_err(io_error)?;
        images::data_uri(&header[..count])?;
        files.insert(relative.clone(), file.clone());
        selected.insert(reference.clone(), (relative, file));
    }
    let stamp = inventory_stamp(&files, checkpoint)?;
    for (relative, file) in &files { save_file(connection, id, capture::text(file)?, relative, file, checkpoint)?; }
    for (reference, (relative, file)) in selected { save_file(connection, id, &reference, &relative, &file, checkpoint)?; }
    Ok((issues, stamp))
}

fn save_file(connection: &Connection, source: &str, reference: &str, relative: &str, path: &Path, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<(), String> {
    checkpoint()?;
    validate_relative(relative)?;
    let metadata = std::fs::symlink_metadata(path).map_err(io_error)?;
    if capture::is_link(&metadata) || !metadata.is_file() { return Err(format!("附件不是普通文件：{}", path.display())); }
    let before = capture::stamp(&metadata)?;
    let previous: Option<String> = connection.query_row("SELECT stamp FROM attachments WHERE source=?1 AND reference=?2",
        params![source,reference], |row| row.get(0)).optional().map_err(db_error)?;
    if previous.as_deref() == Some(&before) { return Ok(()); }
    let hash = capture::digest_observed(path, None, checkpoint)?;
    let known: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM assets WHERE id=?1)", [&hash], |row| row.get(0)).map_err(db_error)?;
    if !known {
        connection.execute("INSERT INTO assets(id,size) VALUES(?1,?2)", params![hash,metadata.len()]).map_err(db_error)?;
        let mut input = File::open(path).map_err(io_error)?;
        let mut buffer = vec![0u8; 1_048_576];
        let mut ordinal = 0;
        let mut copied = Sha256::new();
        loop {
            checkpoint()?;
            let count = input.read(&mut buffer).map_err(io_error)?;
            if count == 0 { break; }
            copied.update(&buffer[..count]);
            connection.execute("INSERT INTO asset_chunks(asset,ordinal,data) VALUES(?1,?2,?3)",
                params![hash,ordinal,&buffer[..count]]).map_err(db_error)?;
            ordinal += 1;
        }
        if format!("{:x}", copied.finalize()) != hash { return Err("附件在读取期间发生变化，未替换已保留内容".into()); }
    }
    if before != capture::stamp(&path.metadata().map_err(io_error)?)? { return Err("附件在读取期间发生变化，请重试".into()); }
    connection.execute("INSERT INTO attachments(source,reference,relative_path,stamp,asset) VALUES(?1,?2,?3,?4,?5)
        ON CONFLICT(source,reference) DO UPDATE SET relative_path=excluded.relative_path,stamp=excluded.stamp,asset=excluded.asset",
        params![source,reference,relative,before,hash]).map_err(db_error)?;
    Ok(())
}

fn reference_path(reference: &str, project: Option<&str>) -> Result<PathBuf, String> {
    let path = Path::new(reference);
    if path.is_absolute() { return Ok(path.into()); }
    project.map(|project| Path::new(project).join(path)).ok_or_else(|| format!("图片附件缺少可解析的原项目目录：{reference}"))
}

pub(super) fn validate_relative(relative: &str) -> Result<(), String> {
    let path = Path::new(relative);
    if relative.is_empty() || path.components().any(|part| !matches!(part, Component::Normal(_))) {
        return Err("已保留附件的相对位置无效".into());
    }
    Ok(())
}

pub(super) fn copy_asset(connection: &Connection, asset: &str, writer: &mut impl Write) -> Result<(), String> {
    let expected: u64 = connection.query_row("SELECT size FROM assets WHERE id=?1", [asset], |row| row.get(0)).map_err(db_error)?;
    let mut statement = connection.prepare("SELECT data FROM asset_chunks WHERE asset=?1 ORDER BY ordinal").map_err(db_error)?;
    let mut rows = statement.query([asset]).map_err(db_error)?;
    let mut hash = Sha256::new();
    let mut length = 0u64;
    while let Some(row) = rows.next().map_err(db_error)? {
        let bytes: Vec<u8> = row.get(0).map_err(db_error)?;
        hash.update(&bytes);
        length += bytes.len() as u64;
        writer.write_all(&bytes).map_err(io_error)?;
    }
    if length != expected || format!("{:x}", hash.finalize()) != asset { return Err("已保留附件校验失败，未使用不完整内容".into()); }
    Ok(())
}

pub(super) fn resolve_images(sources: &[Transcript], references: &[String]) -> (Vec<String>, Vec<String>) {
    let mut images = Vec::new();
    let mut issues = Vec::new();
    let mut length = 0usize;
    for reference in references {
        let resolved = if reference.starts_with("data:image/") { Ok(reference.clone()) } else { saved_image(sources, reference) };
        match resolved {
            Ok(image) if length + image.len() <= 32 * 1024 * 1024 => { length += image.len(); images.push(image); }
            Ok(_) => issues.push("图片附件合计超过 32 MiB 单次读取上限；文本仍可阅读".into()),
            Err(error) => issues.push(error),
        }
    }
    (images, issues)
}

pub(super) fn bind_images(source: &Transcript, references: Vec<String>) -> Vec<String> {
    references.into_iter().map(|reference| if reference.starts_with("data:image/") { reference }
        else { format!("asb-claude-image:{}:{reference}", source.id) }).collect()
}

fn saved_image(sources: &[Transcript], reference: &str) -> Result<String, String> {
    let (id, reference) = reference.strip_prefix("asb-claude-image:").and_then(|value| value.split_once(':'))
        .ok_or("保留图片缺少来源版本，未猜测读取其他版本")?;
    let source = sources.iter().find(|source| source.id == id).ok_or("保留图片来源版本已不可用")?;
    let connection = store::read(&source.database)?;
    let asset: Option<(String,u64)> = connection.query_row("SELECT a.id,a.size FROM attachments x JOIN assets a ON a.id=x.asset
        WHERE x.source=?1 AND x.reference=?2", params![id,reference], |row| Ok((row.get(0)?,row.get(1)?)))
        .optional().map_err(db_error)?;
    let (asset,size) = asset.ok_or_else(|| format!("保留库中找不到图片附件：{reference}"))?;
    if size > 24 * 1024 * 1024 { return Err("已保留图片超过 24 MiB 文件读取上限；文本仍可阅读".into()); }
    let mut bytes = Vec::with_capacity(size as usize);
    copy_asset(&connection, &asset, &mut bytes)?;
    images::data_uri(&bytes)
}

fn io_error(error: impl std::fmt::Display) -> String { error.to_string() }
