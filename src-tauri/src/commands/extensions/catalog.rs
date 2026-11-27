//! Read-only source discovery and catalog configuration. No library import
//! or client deployment is authorized by any command in this module.

use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::AppHandle;
use uuid::Uuid;

use crate::commands::error::{blocking, state, CommandError};
use crate::extensions::skill_catalog::{
    self, CatalogError, CatalogScanMonitor, CatalogScanProgress, RepositoryScan, SkillRepository,
    SkillRepositoryCatalog, SkillRepositoryRecoveryRequest,
};
use crate::extensions::sources::{self, SkillCandidate};

pub use crate::extensions::skill_catalog::{SkillDirectoryEntry, SkillRepositoryInput};

use super::sources::{cache_candidates, candidate_dto, detailed_source_error, SkillCandidateDto};

const REPOSITORY_SCAN_NETWORK_BUDGET: Duration = Duration::from_secs(45);

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillCatalogResult {
    candidates: Vec<SkillCatalogCandidate>,
    progress: CatalogScanProgress,
    updated_repositories: Option<Vec<SkillRepository>>,
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
        CatalogError::Changed => "skill-repository-changed",
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
pub async fn scan_skill_repositories(
    app: AppHandle,
    scan_id: String,
    recoveries: Vec<SkillRepositoryRecoveryRequest>,
) -> Result<SkillCatalogResult, CommandError> {
    let scan_id = parse_scan_id(&scan_id)?;
    blocking(move || {
        let catalog = catalog(&app)?;
        let mut repositories = catalog.list().map_err(catalog_error)?;
        let recovery = skill_catalog::plan_repository_recovery(&repositories, recoveries)
            .map_err(catalog_error)?;
        if !recovery.targets.is_empty() {
            repositories.sort_by_key(|repo| !recovery.targets.contains(&repo.id));
        }
        let enabled: Vec<_> = repositories.iter().filter(|repo| repo.enabled).cloned().collect();
        let started_at = Instant::now();
        let monitor = CatalogScanMonitor::begin(scan_id, started_at, &enabled).ok_or_else(|| {
            CommandError::new("skill-scan-already-active", "此仓库扫描已在进行")
        })?;
        let scanned = skill_catalog::scan_repositories(
            &repositories,
            &recovery.changes,
            started_at + REPOSITORY_SCAN_NETWORK_BUDGET,
            &monitor,
            sources::cached_repository,
        );
        let verified: Vec<_> = recovery.changes.iter().filter(|change| {
            scanned.iter().any(|scan| scan.repository == change.replacement)
        }).collect();
        let updated_repositories = if verified.is_empty() {
            None
        } else {
            let saved = catalog.apply_recovery(&verified).map_err(catalog_error)?;
            for change in verified {
                monitor.replace_repository(&change.replacement);
            }
            Some(saved)
        };
        let candidates = cache_catalog_candidates(scanned);
        Ok(SkillCatalogResult { candidates, progress: monitor.snapshot(), updated_repositories })
    })
    .await
}

#[tauri::command]
pub async fn read_skill_repository_scan(scan_id: String) -> Result<Option<CatalogScanProgress>, CommandError> {
    Ok(CatalogScanMonitor::read(parse_scan_id(&scan_id)?))
}

fn parse_scan_id(id: &str) -> Result<Uuid, CommandError> {
    Uuid::parse_str(id).map_err(|_| CommandError::new("skill-scan-invalid-id", "仓库扫描标识无效"))
}

fn cache_catalog_candidates(repositories: Vec<RepositoryScan>) -> Vec<SkillCatalogCandidate> {
    sources::cache_catalog_repositories(&repositories);
    let mut result = Vec::new();
    for group in repositories {
        let repository = group.repository;
        let dtos: Vec<_> = group
            .result.candidates
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
        result.extend(dtos);
    }
    result.sort_by(|a, b| {
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

