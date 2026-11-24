//! Saved public repositories and explicit directory discovery. This module
//! never imports a definition, deploys content, or executes source files.

mod directory;
mod repositories;
mod scan;

use serde::{Deserialize, Serialize};

pub use directory::{resolve_directory_skill, search_directory};
pub use repositories::SkillRepositoryCatalog;
pub use scan::{scan_repositories, CatalogScan};

pub const MAX_REPOSITORIES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillRepository {
    pub id: String,
    pub repo: String,
    pub subpath: String,
    pub ref_name: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillRepositoryInput {
    pub id: Option<String>,
    pub repo: String,
    pub subpath: String,
    pub ref_name: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRepositoryFailure {
    pub repository_id: String,
    pub repo: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillDirectoryEntry {
    pub id: String,
    pub name: String,
    pub repo: String,
    pub subpath: String,
    pub installs: u64,
    pub readme_url: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("Invalid Skill repository: {0}")]
    Invalid(String),
    #[error("Skill repository catalog could not be read or saved: {0}")]
    Unreadable(String),
    #[error("Unsupported Skill repository catalog: {0}")]
    Unsupported(String),
    #[error("Skill repository no longer exists; refresh the repository list")]
    NotFound,
    #[error("Skill repository already exists with this repository, subpath and ref")]
    Conflict,
}

