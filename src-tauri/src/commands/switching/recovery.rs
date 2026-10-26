use super::plan::{build_plan, execute_projection_with_auth, preview_projection};
use super::profile_save::invalidate_provider_cache;
use crate::commands::error::CommandError;
use tauri::{AppHandle, Manager};

/// Finishes the only kind of interrupted provider save that can leave a
/// durable marker: an already-written profile awaiting its active owner's
/// projection. The marker contains no secret; the current provider file is
/// revalidated and rendered again before any live client file is touched.
pub(crate) fn recover_pending_profile_save(app: &AppHandle) -> Result<(), String> {
    let state = crate::local_state::LocalState::from_app(app)?;
    let gateway = app
        .state::<crate::gateway::GatewayController>()
        .inner()
        .clone();
    if let Some(id) = recover_saved_profile(&state, &gateway)? {
        invalidate_provider_cache(app, &id);
    }
    Ok(())
}

pub(super) fn recover_saved_profile(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
) -> Result<Option<String>, String> {
    super::transaction::recover(state, gateway)?;
    let Some(pending) = state
        .configuration()
        .pending_profile_save()
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let revision = saved_profile_revision(state, pending.app, &pending.profile_id)?;
    // A process can stop after writing the marker but before replacing the
    // provider file. In that case the old revision is still authoritative and
    // recovery must discard the marker without touching client configuration.
    if revision == pending.previous_file_hash {
        state.configuration().clear_profile_save()?;
        super::profile_rollback::clear(state)?;
        return Ok(None);
    }
    super::profile_rollback::validate_saved_revision(state, &pending.profile_id, &revision)?;
    let projection =
        build_plan(state, gateway, &pending.projection_profile_id).map_err(|error| error.message)?;
    let preview = preview_projection(state, &projection).map_err(|error| error.message)?;
    if already_committed(state, &pending.projection_profile_id, pending.app, &preview)? {
        gateway.commit(&projection, || Ok(()))?;
        state.configuration().clear_profile_save()?;
        super::profile_rollback::clear(state)?;
        return Ok(Some(pending.profile_id));
    }
    super::profile_rollback::validate_projection(state, pending.app, &preview)?;
    execute_projection_with_auth(
        state,
        gateway,
        &projection,
        &preview.content_hash,
        &preview.rendered_hash,
        preview.auth_hash.as_deref(),
        preview.auth_existed,
        preview.auth_rendered_hash.as_deref(),
    )
    .map_err(|error| error.message)?;
    state.configuration().clear_profile_save()?;
    super::profile_rollback::clear(state)?;
    Ok(Some(pending.profile_id))
}

fn saved_profile_revision(
    state: &crate::local_state::LocalState,
    app: asb_core::AppKind,
    profile_id: &str,
) -> Result<String, String> {
    super::transaction::profile_revision(state, app, profile_id)
}

/// Every later configuration write first completes a previously confirmed
/// profile update. The journal is the sole recovery owner for a crash after
/// the provider file changed but before its client projection did.
pub(crate) fn ensure_profile_save_recovered(app: &AppHandle) -> Result<(), CommandError> {
    recover_pending_profile_save(app)
        .map_err(|error| CommandError::new("profile-save-recovery-required", error))?;
    let state = crate::local_state::LocalState::from_app(app)
        .map_err(|error| CommandError::new("codex-policy-recovery-required", error))?;
    super::codex_policy::recover_on_startup(
        &state,
        app.state::<crate::gateway::GatewayController>().inner(),
    )
    .map_err(|error| CommandError::new("codex-policy-recovery-required", error))
}

fn already_committed(
    state: &crate::local_state::LocalState,
    profile_id: &str,
    app: asb_core::AppKind,
    preview: &asb_switch::FilePreview,
) -> Result<bool, String> {
    let last = state
        .configuration()
        .latest_config_write(app)
        .map_err(|e| e.to_string())?;
    Ok(preview.content_hash == preview.rendered_hash
        && last.is_some_and(|record| {
            record.profile_id.as_deref() == Some(profile_id)
                && record.content_hash == preview.content_hash
                && record.operation == asb_core::WriteOperation::Projection
        }))
}
