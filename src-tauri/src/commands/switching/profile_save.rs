use super::plan::execute_projection;
mod dependencies;
use crate::commands::error::{operation_error, require_write_confirmation, validation_error, CommandError};
use crate::config_store::PendingProfileSave;
use asb_core::contracts::{
    classify_profile_save, ProfileSaveKind, ProviderDraft, ProviderProfile, ProviderRecord,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProfileSaveIntent {
    Save,
    /// Projects the selected profile even when its stored content is unchanged.
    Activate,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileSaveSource {
    Profile,
    Codex,
}

/// Validates the optimistic provider revision and determines whether this
/// edit changes the active client projection. The backend alone owns this
/// classification because the renderer cannot safely infer live identity.
pub(crate) fn classify_existing_profile_save(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    profile_id: &str,
    draft: &ProviderDraft,
    expected_file_hash: &str,
    intent: ProfileSaveIntent,
) -> Result<(ProviderRecord, ProfileSaveKind, Option<ProviderProfile>), CommandError> {
    draft
        .validate()
        .map_err(|error| validation_error("profile-save-invalid", error))?;
    let record = state
        .configuration()
        .find_provider_record(profile_id)
        .map_err(|error| operation_error("profile-not-found", error))?;
    if record.profile.app != draft.app {
        return Err(CommandError::keyed(
            "profile-save-invalid",
            "validate.save.clientMismatch",
            "供应商不能变更所属客户端",
        ));
    }
    if record.file_hash != expected_file_hash {
        return Err(CommandError::keyed(
            "profile-save-stale",
            "validate.save.staleFile",
            "供应商文件已被外部修改，请重新读取后再保存",
        ));
    }
    if intent == ProfileSaveIntent::Activate {
        let owner = ProviderProfile::from_draft(record.profile.id.clone(), draft.clone());
        return Ok((record, ProfileSaveKind::SaveAndApply, Some(owner)));
    }
    let touches_live = record.profile.draft_touches_live_configuration(draft);
    let owner = if touches_live {
        dependencies::affected_active_profile(state, gateway, &record.profile, draft)?
    } else {
        None
    };
    Ok((
        record.clone(),
        classify_profile_save(Some(&record.profile), draft, owner.is_some()),
        owner,
    ))
}

const PROFILE_SAVE_PREPARATION_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_PROFILE_SAVE_PREPARATIONS: usize = 32;

/// Process-local, one-shot preparations for the provider editor. The entry is
/// deliberately not persisted: a restart requires a fresh preview, while an
/// already confirmed live update has its separate durable recovery journal.
#[derive(Clone, Default)]
pub(crate) struct ProfileSavePreparations {
    inner: Arc<ProfileSavePreparationInner>,
}

#[derive(Default)]
struct ProfileSavePreparationInner {
    entries: Mutex<HashMap<String, PreparedProfileSave>>,
    commit_lock: Mutex<()>,
}

pub(super) struct PreparedProfileSave {
    pub(super) intent: ProfileSaveIntent,
    pub(super) created_at: Instant,
    pub(super) profile_id: Option<String>,
    pub(super) expected_file_hash: Option<String>,
    pub(super) draft: ProviderDraft,
    pub(super) kind: ProfileSaveKind,
    pub(super) preview: Option<asb_switch::FilePreview>,
    pub(super) projection_profile: Option<ProviderProfile>,
    pub(super) candidate_revision: Option<String>,
}

impl ProfileSavePreparations {
    pub(super) fn cancel(&self, preparation_id: &str) -> Result<(), CommandError> {
        self.inner.entries.lock().map_err(|_| CommandError::keyed(
            "profile-save-unavailable", "errors.sw.preparationStateUnavailable", "供应商保存准备状态不可用",
        ))?.remove(preparation_id);
        Ok(())
    }

    pub(super) fn issue(&self, prepared: PreparedProfileSave) -> Result<String, CommandError> {
        let mut entries = self.inner.entries.lock().map_err(|_| {
            CommandError::keyed(
                "profile-save-unavailable",
                "errors.sw.preparationStateUnavailable",
                "供应商保存准备状态不可用",
            )
        })?;
        let now = Instant::now();
        entries.retain(|_, entry| {
            now.saturating_duration_since(entry.created_at) <= PROFILE_SAVE_PREPARATION_TTL
        });
        if entries.len() >= MAX_PROFILE_SAVE_PREPARATIONS {
            if let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.created_at)
                .map(|(id, _)| id.clone())
            {
                entries.remove(&oldest);
            }
        }
        let preparation_id = Uuid::new_v4().to_string();
        entries.insert(preparation_id.clone(), prepared);
        Ok(preparation_id)
    }

    pub(super) fn take(&self, preparation_id: &str) -> Result<PreparedProfileSave, CommandError> {
        let mut entries = self.inner.entries.lock().map_err(|_| {
            CommandError::keyed(
                "profile-save-unavailable",
                "errors.sw.preparationStateUnavailable",
                "供应商保存准备状态不可用",
            )
        })?;
        let prepared = entries.remove(preparation_id).ok_or_else(|| {
            CommandError::keyed(
                "profile-save-stale",
                "errors.sw.savePreviewStale",
                "保存预览已失效，请重新保存并查看最新差异",
            )
        })?;
        if Instant::now().saturating_duration_since(prepared.created_at)
            > PROFILE_SAVE_PREPARATION_TTL
        {
            return Err(CommandError::keyed(
                "profile-save-stale",
                "errors.sw.savePreviewExpired",
                "保存预览已过期，请重新保存并查看最新差异",
            ));
        }
        Ok(prepared)
    }

    pub(super) fn lock_commit(&self) -> Result<MutexGuard<'_, ()>, CommandError> {
        self.inner
            .commit_lock
            .lock()
            .map_err(|_| {
                CommandError::keyed(
                    "profile-save-unavailable",
                    "errors.sw.saveCommitLockUnavailable",
                    "供应商保存事务锁不可用",
                )
            })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSavePreparation {
    pub preparation_id: String,
    pub kind: ProfileSaveKind,
    pub preview: Option<asb_switch::FilePreview>,
}

pub(super) fn prepare_profile_save_data(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    profile_id: Option<&str>,
    draft: &ProviderDraft,
    expected_file_hash: Option<&str>,
    intent: ProfileSaveIntent,
) -> Result<(ProfileSaveKind, Option<asb_switch::FilePreview>, Option<ProviderProfile>, Option<String>), CommandError> {
    if intent == ProfileSaveIntent::Activate && profile_id.is_none() {
        return Err(CommandError::keyed("profile-activation-invalid", "modelRoutes.empty", "请先保存供应商档案，再配置模型路由"));
    }
    match (profile_id, expected_file_hash) {
        (None, None) => {
            draft
                .validate()
                .map_err(|error| validation_error("profile-save-invalid", error))?;
            Ok((ProfileSaveKind::Create, None, None, None))
        }
        (None, Some(_)) => Err(CommandError::keyed(
            "profile-save-invalid",
            "validate.save.createCannotCarryVersion",
            "新建供应商不能携带已有档案版本",
        )),
        (Some(_), None) => Err(CommandError::keyed(
            "profile-save-invalid",
            "validate.save.editRequiresVersion",
            "编辑供应商必须携带当前档案版本",
        )),
        (Some(profile_id), Some(expected_file_hash)) => {
            let (record, kind, owner) = classify_existing_profile_save(
                state,
                gateway,
                profile_id,
                draft,
                expected_file_hash,
                intent,
            )?;
            let (preview, candidate_revision) = if let Some(owner) = &owner {
                let candidate = ProviderProfile::from_draft(record.profile.id, draft.clone());
                let (preview, revision) = dependencies::preview(state, gateway, owner, &candidate)?;
                (Some(preview), Some(revision))
            } else {
                (None, None)
            };
            Ok((kind, preview, owner, candidate_revision))
        }
    }
}

pub(super) fn invalidate_provider_cache(app: &AppHandle, profile_id: &str) {
    if let Ok(state) = crate::local_state::LocalState::from_app(app) {
        if let Err(error) = crate::usage_cache::invalidate(&state, profile_id) {
            log::warn!("无法清除更新供应商的托盘用量缓存: {error}");
        }

    }
    crate::codex_official_quota::invalidate(profile_id);
    crate::tray::refresh(app);
}

fn apply_prepared_profile_save(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    stored: ProviderRecord,
    draft: &ProviderDraft,
    projection: &crate::gateway::GatewayProjection,
    preview: &asb_switch::FilePreview,
) -> Result<ProviderRecord, CommandError> {
    let profile_id = stored.profile.id.clone();
    let candidate = ProviderProfile::from_draft(profile_id.clone(), draft.clone());
    let (original_app, original_file) = state
        .configuration()
        .load_provider_file(&profile_id)
        .map_err(|error| operation_error("profile-save-failed", error))?;
    super::profile_rollback::save(
        state,
        original_app,
        &original_file,
        &candidate,
        &preview.content_hash,
        &preview.rendered_hash,
    )
    .map_err(|e| CommandError::new("profile-save-recovery-required", e))?;
    state
        .configuration()
        .begin_profile_save(&PendingProfileSave {
            profile_id: profile_id.clone(),
            projection_profile_id: projection.plan.profile.id.clone(),
            app: original_app,
            previous_file_hash: stored.file_hash.clone(),
        })
        .map_err(|error| CommandError::new("profile-save-recovery-required", error))?;
    let saved =
        match state
            .configuration()
            .update_provider(&profile_id, draft.clone(), &stored.file_hash)
        {
            Ok(saved) => saved,
            Err(error) => {
                return match state
                    .configuration()
                    .clear_profile_save()
                    .and_then(|_| super::profile_rollback::clear(state))
                {
                    Ok(()) => Err(operation_error("profile-save-failed", error)),
                    Err(clear_error) => Err(CommandError::new(
                        "profile-save-recovery-required",
                        format!("{error}；{clear_error}"),
                    )),
                };
            }
        };
    let execution = execute_projection(
        state,
        gateway,
        projection,
        &preview.content_hash,
        &preview.rendered_hash,
    );
    if let Err(command) = execution {
        if command.code == "config-recovery-required" {
            return Err(command);
        }
        return Err(rollback_profile_save(
            state,
            gateway,
            &profile_id,
            &saved.file_hash,
            command,
        ));
    }
    state
        .configuration()
        .clear_profile_save()
        .map_err(|error| CommandError::new("profile-save-recovery-required", error))?;
    super::profile_rollback::clear(state)
        .map_err(|e| CommandError::new("profile-save-recovery-required", e))?;
    Ok(saved)
}

fn rollback_profile_save(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    profile_id: &str,
    saved_hash: &str,
    command: CommandError,
) -> CommandError {
    match super::profile_rollback::restore(state, profile_id, saved_hash) {
        Ok(_) => match super::transaction::recover(state, gateway)
            .and_then(|_| state.configuration().clear_profile_save())
            .and_then(|_| super::profile_rollback::clear(state))
        {
            Ok(()) => command,
            Err(clear_error) => CommandError::localized(
                "profile-save-recovery-required",
                "errors.sw.profileRolledBackClearFailed",
                format!("{}；供应商档案已回滚，但{clear_error}", command.message),
                serde_json::json!({ "detail": command.message, "error": clear_error }),
            ),
        },
        Err(restore_error) => CommandError::localized(
            "profile-save-recovery-required",
            "errors.sw.profileRollbackFailedRecoveryKept",
            format!(
                "{}；供应商档案未能回滚：{restore_error}；已保留恢复记录以继续完成已确认的保存",
                command.message
            ),
            serde_json::json!({ "detail": command.message, "error": restore_error }),
        ),
    }
}

pub(super) fn commit_prepared_profile_save(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    prepared: &PreparedProfileSave,
    confirm_write: bool,
) -> Result<ProviderRecord, CommandError> {
    let (stored, actual_kind, owner) = match (
        prepared.profile_id.as_deref(),
        prepared.expected_file_hash.as_deref(),
    ) {
        (None, None) if prepared.intent == ProfileSaveIntent::Save => {
            prepared
                .draft
                .validate()
                .map_err(|error| validation_error("profile-save-invalid", error))?;
            (None, ProfileSaveKind::Create, None)
        }
        (Some(profile_id), Some(expected_file_hash)) => {
            let (record, kind, owner) = classify_existing_profile_save(
                state,
                gateway,
                profile_id,
                &prepared.draft,
                expected_file_hash,
                prepared.intent,
            )?;
            (Some(record), kind, owner)
        }
        _ => {
            return Err(CommandError::keyed(
                "profile-save-stale",
                "errors.sw.savePreviewVersionInvalid",
                "保存预览的档案版本无效，请重新保存",
            ));
        }
    };
    if actual_kind != prepared.kind || owner != prepared.projection_profile {
        return Err(CommandError::keyed(
            "profile-save-stale",
            "errors.sw.saveContextChanged",
            "供应商、客户端设置或当前客户端配置已变更，请重新保存并查看最新差异",
        ));
    }
    match prepared.kind {
        ProfileSaveKind::Create => state
            .configuration()
            .create_provider(prepared.draft.clone())
            .map_err(|error| operation_error("profile-save-failed", error)),
        ProfileSaveKind::NoChange => Ok(stored.expect("existing preparation has a record")),
        ProfileSaveKind::SaveOnly => {
            let stored = stored.expect("existing preparation has a record");
            state
                .configuration()
                .update_provider(
                    &stored.profile.id,
                    prepared.draft.clone(),
                    &stored.file_hash,
                )
                .map_err(|error| operation_error("profile-save-failed", error))
        }
        ProfileSaveKind::SaveAndApply => {
            require_write_confirmation(confirm_write, "保存并应用供应商")?;
            let stored = stored.expect("active preparation has a record");
            let preview = prepared.preview.as_ref().ok_or_else(|| {
                CommandError::keyed(
                    "profile-save-stale",
                    "errors.sw.savePreviewMissing",
                    "保存预览缺失，请重新保存",
                )
            })?;
            let projection = dependencies::confirmed_projection(
                state, gateway, owner.as_ref().expect("active preparation has an owner"), prepared,
            )?;
            apply_prepared_profile_save(
                state, gateway, stored, &prepared.draft, &projection, preview,
            )
        }
    }
}

#[cfg(test)]
mod tests;
