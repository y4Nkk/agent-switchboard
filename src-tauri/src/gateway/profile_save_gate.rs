use super::GatewayInner;

impl GatewayInner {
    pub(crate) fn ensure_target_available(&self, target: &str) -> Result<(), String> {
        let store = crate::config_store::ConfigStore::new(self.state_root.clone());
        if store.pending_profile_save().map_err(|e| e.to_string())?
            .is_some_and(|pending| pending.profile_id == target) {
            return Err("目标供应商保存事务尚未完成，请稍后重试；若持续失败请恢复未完成事务".into());
        }
        Ok(())
    }
}
