//! Application-owned artifacts and history around an executor projection.
use crate::{
    commands::error::CommandError,
    gateway::{GatewayController, GatewayProjection},
    local_state::LocalState,
};
use asb_core::AppKind;
use std::path::Path;
pub(super) fn auth_intent(
    before: Option<&str>,
    existed: Option<bool>,
    after: Option<&str>,
) -> Result<Option<super::transaction::AuthIntent>, CommandError> {
    match (before, existed, after) {
        (None, None, None) => Ok(None),
        (Some(before), Some(existed), Some(after)) => Ok(Some(super::transaction::AuthIntent {
            before_hash: before.into(),
            before_existed: existed,
            after_hash: after.into(),
            after_existed: true,
        })),
        _ => Err(CommandError::keyed(
            "codex-auth-preview-invalid",
            "errors.sw.codexAuthPreviewIncomplete",
            "Codex 认证预览字段不完整，请重新查看差异",
        )),
    }
}
pub(super) fn begin(
    state: &LocalState,
    gateway: &GatewayController,
    projection: &GatewayProjection,
    target: &Path,
    expected_hash: &str,
    expected_rendered_hash: &str,
    auth: Option<super::transaction::AuthIntent>,
) -> Result<Vec<asb_core::contracts::LocalizedMessage>, CommandError> {
    let plan = &projection.plan;
    let preservation = if plan.app() == AppKind::Claude {
        crate::session_manager::history::preserve_before_switch(state.root())
            .map_err(|message| CommandError::new("claude-session-preservation-failed", message))?
    } else { Vec::new() };
    let catalog = match (plan.app(), plan.profile.route_mode) {
        (AppKind::Codex, asb_core::RouteMode::Custom) => {
            super::plan::catalog_artifact(target, projection.codex_catalog.as_ref())?
        }
        (AppKind::Codex, asb_core::RouteMode::Official) => {
            super::plan::official_catalog_artifact(target)?
        }
        (AppKind::Claude, _) => None,
    };
    let backfill = if plan.app() == AppKind::Codex {
        super::codex_backfill::prepare(state, gateway, &plan.profile.id, expected_hash)?
    } else {
        None
    };
    super::transaction::begin_with_codex_backfill_and_auth(
        state,
        gateway,
        plan.app(),
        Some(&plan.profile.id),
        expected_rendered_hash,
        true,
        catalog.clone(),
        backfill.as_ref(),
        auth,
    )?;
    if let Some(backfill) = backfill {
        if let Err(error) = super::codex_backfill::apply(state, backfill) {
            return match super::transaction::recover(state, gateway) {
                Ok(()) => Err(error),
                Err(recovery) => Err(CommandError::localized(
                    "codex-live-backfill-recovery-required",
                    "errors.sw.codexBackfillRecoveryFailed",
                    format!("{}；Codex 回填未能恢复：{recovery}", error.message),
                    serde_json::json!({ "detail": error.message, "recovery": recovery }),
                )),
            };
        }
    }
    super::transaction::stage_catalog(state, gateway, catalog.as_ref())?;
    Ok(preservation)
}
pub(super) fn commit(
    state: &LocalState,
    gateway: &GatewayController,
    projection: &GatewayProjection,
    outcome: &asb_switch::SwitchOutcome,
) -> Result<(), String> {
    let plan = &projection.plan;
    gateway.commit(projection, || {
        state
            .configuration()
            .record_config_write(asb_core::ConfigWriteRecord {
                app: plan.app(),
                profile_id: Some(plan.profile.id.clone()),
                profile_name: Some(plan.profile.name.clone()),
                content_hash: outcome.final_hash.clone(),
                backup_id: outcome.backup.id.clone(),
                at: outcome.backup.created_at.clone(),
                operation: asb_core::WriteOperation::Projection,
            })
            .map_err(|error| error.to_string())
    })
}
