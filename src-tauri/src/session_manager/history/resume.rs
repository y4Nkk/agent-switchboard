use super::{attachments, db_error, reading, store, Transcript};
use crate::session_manager::{parser, records, SessionSource};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

struct Pending(PathBuf);
impl Drop for Pending {
    fn drop(&mut self) {
        if self.0.is_dir() { let _ = std::fs::remove_dir_all(&self.0); }
    }
}

pub(super) fn prepare(root: &Path, source: &SessionSource) -> Result<PathBuf, String> {
    let mut connection = store::open(root)?;
    let name = uuid::Uuid::new_v4().to_string();
    connection.execute("INSERT INTO resume_packages(id,session_id) VALUES(?1,?2)", params![name,source.meta.session_id]).map_err(db_error)?;
    let transaction = connection.transaction().map_err(db_error)?;
    let sources = reading::sources(&transaction, &store::path(root))?;
    let id = &source.meta.session_id;
    if !parser::valid_session_id(id) { return Err("恢复会话身份无效".into()); }
    let published = super::packages::directory(root, &name)?;
    let directory = published.parent().ok_or("恢复目录无效")?;
    std::fs::create_dir_all(&directory).map_err(io_error)?;
    let temporary = Pending(directory.join(format!(".{name}.tmp")));
    let target = temporary.0.join("projects/restored");
    let published_target = published.join("projects/restored");
    std::fs::create_dir_all(&target).map_err(io_error)?;
    let selected: Vec<_> = sources.iter().filter(|item| &item.meta.session_id == id || item.meta.parent_session_id.as_ref() == Some(id)).collect();
    let mut assets = BTreeMap::new();
    let mut references = HashMap::new();
    for item in &selected {
        for transcript in &item.retained {
            let mut mapped = HashMap::new();
            stage_assets(&transaction, transcript, &target, &published_target, &mut assets, &mut mapped)?;
            references.insert(transcript.id.clone(), mapped);
        }
    }
    for item in selected {
        let relative = if &item.meta.session_id == id { format!("{id}.jsonl") }
            else { format!("{id}/subagents/{}.jsonl", item.retained.first().and_then(|item| item.origin.file_stem())
                .and_then(|name| name.to_str()).ok_or("子代理文件名无效")?) };
        let path = target.join(relative);
        std::fs::create_dir_all(path.parent().ok_or("恢复目录无效")?).map_err(io_error)?;
        let mut file = File::create_new(&path).map_err(io_error)?;
        {
            let mut writer = BufWriter::new(&mut file);
            write_records(&transaction, &item.retained, &references, &mut writer)?;
            writer.flush().map_err(io_error)?;
        }
        file.sync_all().map_err(io_error)?;
    }
    if !target.join(format!("{id}.jsonl")).is_file() { return Err("已保留的主会话正文不可用，未启动客户端".into()); }
    crate::storage::publish::move_new(&temporary.0, &published)?;
    Ok(published_target.join(format!("{id}.jsonl")))
}

fn stage_assets(connection: &Connection, transcript: &Transcript, target: &Path, published: &Path,
    assets: &mut BTreeMap<String,String>, references: &mut HashMap<String,String>,
) -> Result<(), String> {
    let mut statement = connection.prepare("SELECT reference,relative_path,asset FROM attachments WHERE source=?1 ORDER BY relative_path")
        .map_err(db_error)?;
    let mut rows = statement.query([&transcript.id]).map_err(db_error)?;
    while let Some(row) = rows.next().map_err(db_error)? {
        let reference: String = row.get(0).map_err(db_error)?;
        let relative: String = row.get(1).map_err(db_error)?;
        let relative = if let Some(parent) = &transcript.parent { format!("{parent}/subagents/{relative}") } else { relative };
        let asset: String = row.get(2).map_err(db_error)?;
        attachments::validate_relative(&relative)?;
        if assets.get(&relative).is_some_and(|current| current != &asset) {
            return Err(format!("同一会话的不同来源包含冲突附件，未猜测覆盖：{relative}"));
        }
        if !assets.contains_key(&relative) {
            let path = target.join(&relative);
            std::fs::create_dir_all(path.parent().ok_or("附件恢复目录无效")?).map_err(io_error)?;
            let mut file = File::create_new(path).map_err(io_error)?;
            attachments::copy_asset(connection, &asset, &mut file)?;
            file.sync_all().map_err(io_error)?;
            assets.insert(relative.clone(), asset);
        }
        references.insert(reference, published.join(relative).to_string_lossy().into_owned());
    }
    Ok(())
}

struct Entry { source: String, position: u64, digest: String, at: Option<String> }

fn write_records(connection: &Connection, sources: &[Transcript], references: &HashMap<String,HashMap<String,String>>, writer: &mut impl Write) -> Result<(), String> {
    let mut seen = HashMap::<String,usize>::new();
    let mut entries = Vec::new();
    for source in sources {
        let mut occurrences = HashMap::<String,usize>::new();
        let mut at = None;
        super::visit_records_observed(source, 0, &|| Ok(()), |_, position, value| {
            let digest = parser::record_identity(&value);
            let key = parser::claude_record_id(&value).unwrap_or_else(|| digest.clone());
            let count = occurrences.entry(key.clone()).or_default();
            *count += 1;
            if let Some(time) = parser::timestamp_from(&value) { at = Some(time); }
            if seen.get(&key).is_none_or(|current| *current < *count) {
                seen.insert(key, *count);
                entries.push(Entry { source: source.id.clone(), position, digest, at: at.clone() });
            }
            Ok(ControlFlow::Continue(()))
        })?;
    }
    entries.sort_by(|left, right| left.at.cmp(&right.at));
    let mut statement = connection.prepare("SELECT raw FROM records WHERE source=?1 AND position=?2").map_err(db_error)?;
    for entry in entries {
        let bytes: Vec<u8> = statement.query_row(params![entry.source,entry.position], |row| row.get(0)).map_err(db_error)?;
        if bytes.len() as u64 > records::MAX_RECORD_BYTES { return Err("恢复记录超过单条读取上限".into()); }
        let mut value: Value = serde_json::from_slice(&bytes).map_err(io_error)?;
        if parser::record_identity(&value) != entry.digest { return Err("会话在恢复期间已改变，请重试".into()); }
        rewrite(&mut value, references.get(&entry.source).ok_or("恢复附件映射缺失")?);
        serde_json::to_writer(&mut *writer, &value).map_err(io_error)?;
        writer.write_all(b"\n").map_err(io_error)?;
    }
    Ok(())
}

fn rewrite(value: &mut Value, references: &HashMap<String,String>) {
    match value {
        Value::String(text) => {
            if let Some(path) = references.get(text) { *text = path.clone(); }
            else {
                let mut paths: Vec<_> = references.iter().filter(|(path, _)| Path::new(path.as_str()).is_absolute()).collect();
                paths.sort_by_key(|(path, _)| std::cmp::Reverse(path.len()));
                for (reference, path) in paths { *text = text.replace(reference.as_str(), path.as_str()); }
            }
        }
        Value::Array(values) => { for value in values { rewrite(value, references); } }
        Value::Object(values) => { for value in values.values_mut() { rewrite(value, references); } }
        _ => {}
    }
}

fn io_error(error: impl std::fmt::Display) -> String { error.to_string() }
