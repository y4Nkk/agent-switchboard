use super::{AppSettings, LocalState};
use crate::application_data_io as io;
use serde::{Deserialize, Serialize};
use std::fs;

const RECORD: &str = "settings-recovery.json";

pub(crate) struct SettingsPreparation {
    pub settings: AppSettings,
    pub backup_path: Option<String>,
    pub reset: bool,
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Recovery {
    version: u8,
    id: String,
    original: String,
    candidate: AppSettings,
    reset: bool,
}

impl LocalState {
    pub(crate) fn prepare_app_settings(&self) -> Result<SettingsPreparation, String> {
        if let Some(bytes) = io::read_bounded(&self.root.join(RECORD), 256 * 1024)? {
            if bytes.len() > 256 * 1024 { return Err("偏好恢复记录超过大小上限，原件已保留".into()); }
            let record: Recovery = serde_json::from_slice(&bytes).map_err(|error| format!("偏好恢复记录无效：{error}"))?;
            return self.finish_settings_recovery(record);
        }
        let Some(original) = io::read_bounded(&self.settings_path(), 256 * 1024)? else {
            return Ok(SettingsPreparation { settings: AppSettings::default(), backup_path: None, reset: false });
        };
        if original.len() > 256 * 1024 { return Err("应用偏好超过大小上限，原件已保留".into()); }
        if let Ok(settings) = self.get_app_settings() {
            return Ok(SettingsPreparation { settings, backup_path: None, reset: false });
        }
        let (candidate, reset) = super::settings_upgrade::candidate(&original)?;
        let record = Recovery { version: 1, id: uuid::Uuid::new_v4().simple().to_string(),
            original: io::digest(&original), candidate, reset };
        let bytes = serde_json::to_vec(&record).map_err(|error| error.to_string())?;
        io::write_new(&self.root.join(RECORD), &bytes)?;
        #[cfg(test)]
        crate::application_preparation::tests::checkpoint("preferences-recorded", &self.root);
        self.finish_settings_recovery(record)
    }

    fn finish_settings_recovery(&self, record: Recovery) -> Result<SettingsPreparation, String> {
        let valid_id = uuid::Uuid::parse_str(&record.id).is_ok_and(|id| id.simple().to_string() == record.id);
        if record.version != 1 || !valid_id || record.original.len() != 64
            || !record.original.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("偏好恢复记录不是当前契约，保留全部原件".into());
        }
        record.candidate.validate().map_err(|error| error.to_string())?;
        let candidate = serde_json::to_vec_pretty(&record.candidate).map_err(|error| error.to_string())?;
        let backup = self.root.join(format!("settings-backup-{}.json", record.id));
        let staged = self.root.join(format!("settings-prepared-{}.json", record.id));
        let current = io::read(&self.settings_path())?.ok_or("偏好原文件缺失，保留恢复材料")?;
        if current != candidate {
            if io::digest(&current) != record.original {
                return Err("偏好在恢复期间被外部修改，原件和恢复记录已保留".into());
            }
            io::preserve(&backup, &current)?;
            #[cfg(test)]
            crate::application_preparation::tests::checkpoint("preferences-backed-up", &self.root);
            io::preserve(&staged, &candidate)?;
            #[cfg(test)]
            crate::application_preparation::tests::checkpoint("preferences-prepared", &self.root);
            if io::read(&self.settings_path())?.as_deref() != Some(current.as_slice()) {
                return Err("偏好在提交前发生变化，未覆盖外部修改".into());
            }
            fs::rename(&staged, self.settings_path()).map_err(|error| format!("偏好启用失败，原件与备份已保留：{error}"))?;
        }
        #[cfg(test)]
        crate::application_preparation::tests::checkpoint("preferences-enabled", &self.root);
        if io::read(&self.settings_path())?.as_deref() != Some(candidate.as_slice())
            || io::read(&backup)?.as_deref().map(io::digest).as_deref() != Some(record.original.as_str()) {
            return Err("偏好恢复回读或备份校验失败，保留恢复记录".into());
        }
        if let Some(bytes) = io::read(&staged)? {
            if bytes != candidate { return Err("偏好暂存内容已变化，保留恢复记录与全部材料".into()); }
            fs::remove_file(&staged).map_err(|error| error.to_string())?;
        }
        let current: Recovery = serde_json::from_slice(&io::read_bounded(&self.root.join(RECORD), 256 * 1024)?
            .ok_or("偏好恢复记录已消失，保留全部材料")?).map_err(|error| error.to_string())?;
        if current != record { return Err("偏好恢复记录已变化，保留全部材料".into()); }
        fs::remove_file(self.root.join(RECORD)).map_err(|error| error.to_string())?;
        Ok(SettingsPreparation { settings: record.candidate,
            backup_path: Some(backup.display().to_string()), reset: record.reset })
    }
}

#[cfg(test)]
mod tests;
