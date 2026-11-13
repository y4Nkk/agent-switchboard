use super::{ConfigStore, StoreOperationError};
use asb_core::contracts::AppKind;
use uuid::Uuid;

impl ConfigStore {
    pub(crate) fn validate_new_provider_id(&self, id: &str) -> Result<(), StoreOperationError> {
        if Uuid::parse_str(id).ok().map(|value| value.to_string()).as_deref() != Some(id) {
            return Err("新建供应商标识必须是标准 UUID".into());
        }
        let directories = [
            self.providers_dir(AppKind::Codex),
            self.providers_dir(AppKind::Codex).join("official"),
            self.providers_dir(AppKind::Claude),
        ];
        for directory in directories {
            match std::fs::symlink_metadata(directory.join(format!("{id}.json"))) {
                Ok(_) => return Err("该供应商标识已存在，不能覆盖已有档案".into()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(format!("无法检查新建供应商标识：{error}").into()),
            }
        }
        Ok(())
    }
}
