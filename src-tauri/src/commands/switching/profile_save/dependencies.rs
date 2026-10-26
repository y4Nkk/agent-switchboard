use super::*;
use crate::commands::switching::plan::{build_plan_for_profile, preview_projection};
use asb_core::AppKind;

pub(super) fn affected_active_profile(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    current: &ProviderProfile,
    draft: &ProviderDraft,
) -> Result<Option<ProviderProfile>, CommandError> {
    let active_id = crate::commands::config_status_report(state, gateway)?
        .into_iter().find(|status| status.app == current.app)
        .and_then(|status| status.active_profile_id);
    let Some(active_id) = active_id else { return Ok(None) };
    if active_id == current.id {
        return Ok(Some(ProviderProfile::from_draft(current.id.clone(), draft.clone())));
    }
    if current.app != AppKind::Claude
        || gateway.active_profile_id_for(AppKind::Claude).as_deref() != Some(active_id.as_str())
    {
        return Ok(None);
    }
    let policy = crate::gateway::failover::load(state.root())
        .map_err(|error| CommandError::new("claude-policy-invalid", error))?;
    if !policy.enabled || !policy.provider_ids.contains(&current.id) {
        return Ok(None);
    }
    state.configuration().find_provider(&active_id).map(Some)
        .map_err(|error| operation_error("profile-not-found", error))
}

pub(super) fn projection(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    owner: &ProviderProfile,
    candidate: &ProviderProfile,
) -> Result<crate::gateway::GatewayProjection, CommandError> {
    build_plan_for_profile(state, gateway, owner.clone(), Some(candidate))
}

pub(super) fn preview(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    owner: &ProviderProfile,
    candidate: &ProviderProfile,
) -> Result<(asb_switch::FilePreview, String), CommandError> {
    let projection = projection(state, gateway, owner, candidate)?;
    let revision = projection.candidate_revision();
    let mut preview = preview_projection(state, &projection)?;
    if owner.id != candidate.id {
        preview.preview.warnings.push(asb_core::contracts::LocalizedMessage::new(
            "warnings.provider.dependentRoutesSynced",
            serde_json::json!({"owner":owner.name,"candidate":candidate.name}),
            format!(
                "当前主供应商「{}」引用了「{}」；本次保存同步更新调用配置与模型目录，主供应商保持不变。",
                owner.name, candidate.name,
            ),
        ));
    }
    Ok((preview, revision))
}

pub(super) fn confirmed_projection(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    owner: &ProviderProfile,
    prepared: &PreparedProfileSave,
) -> Result<crate::gateway::GatewayProjection, CommandError> {
    let candidate = ProviderProfile::from_draft(
        prepared.profile_id.as_ref().expect("active preparation has a profile").clone(),
        prepared.draft.clone(),
    );
    let projection = projection(state, gateway, owner, &candidate)?;
    if prepared.candidate_revision.as_deref() != Some(projection.candidate_revision().as_str()) {
        return Err(CommandError::keyed(
            "profile-save-stale", "errors.sw.saveContextChanged",
            "供应商、客户端设置或当前客户端配置已变更，请重新保存并查看最新差异",
        ));
    }
    Ok(projection)
}
