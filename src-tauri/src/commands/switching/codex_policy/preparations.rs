use crate::commands::error::CommandError;
use crate::gateway::codex::policy::{self, CodexGatewayPolicy};
use crate::gateway::{GatewayController, GatewayProjection};
use crate::local_state::LocalState;
use asb_switch::FilePreview;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub(super) struct Prepared {
    pub root: PathBuf,
    pub profile_id: String,
    pub profile_revision: String,
    pub policy_revision: String,
    pub policy: CodexGatewayPolicy,
    pub projection: GatewayProjection,
    pub preview: FilePreview,
    created: Instant,
}
#[derive(Default, Clone)]
pub(crate) struct CodexPolicyPreparations(Arc<Mutex<HashMap<String, Prepared>>>);

impl CodexPolicyPreparations {
    pub(super) fn insert(&self, prepared: Prepared) -> Result<String, CommandError> {
        let mut entries = self.0.lock().map_err(|_| unavailable())?;
        entries.retain(|_, entry| entry.created.elapsed() < Duration::from_secs(600));
        if entries.len() >= 64 {
            return Err(unavailable());
        }
        let id = uuid::Uuid::new_v4().to_string();
        entries.insert(id.clone(), prepared);
        Ok(id)
    }
    pub(super) fn take(&self, id: &str) -> Result<Prepared, CommandError> {
        let entry = self
            .0
            .lock()
            .map_err(|_| unavailable())?
            .remove(id)
            .filter(|entry| entry.created.elapsed() < Duration::from_secs(600))
            .ok_or_else(|| {
                CommandError::keyed(
                    "codex-policy-preview-stale",
                    "errors.sw.codexPolicyPreviewStale",
                    "Codex 网关预览已失效，请重新预览",
                )
            })?;
        Ok(entry)
    }
    pub(super) fn cancel(&self, id: &str) -> Result<(), CommandError> {
        self.0.lock().map_err(|_| unavailable())?.remove(id);
        Ok(())
    }
}
fn unavailable() -> CommandError {
    CommandError::keyed(
        "codex-policy-preview-unavailable",
        "errors.sw.codexPolicyPreviewUnavailable",
        "Codex 网关预览暂不可用",
    )
}

pub(super) fn prepare(
    state: &LocalState,
    gateway: &GatewayController,
    profile_id: String,
    policy: CodexGatewayPolicy,
) -> Result<Prepared, CommandError> {
    let invalid = |message| CommandError::new("codex-policy-invalid", message);
    policy.validate().map_err(invalid)?;
    if policy.enabled && policy.provider_ids.first() != Some(&profile_id) {
        return Err(CommandError::keyed(
            "codex-policy-invalid",
            "errors.sw.failoverRequiresQueueHead",
            "开启故障转移必须先通过事务激活队列首项",
        ));
    }
    let (_, policy_revision) = policy::load(state.root()).map_err(invalid)?;
    let store = state.configuration();
    let (file, profile_revision) = store
        .find_codex_provider_with_revision(&profile_id)
        .map_err(crate::commands::error::store_error)?;
    let settings = crate::codex_common::resolve(state, &profile_id).map_err(invalid)?;
    let projection = gateway
        .project_codex_with_policy(&file, settings, &policy, None)
        .map_err(invalid)?;
    let preview = super::super::plan::preview_projection(state, &projection)?;
    Ok(Prepared {
        root: state.root().to_path_buf(),
        profile_id,
        profile_revision,
        policy_revision,
        policy,
        projection,
        preview,
        created: Instant::now(),
    })
}

pub(super) fn validate(
    state: &LocalState,
    gateway: &GatewayController,
    prepared: &Prepared,
) -> Result<(), CommandError> {
    let stale = || {
        CommandError::keyed(
            "codex-policy-preview-stale",
            "errors.sw.policyContextChanged",
            "供应商、网关策略或配置来源已变化，请重新预览",
        )
    };
    if state.root() != prepared.root {
        return Err(stale());
    }
    let current = prepare(
        state,
        gateway,
        prepared.profile_id.clone(),
        prepared.policy.clone(),
    )?;
    if current.profile_revision != prepared.profile_revision
        || current.policy_revision != prepared.policy_revision
        || current.projection.candidate_revision()
            != prepared.projection.candidate_revision()
        || current.preview.preview.target != prepared.preview.preview.target
        || current.preview.content_hash != prepared.preview.content_hash
        || current.preview.rendered_hash != prepared.preview.rendered_hash
        || current.preview.auth_hash != prepared.preview.auth_hash
        || current.preview.auth_rendered_hash != prepared.preview.auth_rendered_hash
    {
        return Err(stale());
    }
    Ok(())
}
