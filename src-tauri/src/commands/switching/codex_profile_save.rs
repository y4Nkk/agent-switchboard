//! Active-save transaction for the current Codex-only provider contract.
use super::plan::preview_projection;
#[cfg(test)]
use super::plan::{build_codex_plan, execute_projection};
mod dependencies;
use crate::commands::error::{
    operation_error, require_write_confirmation, store_error, CommandError,
};
use crate::config_store::{PendingProfileSave, StoreOperationError};
use asb_core::contracts::{
    CodexProviderDraft, CodexProviderFile, CodexProviderRecord, ProfileSaveKind,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use uuid::Uuid;
use super::profile_save::ProfileSaveIntent;

const PREPARATION_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_PREPARATIONS: usize = 32;

#[derive(Clone, Default)]
pub(crate) struct CodexProfileSavePreparations {
    inner: Arc<PreparationInner>,
}

#[derive(Default)]
struct PreparationInner {
    entries: Mutex<HashMap<String, PreparedCodexProfileSave>>,
    commit_lock: Mutex<()>,
}

pub(super) struct PreparedCodexProfileSave {
    pub(super) intent: ProfileSaveIntent,
    pub(super) created_at: Instant,
    pub(super) profile_id: String,
    pub(super) expected_file_hash: String,
    pub(super) draft: CodexProviderDraft,
    pub(super) kind: ProfileSaveKind,
    pub(super) preview: Option<asb_switch::FilePreview>,
    pub(super) projection_profile: Option<CodexProviderFile>,
    pub(super) candidate_revision: Option<String>,
}

impl CodexProfileSavePreparations {
    pub(super) fn cancel(&self, id: &str) -> Result<(), CommandError> {
        self.inner
            .entries
            .lock()
            .map_err(|_| unavailable())?
            .remove(id);
        Ok(())
    }

    pub(super) fn issue(&self, prepared: PreparedCodexProfileSave) -> Result<String, CommandError> {
        let mut entries = self.inner.entries.lock().map_err(|_| unavailable())?;
        let now = Instant::now();
        entries
            .retain(|_, entry| now.saturating_duration_since(entry.created_at) <= PREPARATION_TTL);
        if entries.len() >= MAX_PREPARATIONS {
            if let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.created_at)
                .map(|(id, _)| id.clone())
            {
                entries.remove(&oldest);
            }
        }
        let id = Uuid::new_v4().to_string();
        entries.insert(id.clone(), prepared);
        Ok(id)
    }

    pub(super) fn take(&self, id: &str) -> Result<PreparedCodexProfileSave, CommandError> {
        let mut entries = self.inner.entries.lock().map_err(|_| unavailable())?;
        let prepared = entries.remove(id).ok_or_else(stale)?;
        if Instant::now().saturating_duration_since(prepared.created_at) > PREPARATION_TTL {
            return Err(stale());
        }
        Ok(prepared)
    }

    pub(super) fn lock_commit(&self) -> Result<MutexGuard<'_, ()>, CommandError> {
        self.inner.commit_lock.lock().map_err(|_| unavailable())
    }
}

pub(super) fn prepare_data(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    profile_id: &str,
    draft: &CodexProviderDraft,
    expected_file_hash: &str,
    intent: ProfileSaveIntent,
) -> Result<
    (ProfileSaveKind, Option<asb_switch::FilePreview>, Option<CodexProviderFile>, Option<String>),
    CommandError,
> {
    let (_, candidate, kind, owner) = classify(state, gateway, profile_id, draft, expected_file_hash, intent)?;
    let mut candidate_revision = None;
    let preview = if let Some(owner) = &owner {
        let projection = dependencies::projection(state, gateway, owner, &candidate)?;
        candidate_revision = Some(projection.candidate_revision());
        let mut preview = preview_projection(state, &projection)?;
        if owner.profile.id != candidate.profile.id {
            preview.preview.warnings.push(asb_core::contracts::LocalizedMessage::new(
                "warnings.provider.dependentRoutesSynced",
                serde_json::json!({ "owner": owner.profile.name, "candidate": candidate.profile.name }),
                format!(
                    "当前主供应商「{}」引用了「{}」；本次保存同步更新调用配置与模型目录，主供应商保持不变。",
                    owner.profile.name, candidate.profile.name
                ),
            ));
        }
        Some(preview)
    } else {
        None
    };
    Ok((kind, preview, owner, candidate_revision))
}

pub(super) fn commit(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    prepared: &PreparedCodexProfileSave,
    confirm_write: bool,
) -> Result<CodexProviderRecord, CommandError> {
    let (stored, candidate, actual_kind, owner) = classify(
        state,
        gateway,
        &prepared.profile_id,
        &prepared.draft,
        &prepared.expected_file_hash,
        prepared.intent,
    )?;
    if actual_kind != prepared.kind || owner != prepared.projection_profile {
        return Err(stale());
    }
    match actual_kind {
        ProfileSaveKind::NoChange => Ok(stored),
        ProfileSaveKind::SaveOnly => state
            .configuration()
            .update_codex_provider(
                &prepared.profile_id,
                prepared.draft.clone(),
                &stored.file_hash,
            )
            .map_err(save_failed),
        ProfileSaveKind::SaveAndApply => {
            require_write_confirmation(confirm_write, "保存并应用 Codex 供应商")?;
            let preview = prepared.preview.as_ref().ok_or_else(stale)?;
            let projection = dependencies::projection(
                state, gateway, owner.as_ref().ok_or_else(stale)?, &candidate,
            )?;
            if prepared.candidate_revision.as_deref() != Some(projection.candidate_revision().as_str()) {
                return Err(stale());
            }
            apply(
                state, gateway, stored, candidate, &projection,
                &prepared.draft, preview,
            )
        }
        ProfileSaveKind::Create => Err(CommandError::keyed(
            "codex-profile-save-invalid",
            "errors.sw.codexCreateNoActiveSave",
            "Codex 新建供应商不使用活跃保存事务",
        )),
    }
}

fn classify(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    profile_id: &str,
    draft: &CodexProviderDraft,
    expected_file_hash: &str,
    intent: ProfileSaveIntent,
) -> Result<
    (CodexProviderRecord, CodexProviderFile, ProfileSaveKind, Option<CodexProviderFile>),
    CommandError,
> {
    let stored = state
        .configuration()
        .list_codex_providers()
        .map_err(store_error)?
        .into_iter()
        .find(|record| record.profile.id == profile_id)
        .ok_or_else(|| {
            CommandError::keyed(
                "codex-profile-not-found",
                "errors.sw.codexProfileNotFound",
                "Codex 供应商不存在",
            )
        })?;
    if stored.file_hash != expected_file_hash {
        return Err(CommandError::keyed(
            "codex-profile-save-stale",
            "errors.sw.codexProfileFileModified",
            "Codex 供应商文件已被外部修改，请重新读取后再保存",
        ));
    }
    let current = state
        .configuration()
        .find_codex_provider_file(profile_id)
        .map_err(store_error)?;
    let candidate = draft
        .clone()
        .into_file(profile_id.to_string(), current.position);
    candidate
        .validate()
        .map_err(|error| CommandError::new("codex-profile-save-invalid", error))?;
    dependencies::validate_references(state, &candidate)?;
    if intent == ProfileSaveIntent::Activate {
        return Ok((stored, candidate.clone(), ProfileSaveKind::SaveAndApply, Some(candidate)));
    }
    if candidate == current {
        return Ok((stored, candidate, ProfileSaveKind::NoChange, None));
    }
    // The catalog and protocol capabilities are not all represented in the
    // client TOML, but they do define the active gateway snapshot. Any change
    // to route facts or client-owned parameters must therefore re-apply an
    // active Codex route; local notes and usage metadata do not.
    let live_change =
        current.profile != candidate.profile || current.parameters != candidate.parameters;
    let owner = if live_change {
        dependencies::affected_active_profile(state, gateway, &candidate)?
    } else {
        None
    };
    let kind = if owner.is_some() {
        ProfileSaveKind::SaveAndApply
    } else {
        ProfileSaveKind::SaveOnly
    };
    Ok((stored, candidate, kind, owner))
}

/// A subagent route names another provider profile by UUID. The reference
/// must resolve at save time — the same fail-loud rule the gateway applies
/// at request time — and its credentials must be statically expressible:
/// an auth-bound profile cannot be forwarded to by another route.
pub(crate) fn validate_subagent_route_reference(
    state: &crate::local_state::LocalState,
    route: &asb_core::contracts::CodexSubagentRoute,
) -> Result<(), CommandError> {
    let target = state
        .configuration()
        .find_codex_provider_file(&route.profile_id)
        .map_err(|_| {
            CommandError::localized(
                "codex-subagent-route-unresolved",
                "errors.sw.subagentRouteTargetMissing",
                format!(
                    "子代理路由引用的 Codex 供应商不存在：{}（模型 {}）",
                    route.profile_id, route.model
                ),
                serde_json::json!({ "profileId": route.profile_id, "model": route.model }),
            )
        })?;
    dependencies::validate_target(route, &target)
}

fn apply(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    stored: CodexProviderRecord,
    candidate: CodexProviderFile,
    projection: &crate::gateway::GatewayProjection,
    draft: &CodexProviderDraft,
    preview: &asb_switch::FilePreview,
) -> Result<CodexProviderRecord, CommandError> {
    let current = state
        .configuration()
        .find_codex_provider_file(&stored.profile.id)
        .map_err(store_error)?;
    super::profile_rollback::save_codex(
        state,
        &current,
        &candidate,
        &preview.content_hash,
        &preview.rendered_hash,
    )
    .map_err(recovery_required)?;
    state
        .configuration()
        .begin_profile_save(&PendingProfileSave {
            profile_id: stored.profile.id.clone(),
            projection_profile_id: projection.plan.profile.id.clone(),
            app: asb_core::AppKind::Codex,
            previous_file_hash: stored.file_hash.clone(),
        })
        .map_err(recovery_required)?;
    let saved = match state.configuration().update_codex_provider(
        &stored.profile.id,
        draft.clone(),
        &stored.file_hash,
    ) {
        Ok(saved) => saved,
        Err(error) => return clear_failed_save(state, error),
    };
    if let Err(error) = super::plan::execute_projection_with_auth(
        state,
        gateway,
        projection,
        &preview.content_hash,
        &preview.rendered_hash,
        preview.auth_hash.as_deref(),
        preview.auth_existed,
        preview.auth_rendered_hash.as_deref(),
    ) {
        if error.code == "config-recovery-required" {
            return Err(error);
        }
        return Err(rollback(
            state,
            gateway,
            &stored.profile.id,
            &saved.file_hash,
            error,
        ));
    }
    state
        .configuration()
        .clear_profile_save()
        .map_err(recovery_required)?;
    super::profile_rollback::clear(state).map_err(recovery_required)?;
    Ok(saved)
}

fn clear_failed_save(
    state: &crate::local_state::LocalState,
    update_error: StoreOperationError,
) -> Result<CodexProviderRecord, CommandError> {
    state
        .configuration()
        .clear_profile_save()
        .and_then(|_| super::profile_rollback::clear(state))
        .map_err(|clear_error| recovery_required(format!("{update_error}；{clear_error}")))?;
    Err(save_failed(update_error))
}

fn rollback(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    profile_id: &str,
    saved_hash: &str,
    original: CommandError,
) -> CommandError {
    match super::profile_rollback::restore(state, profile_id, saved_hash) {
        Ok(()) => match super::transaction::recover(state, gateway)
            .and_then(|_| state.configuration().clear_profile_save())
            .and_then(|_| super::profile_rollback::clear(state))
        {
            Ok(()) => original,
            Err(error) => CommandError::localized(
                "codex-profile-save-recovery-required",
                "errors.sw.codexProfileRolledBackClearFailed",
                format!(
                    "{}；Codex 供应商档案已回滚，但{error}",
                    original.message
                ),
                serde_json::json!({ "detail": original.message, "error": error }),
            ),
        },
        Err(error) => CommandError::localized(
            "codex-profile-save-recovery-required",
            "errors.sw.codexProfileRollbackFailedRecoveryKept",
            format!(
                "{}；Codex 供应商未能回滚：{error}；已保留恢复记录以继续完成已确认的保存",
                original.message
            ),
            serde_json::json!({ "detail": original.message, "error": error }),
        ),
    }
}

fn unavailable() -> CommandError {
    CommandError::keyed(
        "codex-profile-save-unavailable",
        "errors.sw.codexPreparationStateUnavailable",
        "Codex 供应商保存准备状态不可用",
    )
}

fn stale() -> CommandError {
    CommandError::keyed(
        "codex-profile-save-stale",
        "errors.sw.codexSavePreviewStale",
        "Codex 保存预览已失效，请重新保存并查看最新差异",
    )
}

fn save_failed(error: StoreOperationError) -> CommandError {
    operation_error("codex-profile-save-failed", error)
}

fn recovery_required(message: impl Into<String>) -> CommandError {
    CommandError::new("codex-profile-save-recovery-required", message)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod dependency_tests;
#[cfg(test)]
mod queue_tests;
