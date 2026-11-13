use super::*;

pub(super) fn validate_references(
    state: &crate::local_state::LocalState,
    candidate: &CodexProviderFile,
) -> Result<(), CommandError> {
    state.configuration().validate_codex_provider_references(candidate)
        .map_err(|error| CommandError::new("codex-subagent-route-unresolved", error.to_string()))
}

pub(super) fn affected_active_profile(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    candidate: &CodexProviderFile,
) -> Result<Option<CodexProviderFile>, CommandError> {
    let active_id = crate::commands::config_status_report(state, gateway)?
        .into_iter().find(|status| status.app == asb_core::AppKind::Codex)
        .and_then(|status| status.active_profile_id);
    let Some(active_id) = active_id else { return Ok(None) };
    if active_id == candidate.profile.id { return Ok(Some(candidate.clone())); }
    let active = state.configuration().list_codex_providers().map_err(store_error)?
        .into_iter().find(|record| record.profile.id == active_id)
        .map(|record| record.profile);
    let Some(active) = active else { return Ok(None) };
    let referenced = active.subagent_route.as_ref()
        .is_some_and(|route| route.profile_id == candidate.profile.id);
    let (policy, _) = crate::gateway::codex::policy::load(state.root())
        .map_err(|error| CommandError::new("codex-policy-invalid", error))?;
    let queued = policy.enabled
        && policy.provider_ids.contains(&candidate.profile.id)
        && gateway.active_profile_id_for(asb_core::AppKind::Codex).as_deref() == Some(active_id.as_str());
    if !referenced && !queued {
        return Ok(None);
    }
    state.configuration().find_codex_provider_file(&active_id).map(Some).map_err(store_error)
}

pub(super) fn projection(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    owner: &CodexProviderFile,
    candidate: &CodexProviderFile,
) -> Result<crate::gateway::GatewayProjection, CommandError> {
    let settings = crate::codex_common::resolve(state, &owner.profile.id)
        .map_err(|error| CommandError::new("codex-common-config-invalid", error))?;
    gateway.project_codex(owner, settings, Some(candidate))
        .map_err(|error| CommandError::new("gateway-projection-invalid", error))
}
