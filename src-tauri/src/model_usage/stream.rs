use super::{ingest::{Cursor, Delta}, progress::Progress};
use crate::session_manager::{read_index::{prefix, FileRevision}, record_file, records};
use asb_core::contracts::{AppKind, ModelUsagePhase};
use sha2::{Digest, Sha256};
use std::io::{BufReader, Read};
use std::ops::ControlFlow;
use std::path::Path;

struct Observed<'a, 'b, R> { inner: R, progress: &'a Progress<'b> }
impl<R: Read> Read for Observed<'_, '_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let count = self.inner.read(buffer)?;
        self.progress.advance_bytes(count as u64);
        Ok(count)
    }
}

pub(super) fn read(path: &Path, app: AppKind, session: &str, previous: Option<Cursor>,
    progress: &Progress<'_>) -> Result<Delta, String> {
    let before = FileRevision::read(path)?;
    let result = capture(path, app, session, previous.clone(), progress);
    if matches!(&result, Err(prefix::SnapshotError::Changed(_)))
        || result.is_err() && FileRevision::read(path).is_ok_and(|after| after != before) {
        return capture(path, app, session, previous, progress).map_err(String::from);
    }
    result.map_err(String::from)
}

fn capture(path: &Path, app: AppKind, session: &str, previous: Option<Cursor>,
    progress: &Progress<'_>) -> Result<Delta, prefix::SnapshotError> {
    let revision = FileRevision::read(path)?;
    let compressed = record_file::compressed(path);
    let (mut delta, mut hash) = resume(path, &revision, previous, progress)?;
    let start = if compressed { 0 } else { delta.cursor.offset };
    progress.bytes(ModelUsagePhase::Reading, revision.bytes - start);
    let mut physical = prefix::DigestReader::open(path, start, revision.bytes - start, &mut hash)?;
    let scan = {
        let observed = Observed { inner: &mut physical, progress };
        let decoded = record_file::decode(path, observed)?;
        records::scan_reader(&path.display().to_string(), start, delta.cursor.ordinal,
            BufReader::new(decoded), &|| Ok(()), |ordinal, _, value| {
                delta.observe(ordinal, app, session, &value)?;
                Ok(ControlFlow::Continue(()))
            })?
    };
    if physical.bytes != revision.bytes - start {
        return Err(prefix::SnapshotError::Changed(format!("会话未完整读取：{}", path.display())));
    }
    drop(physical);
    delta.cursor.prefix = format!("{:x}", hash.finalize());
    delta.cursor.offset = scan.next_offset;
    delta.cursor.ordinal = scan.ordinal;
    delta.cursor.appendable = scan.appendable && !compressed;
    delta.cursor.issues.extend(scan.issues);
    let length = if compressed { revision.bytes } else { delta.cursor.offset };
    progress.bytes(ModelUsagePhase::Verifying, length);
    prefix::verify_observed(path, &revision, length, &delta.cursor.prefix, &|| Ok(()),
        &|bytes| progress.advance_bytes(bytes))?;
    if compressed && FileRevision::read(path)? != revision {
        return Err(prefix::SnapshotError::Changed("归档在读取期间变化，稍后重试".into()));
    }
    delta.cursor.revision = Some(revision);
    Ok(delta)
}

fn resume(path: &Path, revision: &FileRevision, previous: Option<Cursor>,
    progress: &Progress<'_>) -> Result<(Delta, Sha256), String> {
    if let Some(previous) = previous.filter(|cursor| !record_file::compressed(path) && cursor.appendable
        && cursor.offset <= revision.bytes && cursor.revision.as_ref().is_some_and(|old| old.created == revision.created)) {
        let mut hash = Sha256::new();
        progress.bytes(ModelUsagePhase::Verifying, previous.offset);
        prefix::extend_hash_observed(path, 0, previous.offset, &mut hash, &|| Ok(()),
            &|bytes| progress.advance_bytes(bytes))?;
        if format!("{:x}", hash.clone().finalize()) == previous.prefix {
            return Ok((Delta::new(Some(previous)), hash));
        }
    }
    Ok((Delta::new(None), Sha256::new()))
}
