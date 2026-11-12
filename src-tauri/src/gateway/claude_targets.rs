use super::*;
use asb_core::contracts::{ClaudeModelRole, ClaudeModelRoute};

pub(crate) fn settings(profile: &ProviderProfile) -> Option<&ClaudeModelSettings> {
    match &profile.model_options { Some(ModelOptions::Claude(settings)) => Some(settings), _ => None }
}

pub(crate) fn references(profile: &ProviderProfile, target: &str) -> bool {
    ClaudeModelRole::ALL.into_iter().any(|role| settings(profile).and_then(|s| s.route(role))
        .and_then(ClaudeModelRoute::profile_id) == Some(target))
}

fn target_revision(target: &ProviderProfile) -> Result<String, String> {
    let facts = (route_fingerprint(target)?, &target.parameters, &target.claude_fragment);
    serde_json::to_vec(&facts).map(|bytes| hex_digest(&bytes)).map_err(|e| e.to_string())
}

fn target_preferences(target: &ProviderProfile, model: &str) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    use asb_core::contracts::SettingValue;
    let mut preferences = target.claude_fragment.get("modelSettings").and_then(|settings| settings.get(model))
        .and_then(serde_json::Value::as_object).cloned().unwrap_or_default();
    if let Some(effort) = target.claude_fragment.get("effortLevel") {
        preferences.entry("effortLevel").or_insert_with(|| effort.clone());
    }
    // Within one settings source a per-model ceiling replaces its global ceiling.
    if let Some(SettingValue::Explicit { value }) = target.parameters.value("maxEffortLevel") {
        preferences.entry("maxEffortLevel").or_insert(serde_json::to_value(value).map_err(|e| e.to_string())?);
    }
    let mut check = asb_core::claude_common::Extra::new();
    check.insert("modelSettings".into(), serde_json::json!({model: preferences}));
    asb_core::claude_common::validate(&check)?;
    Ok(preferences)
}

pub(super) fn validate_candidate_roles(candidates: &[ActiveRoute]) -> Result<(), String> {
    let Some(owner) = candidates.first() else { return Ok(()) };
    for role in ClaudeModelRole::ALL {
        if owner.claude_model_options.as_ref().and_then(|s| s.effective_route(role))
            .is_some_and(|route| route.profile_id().is_some()) { continue; }
        for candidate in candidates.iter().skip(1).filter(|candidate| candidate.profile_id != owner.profile_id) {
            if candidate.claude_model_options.as_ref().and_then(|s| s.effective_route(role))
                .is_some_and(|route| route.profile_id().is_some()) {
                return Err(format!("Claude 故障转移候选 {} 的 {} 用途指向其他供应商，不能作为主连接的同用途候选", candidate.profile_id, role.name()));
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_target(target: &ProviderProfile, model: &str) -> Result<(), String> {
    if target.app != AppKind::Claude || target.route_mode != RouteMode::Custom
        || target.connection.claude_native.is_some() || target.connection.auth_binding.is_some() {
        return Err(format!("供应商「{}」不能作为 Claude 用途路由目标", target.name));
    }
    let options = settings(target);
    let declared = target.model.as_deref() == Some(model)
        || options.and_then(|s| s.available_models.as_ref()).is_some_and(|list| list.iter().any(|id| id == model))
        || ClaudeModelRole::ALL.into_iter().any(|role| options.and_then(|s| s.route(role))
            .is_some_and(|route| route.model() == model && route.profile_id().is_none_or(|id| id == target.id)));
    if !declared { return Err(format!("模型 {model} 不在目标供应商「{}」的已保存模型中", target.name)); }
    Ok(())
}

pub(crate) fn validate_references(
    store: &crate::config_store::ConfigStore, candidate: &ProviderProfile,
) -> Result<(), String> {
    let mut profiles = store.list_providers().map_err(|e| e.to_string())?
        .into_iter().map(|record| record.profile).collect::<Vec<_>>();
    profiles.retain(|profile| profile.id != candidate.id);
    profiles.push(candidate.clone());
    for owner in &profiles {
        for role in ClaudeModelRole::ALL {
            if let Some(ClaudeModelRoute::Provider { profile_id, model, .. }) = settings(owner).and_then(|s| s.route(role)) {
                if owner.id != candidate.id && profile_id != &candidate.id { continue; }
                let target = profiles.iter().find(|profile| &profile.id == profile_id)
                    .ok_or_else(|| format!("供应商「{}」引用的 Claude 目标 {profile_id} 已不存在", owner.name))?;
                validate_target(target, model)?;
            }
        }
    }
    Ok(())
}

impl GatewayController {
    pub(super) fn claude_dependency_revision(
        &self, local: &LocalState, owner: &ProviderProfile, replacement: Option<&ProviderProfile>,
    ) -> Result<String, String> {
        let mut revisions = Vec::new();
        for role in ClaudeModelRole::ALL {
            if let Some(ClaudeModelRoute::Provider { profile_id, model, .. }) = settings(owner).and_then(|s| s.route(role)) {
                let target = if profile_id == &owner.id { owner.clone() }
                    else if let Some(profile) = replacement.filter(|profile| &profile.id == profile_id) { profile.clone() }
                    else { local.configuration().find_provider(profile_id).map_err(|e| e.to_string())? };
                validate_target(&target, model)?;
                revisions.push((profile_id, target_revision(&target)?));
            }
        }
        Ok(hex_digest(serde_json::to_string(&revisions).map_err(|e| e.to_string())?.as_bytes()))
    }

    pub(super) fn project_claude_target_preferences(
        &self, local: &LocalState, plan: &mut SwitchPlan, replacement: Option<&ProviderProfile>,
    ) -> Result<(), String> {
        let mut preferences = serde_json::Map::new();
        for role in ClaudeModelRole::ALL {
            if let Some(ClaudeModelRoute::Provider { profile_id, model, .. }) = settings(&plan.profile).and_then(|s| s.effective_route(role)) {
                let target = if profile_id == &plan.profile.id { plan.profile.clone() }
                    else if let Some(profile) = replacement.filter(|profile| &profile.id == profile_id) { profile.clone() }
                    else { local.configuration().find_provider(profile_id).map_err(|e| e.to_string())? };
                let value = target_preferences(&target, model)?;
                if !value.is_empty() {
                    preferences.insert(role.wire().into(), serde_json::Value::Object(value));
                }
            }
        }
        if !preferences.is_empty() {
            let model_settings = plan.profile.claude_fragment.entry("modelSettings")
                .or_insert_with(|| serde_json::json!({})).as_object_mut().ok_or("modelSettings 必须是对象")?;
            model_settings.extend(preferences);
        }
        Ok(())
    }

    pub(crate) fn resolve_claude_target(
        &self, candidates: Vec<ActiveRoute>, body: Vec<u8>,
    ) -> Result<(Vec<ActiveRoute>, Vec<u8>, bool), String> {
        let _targets = self.inner.profile_save_gate.read().map_err(|_| "供应商读取锁不可用")?;
        let mut value: serde_json::Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
        let model = value.get("model").and_then(serde_json::Value::as_str).unwrap_or("");
        let role_name = model.strip_suffix("[1m]").or_else(|| model.strip_suffix("[1M]")).unwrap_or(model);
        let Some(role) = ClaudeModelRole::from_wire(role_name) else {
            return Ok((candidates, body, false));
        };
        let Some(ClaudeModelRoute::Provider { profile_id, model, .. }) = candidates.first()
            .and_then(|route| route.claude_model_options.as_ref()).and_then(|s| s.effective_route(role)) else {
            return Ok((candidates, body, false));
        };
        let store = crate::config_store::ConfigStore::new(self.inner.state_root.clone());
        self.inner.ensure_target_available(profile_id)?;
        let target = store.find_provider(profile_id).map_err(|e| e.to_string())?;
        validate_target(&target, model)?;
        let mut route = self.route_for_profile(&target)?;
        route.fingerprint = target_revision(&target)?;
        let identity = self.inner.state.lock().map_err(|_| "网关状态锁不可用")?.identity.clone();
        route.continuation_key = continuation_key(&identity, &target, &route.fingerprint);
        // The selected destination is a leaf: its own routing plan cannot redirect this request.
        route.claude_primary_model = None;
        route.claude_model_options = None;
        value["model"] = serde_json::Value::String(model.clone());
        let variants = route.connection.endpoint_candidates(&route.upstream_base_url).into_iter()
            .map(|endpoint| { let mut variant = route.clone(); variant.upstream_base_url = endpoint; variant }).collect();
        Ok((variants, serde_json::to_vec(&value).map_err(|e| e.to_string())?, true))
    }
}

#[cfg(test)]
mod tests;
