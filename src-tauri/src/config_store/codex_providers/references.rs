use super::{load, ConfigStore, CodexProviderFile, StoreOperationError};

impl ConfigStore {
    /// References resolve against the complete candidate, including a new
    /// provider that has not been written to disk yet.
    pub(crate) fn validate_codex_provider_references(
        &self,
        candidate: &CodexProviderFile,
    ) -> Result<(), StoreOperationError> {
        let stored = load(self)?;
        if let Some(route) = &candidate.profile.subagent_route {
            let target = if route.profile_id == candidate.profile.id {
                candidate
            } else {
                &stored.iter().find(|entry| entry.file.profile.id == route.profile_id)
                    .ok_or_else(|| format!(
                        "子代理路由引用的 Codex 供应商不存在：{}（模型 {}）",
                        route.profile_id, route.model,
                    ))?.file
            };
            crate::gateway::validate_subagent_target(route, target)?;
        }
        for entry in &stored {
            if entry.file.profile.id == candidate.profile.id { continue; }
            if let Some(route) = entry.file.profile.subagent_route.as_ref()
                .filter(|route| route.profile_id == candidate.profile.id)
            {
                crate::gateway::validate_subagent_target(route, candidate)?;
            }
        }
        Ok(())
    }
}
