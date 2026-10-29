//! Export verified backup families before reclaiming their local copies.

mod archive;
mod families;

use super::{archive::ArchiveDigest, publish, ArchiveOutcome};
use crate::local_state::LocalState;
use asb_core::AppKind;
use asb_switch::{pending_config_write, FsIo};
use chrono::{DateTime, NaiveDate, Utc};
use families::Family;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BackupCleanupPlan {
    pub cutoff: String,
    pub files: u64,
    pub bytes: u64,
    pub digest: ArchiveDigest,
}

fn cutoff_time(value: &str) -> Result<DateTime<Utc>, String> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "请选择有效的清理日期")?;
    if date.to_string() != value { return Err("清理日期格式无效".into()); }
    if date >= Utc::now().date_naive() { return Err("清理日期必须是今天之前的日期".into()); }
    Ok(date.and_hms_opt(0, 0, 0).ok_or("清理日期无效")?.and_utc())
}

fn require_no_pending(local: &LocalState) -> Result<(), String> {
    for app in [AppKind::Codex, AppKind::Claude] {
        if !matches!(pending_config_write(&FsIo, &local.backup_dir(), app), Ok(None)) {
            return Err("存在未完成的配置事务，请先在备份页恢复或处理，再清理备份".into());
        }
    }
    Ok(())
}

struct Selection {
    families: Vec<Family>,
    entries: Vec<archive::Material>,
    plan: BackupCleanupPlan,
}

fn select(local: &LocalState, cutoff: DateTime<Utc>) -> Result<Selection, String> {
    require_no_pending(local)?;
    let mut families = families::collect(local)?;
    let mut newest: BTreeMap<AppKind, DateTime<Utc>> = BTreeMap::new();
    for family in families.iter().filter(|family| family.config) {
        let app = family.app.ok_or("备份记录缺少客户端")?;
        let entry = newest.entry(app).or_insert(family.stamp);
        *entry = (*entry).max(family.stamp);
    }
    families.retain(|family| {
        let protected = family.config
            && family.app.is_some_and(|app| newest.get(&app) == Some(&family.stamp));
        !protected && family.stamp < cutoff
    });
    let entries = archive::inventory(&local.backup_dir(), &families)?;
    let mut hash = Sha256::new();
    for entry in &entries {
        hash.update(serde_json::to_vec(&entry.descriptor()).map_err(|error| error.to_string())?);
        hash.update(b"\n");
    }
    let plan = BackupCleanupPlan {
        cutoff: cutoff.format("%Y-%m-%d").to_string(),
        files: entries.iter().filter(|entry| !entry.directory).count() as u64,
        bytes: entries.iter().map(|entry| entry.bytes).fold(0_u64, u64::saturating_add),
        digest: ArchiveDigest {
            records: families.len() as u64,
            revision: format!("{:x}", hash.finalize()),
        },
    };
    Ok(Selection { families, entries, plan })
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
}

pub(crate) fn preview(local: &LocalState, cutoff: &str) -> Result<BackupCleanupPlan, String> {
    select(local, cutoff_time(cutoff)?).map(|selection| selection.plan)
}

pub(crate) fn execute(
    local: &LocalState, expected: &BackupCleanupPlan, target: &Path,
) -> Result<ArchiveOutcome, String> {
    let cutoff = cutoff_time(&expected.cutoff)?;
    let selected = select(local, cutoff)?;
    if selected.plan != *expected { return Err("待清理备份已经变化，请重新预览".into()); }
    let parent = target.parent().ok_or("归档位置无效")?;
    let temporary = Temporary(parent.join(format!(".asb-backups-{}.tmp", uuid::Uuid::new_v4())));
    archive::write(&temporary.0, &selected.entries, expected)?;
    archive::verify(&temporary.0, &selected.entries, expected)?;
    if select(local, cutoff)?.plan != selected.plan {
        return Err("导出期间待清理备份已经变化，原备份未清理；请重新预览".into());
    }
    publish::replace(&temporary.0, target)?;
    let mut result = ArchiveOutcome {
        path: target.to_string_lossy().into_owned(), removed: 0, warnings: Vec::new(),
    };
    for family in &selected.families {
        let failed = remove_family(family);
        if failed.is_empty() { result.removed += 1; }
        else {
            let detail = failed.join("; ");
            result.warnings.push(asb_core::contracts::LocalizedMessage::new(
                "backup.cleanup.failed", serde_json::json!({"detail":detail}),
                format!("部分备份回收未完成：{detail}"),
            ));
        }
    }
    Ok(result)
}

fn remove_family(family: &Family) -> Vec<String> {
    let mut failed = Vec::new();
    for path in &family.paths {
        let outcome = if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) };
        match outcome {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => failed.push(format!("{}：{error}", path.display())),
        }
    }
    failed
}

#[cfg(test)]
mod tests;
