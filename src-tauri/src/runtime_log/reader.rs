use super::{
    is_runtime_log_file, RuntimeLogEntry, RuntimeLogQuery, RuntimeLogRead, RuntimeLogSeverity,
    MAX_VISIBLE_ENTRIES, PAGE_SIZE,
};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};
use std::fs::{self, File, Metadata};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use uuid::Uuid;

const MAX_LINE_BYTES: u64 = 16 * 1024;
const CHECKPOINT_BYTES: u64 = 128;
const CACHED_FILES: usize = 5;
const CACHED_SNAPSHOTS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
struct FileIdentity(u64, u64);

#[cfg(unix)]
fn file_identity(_: &File, metadata: &Metadata) -> io::Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt;
    Ok(FileIdentity(metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
fn file_identity(file: &File, _: &Metadata) -> io::Result<FileIdentity> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // The open file owns the handle and the API initializes the buffer on success.
    let status = unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) };
    if status == 0 { return Err(io::Error::last_os_error()); }
    let information = unsafe { information.assume_init() };
    let index = (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
    Ok(FileIdentity(information.dwVolumeSerialNumber.into(), index))
}

#[derive(PartialEq, Eq)]
struct FileStamp {
    id: FileIdentity,
    len: u64,
    modified: SystemTime,
}

impl FileStamp {
    fn read(file: &File) -> io::Result<Self> {
        let metadata = file.metadata()?;
        Ok(Self { id: file_identity(file, &metadata)?, len: metadata.len(), modified: metadata.modified()? })
    }
}

#[derive(Clone)]
struct Candidate {
    entry: Arc<RuntimeLogEntry>,
    file_order: usize,
    offset: u64,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool { self.cmp(other) == Ordering::Equal }
}
impl Eq for Candidate {}
impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}
impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        other.entry.at.cmp(&self.entry.at)
            .then(self.file_order.cmp(&other.file_order))
            .then(self.offset.cmp(&other.offset))
    }
}

#[derive(Default)]
struct Latest {
    candidates: BinaryHeap<Candidate>,
    total: usize,
}

impl Latest {
    fn add(&mut self, candidate: Candidate) {
        self.total += 1;
        self.offer(candidate);
    }

    fn offer(&mut self, candidate: Candidate) {
        if self.candidates.len() < MAX_VISIBLE_ENTRIES {
            self.candidates.push(candidate);
        } else if self.candidates.peek().is_some_and(|oldest| candidate < *oldest) {
            *self.candidates.peek_mut().expect("nonempty candidates") = candidate;
        }
    }
}

fn severity_index(level: RuntimeLogSeverity) -> usize {
    match level {
        RuntimeLogSeverity::Debug => 0,
        RuntimeLogSeverity::Info => 1,
        RuntimeLogSeverity::Warn => 2,
        RuntimeLogSeverity::Error => 3,
    }
}

#[derive(PartialEq, Eq)]
struct Checkpoint {
    prefix: Vec<u8>,
    boundary: Vec<u8>,
}

impl Checkpoint {
    fn read(file: &mut File, length: u64, offset: u64) -> io::Result<Self> {
        Ok(Self {
            prefix: read_span(file, 0, length.min(CHECKPOINT_BYTES))?,
            boundary: read_span(file, offset.saturating_sub(CHECKPOINT_BYTES), offset.min(CHECKPOINT_BYTES))?,
        })
    }
}

fn read_span(file: &mut File, start: u64, length: u64) -> io::Result<Vec<u8>> {
    let mut bytes = vec![0; length as usize];
    file.seek(SeekFrom::Start(start))?;
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

enum Tail {
    Event(Candidate),
    Invalid,
}

#[derive(Default)]
struct FileContents {
    offset: u64,
    levels: [Latest; 4],
    skipped: usize,
    tail: Option<Tail>,
}

impl FileContents {
    fn read(&mut self, file: &mut File, length: u64) -> io::Result<()> {
        file.seek(SeekFrom::Start(self.offset))?;
        let mut reader = BufReader::new(file.take(length - self.offset));
        let mut line = Vec::new();
        self.tail = None;
        while let Some(record) = read_line(&mut reader, &mut line)? {
            let candidate = record.valid_length.then(|| serde_json::from_slice::<RuntimeLogEntry>(&line).ok())
                .flatten().filter(RuntimeLogEntry::is_valid)
                .map(|entry| Candidate { entry: Arc::new(entry), file_order: 0, offset: self.offset });
            if !record.complete {
                self.tail = Some(candidate.map_or(Tail::Invalid, Tail::Event));
                break;
            }
            self.offset += record.bytes;
            match candidate {
                Some(candidate) => self.levels[severity_index(candidate.entry.level)].add(candidate),
                None => self.skipped += 1,
            }
        }
        if reader.get_ref().limit() != 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Ok(())
    }

    fn merge(&self, level: Option<usize>, file_order: usize, latest: &mut Latest, skipped: &mut usize) {
        *skipped += self.skipped;
        for (index, records) in self.levels.iter().enumerate() {
            if level.is_some_and(|level| level != index) { continue; }
            latest.total += records.total;
            for candidate in &records.candidates {
                latest.offer(Candidate { file_order, ..candidate.clone() });
            }
        }
        match &self.tail {
            Some(Tail::Invalid) => *skipped += 1,
            Some(Tail::Event(candidate)) if level.is_none_or(|level| level == severity_index(candidate.entry.level)) => {
                latest.add(Candidate { file_order, ..candidate.clone() });
            }
            _ => {}
        }
    }
}

struct CachedFile {
    stamp: FileStamp,
    checkpoint: Checkpoint,
    contents: FileContents,
}

impl CachedFile {
    fn read(mut file: File, stamp: FileStamp, saved: Option<Self>) -> io::Result<Self> {
        let mut contents = FileContents::default();
        if let Some(saved) = saved {
            if saved.stamp == stamp { return Ok(saved); }
            // The application appends to the live file and renames it when rotating.
            if stamp.len > saved.stamp.len
                && Checkpoint::read(&mut file, saved.stamp.len, saved.contents.offset)? == saved.checkpoint {
                contents = saved.contents;
            }
        }
        contents.read(&mut file, stamp.len)?;
        let checkpoint = Checkpoint::read(&mut file, stamp.len, contents.offset)?;
        let after = FileStamp::read(&file)?;
        if after.len < stamp.len || (after.len == stamp.len && after != stamp) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        Ok(Self { stamp, checkpoint, contents })
    }
}

#[derive(Default)]
pub(super) struct LogReader {
    directory: PathBuf,
    files: VecDeque<CachedFile>,
    snapshots: VecDeque<LogSnapshot>,
}

struct LogSnapshot {
    id: Uuid,
    ordered: Vec<Candidate>,
    total: usize,
    skipped: usize,
}

impl LogSnapshot {
    fn page(&self, requested: usize) -> RuntimeLogRead {
        let visible = self.ordered.len();
        let page = requested.clamp(1, visible.div_ceil(PAGE_SIZE).max(1));
        let entries = self.ordered.iter().skip((page - 1) * PAGE_SIZE).take(PAGE_SIZE)
            .map(|candidate| (*candidate.entry).clone()).collect();
        RuntimeLogRead {
            snapshot_id: self.id, entries, total: self.total, skipped: self.skipped,
            visible, page, page_size: PAGE_SIZE,
        }
    }
}

impl LogReader {
    pub(super) fn read(&mut self, directory: &Path, query: RuntimeLogQuery) -> Result<RuntimeLogRead, String> {
        if directory != self.directory {
            self.directory = directory.to_owned();
            self.files.clear();
            self.snapshots.clear();
        }
        match query {
            RuntimeLogQuery::Refresh { level, page } => self.new_snapshot(directory, level.map(severity_index), page),
            RuntimeLogQuery::Page { snapshot_id, page } => self.snapshots.iter()
                .find(|snapshot| snapshot.id == snapshot_id).map(|snapshot| snapshot.page(page))
                .ok_or_else(|| "日志分页已过期，请重新刷新".into()),
        }
    }

    fn new_snapshot(&mut self, directory: &Path, level: Option<usize>, page: usize) -> Result<RuntimeLogRead, String> {
        let (latest, skipped) = self.refresh(directory, level).or_else(|error| {
            if matches!(error.kind(), io::ErrorKind::UnexpectedEof | io::ErrorKind::Interrupted) {
                self.refresh(directory, level)
            } else { Err(error) }
        }).map_err(|error| format!("应用日志不可读：{error}"))?;
        let snapshot = LogSnapshot {
            id: Uuid::new_v4(), ordered: latest.candidates.into_sorted_vec(),
            total: latest.total, skipped,
        };
        let result = snapshot.page(page);
        if self.snapshots.len() == CACHED_SNAPSHOTS { self.snapshots.pop_front(); }
        self.snapshots.push_back(snapshot);
        Ok(result)
    }

    fn refresh(&mut self, directory: &Path, level: Option<usize>) -> io::Result<(Latest, usize)> {
        let mut old = std::mem::take(&mut self.files);
        let mut latest = Latest::default();
        let mut skipped = 0;
        let files = match fs::read_dir(directory) {
            Ok(files) => files,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((latest, skipped)),
            Err(error) => return Err(error),
        };
        for (order, entry) in files.enumerate() {
            let entry = entry?;
            let path = entry.path();
            if !is_runtime_log_file(&path) || !entry.file_type()?.is_file() { continue; }
            let file = match File::open(path) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            let stamp = FileStamp::read(&file)?;
            let saved = old.iter().position(|file| file.stamp.id == stamp.id)
                .and_then(|position| old.remove(position));
            let cached = CachedFile::read(file, stamp, saved)?;
            cached.contents.merge(level, order, &mut latest, &mut skipped);
            if self.files.len() == CACHED_FILES { self.files.pop_front(); }
            self.files.push_back(cached);
        }
        Ok((latest, skipped))
    }
}

struct Line {
    bytes: u64,
    complete: bool,
    valid_length: bool,
}

fn read_line(reader: &mut impl BufRead, line: &mut Vec<u8>) -> io::Result<Option<Line>> {
    line.clear();
    let mut bytes = (&mut *reader).take(MAX_LINE_BYTES + 1).read_until(b'\n', line)? as u64;
    if bytes == 0 { return Ok(None); }
    let valid_length = bytes <= MAX_LINE_BYTES;
    let mut complete = line.last() == Some(&b'\n');
    if !valid_length {
        while !complete {
            let available = reader.fill_buf()?;
            if available.is_empty() { break; }
            let end = available.iter().position(|byte| *byte == b'\n');
            let consumed = end.map_or(available.len(), |index| index + 1);
            reader.consume(consumed);
            bytes += consumed as u64;
            complete = end.is_some();
        }
    }
    Ok(Some(Line { bytes, complete, valid_length }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_lines_are_discarded_without_consuming_the_following_record() {
        let mut input = vec![b'x'; MAX_LINE_BYTES as usize * 3];
        input.extend_from_slice(b"\n{\"valid\":true}\n");
        let mut reader = std::io::Cursor::new(input);
        let mut line = Vec::new();
        let oversized = read_line(&mut reader, &mut line).unwrap().unwrap();
        assert!(!oversized.valid_length && oversized.complete);
        assert_eq!(oversized.bytes, MAX_LINE_BYTES * 3 + 1);
        assert!(line.len() <= MAX_LINE_BYTES as usize + 1);
        assert!(read_line(&mut reader, &mut line).unwrap().unwrap().valid_length);
        assert_eq!(line, b"{\"valid\":true}\n");
        assert!(read_line(&mut reader, &mut line).unwrap().is_none());
    }
}
