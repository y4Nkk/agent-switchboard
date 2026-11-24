use asb_core::extensions::contracts::{ExtensionPayload, SourceRef};
use asb_core::extensions::skill::{claude_compatibility_notes, ContentEntry};
use asb_core::extensions::validate::validate_definition;
use serde::Deserialize;

use crate::commands::error::CommandError;
use crate::commands::extensions::sources::{cached_candidate, manifest_from_entries};
use crate::commands::extensions::support::{extension_mutation, now, store_error, ExtensionMutationDto};
use crate::extensions::store::ExtensionStore;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum SkillVersionChange {
    Upstream { candidate_id: String },
    Restore { digest: String },
}

struct SkillVersionContent {
    digest: String,
    entries: Vec<ContentEntry>,
    upstream_commit: Option<String>,
}

pub(super) fn apply(
    store: &ExtensionStore,
    definition_id: &str,
    expected_revision: u64,
    change: SkillVersionChange,
) -> Result<ExtensionMutationDto, CommandError> {
    let mut definition = store.get_definition(definition_id).map_err(store_error)?
        .ok_or_else(|| CommandError::keyed("extension-not-found", "errors.extlib.extensionNotFound", "扩展不存在或已被删除"))?;
    if definition.revision != expected_revision {
        return Err(CommandError::keyed("extension-conflict", "errors.extlib.extensionConflictRefresh", "扩展已被其他窗口修改；请刷新后重试"));
    }
    let ExtensionPayload::Skill(skill) = &mut definition.payload else {
        return Err(CommandError::keyed("extension-invalid", "errors.extlib.updateOnlySkill", "更新只适用于 Skill"));
    };
    let content = load(store, definition_id, change, skill.source.as_ref())?;
    let manifest = manifest_from_entries(&content.entries)?;
    skill.compatibility = claude_compatibility_notes(&manifest);
    skill.manifest = manifest;
    skill.content_digest = content.digest.clone();
    if let (Some(source), Some(commit)) = (&mut skill.source, content.upstream_commit) {
        source.resolved_commit = Some(commit);
    }
    definition.revision += 1;
    definition.updated_at = now();
    validate_definition(&definition)
        .map_err(|error| CommandError::new("extension-invalid", error.message))?;
    store.save_skill_version(definition_id, &content.digest, &content.entries).map_err(store_error)?;
    store.update_definition(&definition, expected_revision).map_err(store_error)?;
    Ok(extension_mutation(&definition))
}

fn load(
    store: &ExtensionStore,
    definition_id: &str,
    change: SkillVersionChange,
    source: Option<&SourceRef>,
) -> Result<SkillVersionContent, CommandError> {
    match change {
        SkillVersionChange::Restore { digest } => {
            let entries = store.load_skill_version(definition_id, &digest).map_err(store_error)?;
            Ok(SkillVersionContent { digest, entries, upstream_commit: None })
        }
        SkillVersionChange::Upstream { candidate_id } => upstream(&candidate_id, source),
    }
}

fn upstream(id: &str, source: Option<&SourceRef>) -> Result<SkillVersionContent, CommandError> {
    let candidate = cached_candidate(id).ok_or_else(|| {
        CommandError::keyed(
            "candidate-expired",
            "errors.extlib.updateCandidateExpired",
            "更新候选已过期；请重新检查更新",
        )
    })?;
    if !source.is_some_and(|source| candidate.matches_source(source)) || candidate.resolved_commit.is_none() {
        return Err(CommandError::keyed(
            "candidate-source-conflict",
            "errors.extlib.candidateSourceConflict",
            "候选内容来自不同来源；未更新当前 Skill 的来源绑定",
        ));
    }
    if !candidate.diagnostics.is_empty() {
        return Err(CommandError::new("source-rejected", candidate.diagnostics.join("；")));
    }
    Ok(SkillVersionContent {
        digest: candidate.content_digest,
        entries: candidate.entries,
        upstream_commit: candidate.resolved_commit,
    })
}

#[cfg(test)]
mod tests;
