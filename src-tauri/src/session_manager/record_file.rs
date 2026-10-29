use super::{read_index::FileRevision, records};
use serde_json::Value;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

pub(crate) fn compressed(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(".jsonl.zst"))
}

pub(crate) fn transcript(path: &Path) -> bool {
    compressed(path) || path.extension().is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
}

pub(crate) fn decode<'a>(path: &Path, mut raw: impl Read + 'a) -> Result<Box<dyn Read + 'a>, String> {
    if !compressed(path) { return Ok(Box::new(raw)); }
    let mut header = [0; 4];
    raw.read_exact(&mut header).map_err(|error| format!("归档文件头不完整 {}: {error}", path.display()))?;
    if header != [0x28, 0xb5, 0x2f, 0xfd] {
        return Err(format!("归档 {} 缺少 Zstandard 文件头", path.display()));
    }
    let raw = std::io::Cursor::new(header).chain(raw);
    zstd::stream::read::Decoder::new(raw).map(|reader| Box::new(reader) as Box<dyn Read + 'a>)
        .map_err(|error| format!("无法解压归档 {}: {error}", path.display()))
}

pub(crate) fn open(path: &Path, offset: u64, checkpoint: &dyn Fn() -> Result<(), String>)
    -> Result<BufReader<Box<dyn Read>>, String> {
    checkpoint()?;
    let mut file = File::open(path).map_err(|error| format!("无法读取会话记录 {}: {error}", path.display()))?;
    let length = file.metadata().map_err(|error| error.to_string())?.len();
    let physical_offset = if compressed(path) { 0 } else { offset };
    file.seek(SeekFrom::Start(physical_offset)).map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(decode(path, file.take(length.saturating_sub(physical_offset)))?);
    if compressed(path) { skip(&mut reader, offset, checkpoint)?; }
    Ok(reader)
}

fn skip(reader: &mut impl Read, mut length: u64, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<(), String> {
    let mut buffer = [0; 64 * 1024];
    while length > 0 {
        checkpoint()?;
        let size = length.min(buffer.len() as u64) as usize;
        reader.read_exact(&mut buffer[..size]).map_err(|error| format!("会话记录位置已失效：{error}"))?;
        length -= size as u64;
    }
    Ok(())
}

/// One request reuses its decoder; derived indexes never own another full transcript.
pub(super) struct Cursor {
    path: PathBuf,
    revision: FileRevision,
    reader: BufReader<Box<dyn Read>>,
    position: u64,
}

impl Cursor {
    pub(super) fn new(path: &Path, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<Self, String> {
        let revision = FileRevision::read(path)?;
        let reader = open(path, 0, checkpoint)?;
        if FileRevision::read(path)? != revision { return Err("归档在打开期间变化，稍后重新读取".into()); }
        Ok(Self { path: path.into(), revision, reader, position: 0 })
    }

    pub(super) fn value(&mut self, offset: u64, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<Value, String> {
        if FileRevision::read(&self.path)? != self.revision { return Err("归档已经变化，正在重新建立读取索引".into()); }
        if offset < self.position { *self = Self::new(&self.path, checkpoint)?; }
        skip(&mut self.reader, offset - self.position, checkpoint)?;
        let mut found = None;
        let scan = records::scan_reader(&self.path.display().to_string(), offset, 0, &mut self.reader, checkpoint,
            |_, position, value| {
                if position == offset { found = Some(value); }
                Ok(ControlFlow::Break(()))
            })?;
        self.position = scan.next_offset;
        found.ok_or_else(|| if scan.issues.is_empty() { "原会话记录已经改变".into() } else { scan.issues.join("；") })
    }
}
