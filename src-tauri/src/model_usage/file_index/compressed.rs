use super::*;
use crate::session_manager::record_file;

pub(super) fn capture(path: &Path, app: AppKind, session: &str, revision: FileRevision)
    -> Result<(FileUsage, bool), String> {
    let mut state = UsageState::new(app);
    let mut issues = Vec::new();
    let mut hash = Sha256::new();
    let mut physical = DigestReader::open(path, 0, revision.bytes, &mut hash)?;
    let scan = {
        let decoded = record_file::decode(path, &mut physical)?;
        records::scan_reader(&path.display().to_string(), 0, 0, BufReader::new(decoded), &|| Ok(()),
            |ordinal, _, value| {
                if let Some(issue) = state.observe(&value, session)? {
                    issues.push(format!("{} 第 {ordinal} 条: {issue}", path.display()));
                }
                Ok(ControlFlow::Continue(()))
            })?
    };
    if physical.bytes != revision.bytes { return Err(format!("归档未完整读取：{}", path.display())); }
    drop(physical);
    issues.extend(scan.issues);
    Ok((FileUsage { revision, offset: scan.next_offset, ordinal: scan.ordinal,
        prefix: format!("{:x}", hash.finalize()), appendable: false, state, issues }, true))
}
