use super::{fingerprint, message, source_key, workflow_location, Candidate};
use crate::session_manager::{layout, records, SessionMessage, SessionMeta};
use asb_core::contracts::AppKind;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

const FORMAT: &str = "agent-switchboard.converted-session";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Header {
    format: String,
    pub source_key: String,
    pub revision: String,
    source_path: PathBuf,
    pub meta: SessionMeta,
    records: usize,
}

struct Temporary(PathBuf);
impl Drop for Temporary { fn drop(&mut self) { let _ = fs::remove_file(&self.0); } }

fn directory(root: &Path) -> PathBuf { layout::directory(root).join("converted") }

fn header(path: &Path) -> Result<Header, String> {
    if path.is_symlink() { return Err("转换记录不能是符号链接".into()); }
    let mut reader = BufReader::new(File::open(path).map_err(|error| error.to_string())?);
    let mut line = Vec::new();
    (&mut reader).take(records::MAX_RECORD_BYTES + 1).read_until(b'\n', &mut line).map_err(|error| error.to_string())?;
    if line.len() as u64 > records::MAX_RECORD_BYTES { return Err("转换记录头超过读取上限".into()); }
    let header: Header = serde_json::from_slice(&line).map_err(|error| format!("转换记录头不可读: {error}"))?;
    let digest = |text: &str| text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_hexdigit());
    let parent = header.meta.parent_session_id.as_deref().ok_or("转换记录缺少所属主会话")?;
    let (original_parent, _, _) = workflow_location(&header.source_path).ok_or("转换记录来源位置无效")?;
    if header.format != FORMAT || !digest(&header.source_key) || !digest(&header.revision)
        || source_key(&header.source_path) != header.source_key || parent != original_parent.as_str()
        || path.file_stem().and_then(|name| name.to_str()) != Some(header.source_key.as_str())
        || header.meta.app != AppKind::Claude || header.meta.session_id != format!("workflow-{}", header.source_key)
        || !crate::session_manager::parser::valid_session_id(parent) || !header.meta.can_delete
        || header.meta.resume_command.as_deref() != Some(crate::session_manager::resume::resume_command(AppKind::Claude, parent).as_str())
        || header.meta.preservation.is_some()
        || header.records == 0 {
        return Err("转换记录不符合当前格式，原件已保留".into());
    }
    Ok(header)
}

pub(super) fn list(root: &Path) -> (Vec<(Header, PathBuf)>, Vec<String>) {
    let index = match crate::session_manager::read_index::ReadIndex::open(root) {
        Ok(index) => index,
        Err(error) => return (Vec::new(), vec![error]),
    };
    let directory = directory(root);
    let mut converted = Vec::new();
    let mut failures = Vec::new();
    let entries = match fs::read_dir(&directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !directory.exists() => return (converted, failures),
        Err(error) => return (converted, vec![format!("转换记录目录不可读: {error}")]),
        Ok(_) if directory.is_symlink() => return (converted, vec!["转换记录目录不能是符号链接".into()]),
        Ok(entries) => entries,
    };
    let mut paths = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) if entry.path().extension().is_some_and(|extension| extension == "jsonl") => paths.push(entry.path()),
            Ok(_) => {}
            Err(error) => failures.push(format!("转换记录条目不可读: {error}")),
        }
    }
    paths.sort();
    for path in paths {
        let read = index.file("converted-metadata", &path, || header(&path).and_then(|header| {
            validate(&path, header.records)?;
            Ok(header)
        }));
        match read {
            Ok(header) => converted.push((header, path)),
            Err(error) => failures.push(format!("{}: {error}", path.display())),
        }
    }
    (converted, failures)
}

pub(super) fn write(root: &Path, source: &Candidate, meta: SessionMeta, count: usize, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<usize, String> {
    let directory = directory(root);
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    if directory.is_symlink() { return Err("转换记录目录不能是符号链接".into()); }
    let temporary = Temporary(directory.join(format!(".{}.tmp", uuid::Uuid::new_v4())));
    let target = directory.join(format!("{}.jsonl", source.source_key));
    let mut file = File::create_new(&temporary.0).map_err(|error| error.to_string())?;
    let mut writer = BufWriter::new(&mut file);
    let header = Header { format: FORMAT.into(), source_key: source.source_key.clone(), revision: source.revision.clone(),
        source_path: source.path.clone(), meta, records: count };
    line(&mut writer, &header)?;
    let mut written = 0;
    let scan = records::scan_records_observed(&source.path, 0, 0, checkpoint, |_, _, value| {
        written += 1;
        line(&mut writer, &message(written, &value))?;
        Ok(ControlFlow::Continue(()))
    })?;
    if !scan.issues.is_empty() { return Err(scan.issues.join("；")); }
    if written != count || fingerprint(&source.path)? != source.revision {
        return Err("日志在转换期间发生变化，请稍后重试；未发布部分记录".into());
    }
    writer.flush().map_err(|error| error.to_string())?;
    drop(writer);
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    validate(&temporary.0, count)?;
    checkpoint()?;
    crate::storage::publish::replace(&temporary.0, &target)?;
    Ok(count)
}

fn line(writer: &mut impl Write, value: &impl Serialize) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, value).map_err(|error| error.to_string())?;
    writer.write_all(b"\n").map_err(|error| error.to_string())
}

pub(super) fn visit(
    path: &Path,
    mut visitor: impl FnMut(SessionMessage) -> Result<ControlFlow<()>, String>,
) -> Result<Vec<String>, String> {
    records::visit_records(path, 0, |ordinal, _, value| {
        if ordinal == 1 { return Ok(ControlFlow::Continue(())); }
        let message: SessionMessage = serde_json::from_value(value).map_err(|error| format!("转换消息不可读: {error}"))?;
        visitor(message)
    })
}

fn validate(path: &Path, expected: usize) -> Result<(), String> {
    let mut count = 0;
    let issues = visit(path, |_| { count += 1; Ok(ControlFlow::Continue(())) })?;
    if !issues.is_empty() { return Err(issues.join("；")); }
    if count != expected { return Err("转换记录不完整，未使用该副本".into()); }
    Ok(())
}
