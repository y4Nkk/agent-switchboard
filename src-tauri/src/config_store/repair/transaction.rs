use super::{files::file_tree, ConfigStore, RepairReport};
use crate::application_data_io as io;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, path::{Path, PathBuf}};

use crate::config_store::REPAIR_JOURNAL_FILE as JOURNAL;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Journal {
    version: u8,
    id: String,
    original: String,
    candidate: Option<String>,
    repaired_files: Vec<String>,
}

pub(super) struct Transaction {
    root: PathBuf,
    journal: Journal,
}

pub(crate) fn revision(root: &Path) -> Result<String, String> {
    let mut hash = Sha256::new();
    for (path, bytes) in file_tree(root)? {
        let path = path.to_string_lossy();
        hash.update((path.len() as u64).to_le_bytes());
        hash.update(path.as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    Ok(format!("{:x}", hash.finalize()))
}

impl Transaction {
    pub(super) fn begin(store: &ConfigStore) -> Result<Self, String> {
        let transaction = Self {
            root: store.state_root.clone(),
            journal: Journal { version: 1, id: uuid::Uuid::new_v4().simple().to_string(),
                original: revision(&store.configuration_dir())?, candidate: None, repaired_files: Vec::new() },
        };
        io::write_new(&transaction.record(), &transaction.encoded()?)?;
        #[cfg(test)]
        crate::application_preparation::tests::checkpoint("configuration-building", &transaction.root);
        Ok(transaction)
    }

    fn record(&self) -> PathBuf { self.root.join(JOURNAL) }
    pub(super) fn stage_root(&self) -> PathBuf { self.root.join(format!("repair-stage-{}", self.journal.id)) }
    pub(super) fn staged(&self) -> PathBuf { self.stage_root().join("configuration") }
    fn live(&self) -> PathBuf { self.root.join("configuration") }
    fn backup(&self) -> PathBuf { self.root.join(format!("repair-backup-{}", self.journal.id)) }
    fn encoded(&self) -> Result<Vec<u8>, String> { serde_json::to_vec(&self.journal).map_err(|error| error.to_string()) }

    pub(super) fn prepare(&mut self, repaired_files: Vec<String>) -> Result<(), String> {
        self.check_record()?;
        if revision(&self.live())? != self.journal.original {
            return Err("修复期间配置被外部修改；原数据未变".into());
        }
        self.journal.candidate = Some(revision(&self.staged())?);
        self.journal.repaired_files = repaired_files;
        io::replace(&self.record(), &self.encoded()?)?;
        #[cfg(test)]
        crate::application_preparation::tests::checkpoint("configuration-prepared", &self.root);
        Ok(())
    }

    pub(super) fn discard(&self) -> Result<(), String> {
        self.check_record()?;
        // Only generated material identified by this journal may be removed.
        file_tree(&self.stage_root())?;
        if self.stage_root().exists() {
            if self.journal.candidate.is_some() {
                fs::remove_dir(self.stage_root()).map_err(|error| format!("暂存位置出现额外材料，已保留恢复记录：{error}"))?;
            } else { fs::remove_dir_all(self.stage_root()).map_err(|error| error.to_string())?; }
        }
        fs::remove_file(self.record()).map_err(|error| error.to_string())
    }

    pub(super) fn commit(&self) -> Result<RepairReport, String> {
        self.check_record()?;
        let candidate = self.journal.candidate.as_ref().ok_or("修复副本尚未完成校验")?;
        let live = self.live();
        let backup = self.backup();
        if live.try_exists().map_err(|error| error.to_string())? {
            let current = revision(&live)?;
            if current == *candidate && backup.exists() {
                return self.finish();
            }
            if current != self.journal.original || backup.exists() {
                return Err("配置或修复备份已发生外部变化，保留全部恢复材料".into());
            }
            self.verify_stage()?;
            fs::rename(&live, &backup).map_err(|error| format!("无法保留配置原目录：{error}"))?;
            #[cfg(test)]
            crate::application_preparation::tests::checkpoint("configuration-retired", &self.root);
        }
        if revision(&backup)? != self.journal.original || !backup.is_dir() {
            return Err("原目录备份缺失或内容不一致，保留全部恢复材料".into());
        }
        self.verify_stage()?;
        if let Err(error) = fs::rename(self.staged(), &live) {
            let restored = fs::rename(&backup, &live);
            return Err(format!("无法启用修复副本：{error}；恢复原目录：{restored:?}；恢复记录保留在 {}", self.record().display()));
        }
        #[cfg(test)]
        crate::application_preparation::tests::checkpoint("configuration-enabled", &self.root);
        self.finish()
    }

    fn verify_stage(&self) -> Result<(), String> {
        if !self.staged().is_dir() || Some(revision(&self.staged())?) != self.journal.candidate {
            return Err("修复副本缺失或内容已改变，原件和恢复材料均保留".into());
        }
        let probe = ConfigStore::new(self.stage_root());
        let snapshot = super::super::snapshot::read_configuration_snapshot(&probe).map_err(|error| error.to_string())?;
        super::super::snapshot::validate_snapshot(&snapshot)
    }

    fn finish(&self) -> Result<RepairReport, String> {
        if Some(revision(&self.live())?) != self.journal.candidate
            || revision(&self.backup())? != self.journal.original || !self.backup().is_dir() {
            return Err("启用后的配置或原件备份校验不一致，保留恢复记录".into());
        }
        self.discard()?;
        Ok(RepairReport { repaired_files: self.journal.repaired_files.clone(),
            backup_path: Some(self.backup().display().to_string()) })
    }

    fn check_record(&self) -> Result<(), String> {
        let bytes = io::read_bounded(&self.record(), 64 * 1024)?.ok_or("配置恢复记录已消失，保留全部材料")?;
        let current: Journal = serde_json::from_slice(&bytes)
            .map_err(|error| super::super::error::json_error(&self.record(), &error).to_string())?;
        if current != self.journal { return Err("配置恢复记录已被修改，保留全部材料".into()); }
        Ok(())
    }
}

impl ConfigStore {
    pub(crate) fn recover_pending_repair(&self) -> Result<Option<RepairReport>, String> {
        let Some(bytes) = io::read_bounded(&self.state_root.join(JOURNAL), 64 * 1024)? else { return Ok(None); };
        if bytes.len() > 64 * 1024 { return Err("配置修复记录超过大小上限，原件已保留".into()); }
        let journal: Journal = serde_json::from_slice(&bytes)
            .map_err(|error| super::super::error::json_error(&self.state_root.join(JOURNAL), &error).to_string())?;
        let valid_id = uuid::Uuid::parse_str(&journal.id).is_ok_and(|id| id.simple().to_string() == journal.id);
        let valid_hash = |hash: &str| hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit());
        if journal.version != 1 || !valid_id || !valid_hash(&journal.original)
            || journal.candidate.as_ref().is_some_and(|hash| !valid_hash(hash))
            || journal.candidate.is_some() == journal.repaired_files.is_empty() {
            return Err("配置修复记录不是当前契约，保留原件与全部恢复材料".into());
        }
        let transaction = Transaction { root: self.state_root.clone(), journal };
        if transaction.journal.candidate.is_none() {
            if !transaction.live().is_dir() || revision(&transaction.live())? != transaction.journal.original
                || transaction.backup().exists() {
                return Err("未完成的修复准备与原目录不一致，保留全部材料".into());
            }
            transaction.discard()?;
            return Ok(None);
        }
        transaction.commit().map(Some)
    }

    pub(crate) fn data_revision(&self) -> Result<String, String> {
        let data = revision(&self.configuration_dir())?;
        let journal = io::read(&self.state_root.join(JOURNAL))?;
        Ok(format!("{data}:{}", journal.as_deref().map(io::digest).unwrap_or_default()))
    }
}

#[cfg(test)]
mod tests;
