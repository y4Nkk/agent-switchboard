use super::{ArchiveOutcome, ArchiveScope};
use crate::local_state::LocalState;
use asb_core::contracts::AppKind;
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use flate2::{write::GzEncoder, Compression};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ArchiveDigest { pub records: u64, pub revision: String }

pub(crate) struct ArchiveWriter<'a> {
    output: &'a mut dyn Write,
    hash: Sha256,
    records: u64,
}

impl<'a> ArchiveWriter<'a> {
    pub(crate) fn new(output: &'a mut dyn Write, context: &str) -> Self {
        let mut hash = Sha256::new();
        hash.update(context.as_bytes());
        Self { output, hash, records: 0 }
    }
    pub(crate) fn record(&mut self, kind: &str, record: &impl Serialize) -> Result<(), String> {
        #[derive(Serialize)]
        struct Entry<'a, T> { kind: &'a str, record: &'a T }
        let bytes = serde_json::to_vec(&Entry { kind, record }).map_err(|error| error.to_string())?;
        self.output.write_all(&bytes).and_then(|()| self.output.write_all(b"\n")).map_err(|error| error.to_string())?;
        self.hash.update(&bytes); self.hash.update(b"\n"); self.records += 1;
        Ok(())
    }
    pub(crate) fn finish(self) -> ArchiveDigest {
        ArchiveDigest { records: self.records, revision: format!("{:x}", self.hash.finalize()) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct HistoryArchivePlan {
    pub cutoff: String,
    pub configuration: BTreeMap<AppKind, ArchiveDigest>,
}

pub(crate) fn cutoff_date(value: &str) -> Result<DateTime<Utc>, String> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "请选择有效的归档日期")?;
    if date.to_string() != value { return Err("归档日期格式无效".into()); }
    let today = Utc::now().date_naive();
    let month = today.with_day(1).ok_or("无法确定当前计费月份")?;
    if date > month { return Err("当前 UTC 计费月份的记录必须保留，归档日期不能晚于本月首日".into()); }
    Ok(date.and_hms_opt(0, 0, 0).ok_or("归档日期无效")?.and_utc())
}

pub(crate) fn plan(local: &LocalState, cutoff: &str, output: &mut dyn Write) -> Result<HistoryArchivePlan, String> {
    let date = cutoff_date(cutoff)?;
    let store = local.configuration();
    let mut configuration = BTreeMap::new();
    for app in [AppKind::Codex, AppKind::Claude] {
        configuration.insert(app, store.archive_history_digest(app, date, output)?);
    }
    Ok(HistoryArchivePlan { cutoff: cutoff.into(), configuration })
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
}

pub(crate) fn execute(local: &LocalState, expected: &HistoryArchivePlan, target: &Path) -> Result<ArchiveOutcome, String> {
    if plan(local, &expected.cutoff, &mut std::io::sink())? != *expected {
        return Err("待归档历史已经变化，请重新预览".into());
    }
    let parent = target.parent().ok_or("归档位置无效")?;
    let temporary = Temporary(parent.join(format!(".asb-history-{}.tmp", uuid::Uuid::new_v4())));
    let mut file = File::create_new(&temporary.0).map_err(|error| error.to_string())?;
    {
        let writer = BufWriter::new(&mut file);
        let mut archive = GzEncoder::new(writer, Compression::default());
        serde_json::to_writer(&mut archive, &serde_json::json!({"format":"agent-switchboard-history","version":1,"plan":expected}))
            .map_err(|error| error.to_string())?;
        archive.write_all(b"\n").map_err(|error| error.to_string())?;
        if plan(local, &expected.cutoff, &mut archive)? != *expected {
            return Err("导出期间待归档历史已经变化，原历史未清理；请重新预览".into());
        }
        archive.finish().and_then(|mut writer| writer.flush()).map_err(|error| error.to_string())?;
    }
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    super::publish::replace(&temporary.0, target)?;
    let date = cutoff_date(&expected.cutoff)?;
    let mut result = ArchiveOutcome { path: target.to_string_lossy().into_owned(), removed: 0, warnings: Vec::new() };
    for app in [AppKind::Codex, AppKind::Claude] {
        result.add(ArchiveScope::Configuration(app), local.configuration().prune_archived_history(app, date, &expected.configuration[&app]));
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
