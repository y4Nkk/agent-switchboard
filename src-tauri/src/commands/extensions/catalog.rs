//! Read-only source discovery and catalog configuration. No library import
//! or client deployment is authorized by any command in this module.

use serde::Serialize;
use tauri::AppHandle;

use crate::commands::error::{blocking, state, CommandError};
use crate::extensions::skill_catalog::{
    self, CatalogError, SkillRepository, SkillRepositoryCatalog,
    SkillRepositoryFailure,
};
use crate::extensions::sources::{self, SkillCandidate};

pub use crate::extensions::skill_catalog::{SkillDirectoryEntry, SkillRepositoryInput};

use super::sources::{cache_candidates, candidate_dto, detailed_source_error, SkillCandidateDto};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCatalogCandidate {
    #[serde(flatten)]
    candidate: SkillCandidateDto,
    repository_id: String,
    repo: String,
    subpath: String,
    ref_name: Option<String>,
    readme_url: Option<String>,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SkillCatalogResult {
    candidates: Vec<SkillCatalogCandidate>,
    failures: Vec<SkillRepositoryFailure>,
}

fn catalog(app: &AppHandle) -> Result<SkillRepositoryCatalog, CommandError> {
    Ok(SkillRepositoryCatalog::from_root(
        state(app)?.root().join("extensions"),
    ))
}

fn catalog_error(error: CatalogError) -> CommandError {
    let code = match &error {
        CatalogError::Invalid(_) => "skill-repository-invalid",
        CatalogError::Unreadable(_) => "skill-catalog-unreadable",
        CatalogError::Unsupported(_) => "skill-catalog-unsupported",
        CatalogError::NotFound => "skill-repository-not-found",
        CatalogError::Conflict => "skill-repository-conflict",
    };
    CommandError::new(code, error.to_string())
}

#[tauri::command]
pub async fn list_skill_repositories(app: AppHandle) -> Result<Vec<SkillRepository>, CommandError> {
    blocking(move || catalog(&app)?.list().map_err(catalog_error)).await
}

#[tauri::command]
pub async fn save_skill_repository(
    app: AppHandle,
    input: SkillRepositoryInput,
) -> Result<SkillRepository, CommandError> {
    blocking(move || catalog(&app)?.save(input).map_err(catalog_error)).await
}

#[tauri::command]
pub async fn remove_skill_repository(app: AppHandle, id: String) -> Result<(), CommandError> {
    blocking(move || catalog(&app)?.remove(&id).map_err(catalog_error)).await
}

#[tauri::command]
pub async fn scan_skill_repositories(app: AppHandle) -> Result<SkillCatalogResult, CommandError> {
    blocking(move || {
        let repositories = catalog(&app)?.list().map_err(catalog_error)?;
        Ok(cache_catalog_scan(skill_catalog::scan_repositories(
            &repositories,
            &sources::http_fetch,
        )))
    })
    .await
}

fn cache_catalog_scan(scan: skill_catalog::CatalogScan) -> SkillCatalogResult {
    let mut result = SkillCatalogResult {
        candidates: Vec::new(),
        failures: scan.failures,
    };
    for group in scan.repositories {
        let repository = group.repository;
        let dtos: Vec<_> = group
            .candidates
            .iter()
            .map(|candidate| SkillCatalogCandidate {
                candidate: candidate_dto(candidate),
                repository_id: repository.id.clone(),
                repo: repository.repo.clone(),
                subpath: candidate.subpath.clone(),
                ref_name: candidate.ref_name.clone(),
                readme_url: readme_url(&repository.repo, candidate),
            })
            .collect();
        cache_candidates(group.candidates);
        result.candidates.extend(dtos);
    }
    for failure in &mut result.failures {
        failure.message = CommandError::new("skill-source-failed", &failure.message).message;
    }
    result.candidates.sort_by(|a, b| {
        a.candidate
            .name
            .to_lowercase()
            .cmp(&b.candidate.name.to_lowercase())
            .then_with(|| a.repo.cmp(&b.repo))
            .then_with(|| a.subpath.cmp(&b.subpath))
    });
    result
}

fn readme_url(repo: &str, candidate: &SkillCandidate) -> Option<String> {
    let commit = candidate.resolved_commit.as_deref()?;
    let mut url = reqwest::Url::parse(&format!("https://github.com/{repo}/")).ok()?;
    {
        let mut segments = url.path_segments_mut().ok()?;
        segments.pop_if_empty().push("blob").push(commit);
        if !candidate.subpath.is_empty() {
            segments.extend(candidate.subpath.split('/'));
        }
        segments.push("SKILL.md");
    }
    Some(url.into())
}

#[tauri::command]
pub async fn search_skill_directory(
    query: String,
) -> Result<Vec<SkillDirectoryEntry>, CommandError> {
    blocking(move || {
        skill_catalog::search_directory(&query, &sources::http_fetch).map_err(detailed_source_error)
    })
    .await
}

#[tauri::command]
pub async fn resolve_directory_skill(
    entry: SkillDirectoryEntry,
) -> Result<Vec<SkillCandidateDto>, CommandError> {
    blocking(move || {
        let candidates = skill_catalog::resolve_directory_skill(&entry, &sources::http_fetch)
            .map_err(detailed_source_error)?;
        let dtos = candidates.iter().map(candidate_dto).collect();
        cache_candidates(candidates);
        Ok(dtos)
    })
    .await
}

