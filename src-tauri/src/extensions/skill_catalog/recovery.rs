use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{CatalogError, SkillRepository};
use crate::extensions::sources::SourceError;

pub(super) const COMPOSIO_REPO: &str = "composiohq/awesome-claude-skills";
pub(super) const COMPOSIO_SUBPATH: &str = "document-skills";
pub(super) const MYCLAUDE_REPO: &str = "stellarlinkco/myclaude";
const OLD_MYCLAUDE_REPO: &str = "cexll/myclaude";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(super) enum RepositoryRecovery {
    Retry,
    UpdateAddress { repo: String },
    NarrowScope { subpath: String },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillRepositoryRecoveryRequest {
    repository: SkillRepository,
    recovery: RepositoryRecovery,
}

pub(crate) struct RepositoryChange {
    pub(crate) original: SkillRepository,
    pub(crate) replacement: SkillRepository,
    pub(super) recovery: RepositoryRecovery,
}

#[derive(Default)]
pub(crate) struct RepositoryRecoveryPlan {
    pub(crate) targets: BTreeSet<String>,
    pub(crate) changes: Vec<RepositoryChange>,
}

fn replacement(repo: &SkillRepository, recovery: &RepositoryRecovery) -> Option<SkillRepository> {
    let mut edited = repo.clone();
    match recovery {
        RepositoryRecovery::UpdateAddress { repo: address }
            if repo.repo == OLD_MYCLAUDE_REPO && address == MYCLAUDE_REPO =>
        {
            edited.repo = address.clone();
        }
        RepositoryRecovery::NarrowScope { subpath }
            if repo.repo == COMPOSIO_REPO && repo.subpath.is_empty() && subpath == COMPOSIO_SUBPATH
                && matches!(repo.ref_name.as_deref(), None | Some("master")) =>
        {
            edited.subpath = subpath.clone();
        }
        _ => return None,
    }
    Some(edited)
}

pub(super) fn has_coordinate_conflict(repositories: &[SkillRepository], target: &SkillRepository) -> bool {
    repositories.iter().any(|repo| {
        repo.id != target.id && repo.repo == target.repo && repo.subpath == target.subpath
            && repo.ref_name == target.ref_name
    })
}

pub(super) fn offer_recovery(
    repo: &SkillRepository,
    error: &SourceError,
    repositories: &[SkillRepository],
) -> Option<RepositoryRecovery> {
    let recovery = match error {
        SourceError::Http { status: 301 | 308, .. } if repo.repo == OLD_MYCLAUDE_REPO => {
            RepositoryRecovery::UpdateAddress { repo: MYCLAUDE_REPO.into() }
        }
        SourceError::TooManySkills => RepositoryRecovery::NarrowScope { subpath: COMPOSIO_SUBPATH.into() },
        SourceError::Transport(_) | SourceError::GithubRateLimited(_)
        | SourceError::Http { status: 408 | 429 | 500..=599, .. } => {
            return Some(RepositoryRecovery::Retry);
        }
        _ => return None,
    };
    let target = replacement(repo, &recovery)?;
    (!has_coordinate_conflict(repositories, &target)).then_some(recovery)
}

pub(crate) fn plan_repository_recovery(
    repositories: &[SkillRepository],
    requests: Vec<SkillRepositoryRecoveryRequest>,
) -> Result<RepositoryRecoveryPlan, CatalogError> {
    if requests.len() > repositories.len() {
        return Err(CatalogError::Changed);
    }
    let mut plan = RepositoryRecoveryPlan::default();
    for request in requests {
        let original = request.repository;
        if !original.enabled || !repositories.contains(&original) || !plan.targets.insert(original.id.clone()) {
            return Err(CatalogError::Changed);
        }
        let recovery = request.recovery;
        if recovery == RepositoryRecovery::Retry {
            continue;
        }
        let replacement = replacement(&original, &recovery)
            .ok_or_else(|| CatalogError::Invalid("unsupported repository recovery".into()))?;
        if has_coordinate_conflict(repositories, &replacement) {
            return Err(CatalogError::Conflict);
        }
        plan.changes.push(RepositoryChange { original, replacement, recovery });
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_default_offers_verified_scope_without_overwriting_a_new_edit() {
        let repository = SkillRepository {
            id: "composio".into(),
            repo: COMPOSIO_REPO.into(),
            subpath: String::new(),
            ref_name: Some("master".into()),
            enabled: true,
        };
        let recovery = offer_recovery(&repository, &SourceError::TooManySkills, std::slice::from_ref(&repository))
            .unwrap();
        let request = || SkillRepositoryRecoveryRequest { repository: repository.clone(), recovery: recovery.clone() };
        let plan = plan_repository_recovery(std::slice::from_ref(&repository), vec![request()]).unwrap();
        assert_eq!(plan.changes[0].replacement.subpath, COMPOSIO_SUBPATH);

        let edited = SkillRepository { subpath: "slack".into(), ..repository.clone() };
        assert!(matches!(plan_repository_recovery(&[edited], vec![request()]), Err(CatalogError::Changed)));
    }
}
