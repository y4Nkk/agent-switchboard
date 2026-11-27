use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::recovery::{has_coordinate_conflict, RepositoryChange, COMPOSIO_REPO, COMPOSIO_SUBPATH, MYCLAUDE_REPO};
use super::{CatalogError, SkillRepository, SkillRepositoryInput, MAX_REPOSITORIES};
use crate::config_store::write_json_atomic;
use crate::extensions::sources::{
    normalize_github_repository, validate_source_ref, validate_source_subpath,
};

const SCHEMA_VERSION: u8 = 1;
const MAX_CATALOG_BYTES: u64 = 128 * 1024;
static SAVE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogDocument {
    schema_version: u8,
    repositories: Vec<SkillRepository>,
}

pub struct SkillRepositoryCatalog {
    path: PathBuf,
}

impl SkillRepositoryCatalog {
    pub fn from_root(extension_root: PathBuf) -> Self {
        Self {
            path: extension_root.join("skill-repositories.json"),
        }
    }

    /// Absence exposes the seed without a hidden write. The first mutation
    /// persists the complete list, including an intentionally empty catalog.
    pub fn list(&self) -> Result<Vec<SkillRepository>, CatalogError> {
        let file = match fs::File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(seed()),
            Err(error) => return Err(CatalogError::Unreadable(error.to_string())),
        };
        let mut bytes = Vec::new();
        file.take(MAX_CATALOG_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| CatalogError::Unreadable(error.to_string()))?;
        if bytes.len() as u64 > MAX_CATALOG_BYTES {
            return Err(CatalogError::Unsupported(
                "file exceeds the catalog size limit".into(),
            ));
        }
        let document: CatalogDocument = serde_json::from_slice(&bytes)
            .map_err(|error| CatalogError::Unsupported(error.to_string()))?;
        if document.schema_version != SCHEMA_VERSION {
            return Err(CatalogError::Unsupported(format!(
                "schema version {}",
                document.schema_version
            )));
        }
        validate_stored(&document.repositories)?;
        Ok(document.repositories)
    }

    pub fn save(&self, input: SkillRepositoryInput) -> Result<SkillRepository, CatalogError> {
        let input = normalize(input)?;
        let _lock = SAVE_LOCK
            .lock()
            .map_err(|_| CatalogError::Unreadable("save interrupted".into()))?;
        let mut repositories = self.list()?;
        let existing = match &input.id {
            Some(id) => Some(
                repositories
                    .iter()
                    .position(|repo| &repo.id == id)
                    .ok_or(CatalogError::NotFound)?,
            ),
            None => repositories
                .iter()
                .position(|repo| coordinates_match(repo, &input)),
        };
        if repositories
            .iter()
            .enumerate()
            .any(|(index, repo)| Some(index) != existing && coordinates_match(repo, &input))
        {
            return Err(CatalogError::Conflict);
        }
        if existing.is_none() && repositories.len() >= MAX_REPOSITORIES {
            return Err(CatalogError::Invalid(format!(
                "at most {MAX_REPOSITORIES} repositories are supported"
            )));
        }
        let repository = SkillRepository {
            id: existing
                .map(|index| repositories[index].id.clone())
                .unwrap_or_else(|| format!("skill-repo-{}", uuid::Uuid::new_v4().simple())),
            repo: input.repo,
            subpath: input.subpath,
            ref_name: input.ref_name,
            enabled: input.enabled,
        };
        match existing {
            Some(index) => repositories[index] = repository.clone(),
            None => repositories.push(repository.clone()),
        }
        self.write(repositories)?;
        Ok(repository)
    }

    pub fn remove(&self, id: &str) -> Result<(), CatalogError> {
        validate_id(id)?;
        let _lock = SAVE_LOCK
            .lock()
            .map_err(|_| CatalogError::Unreadable("save interrupted".into()))?;
        let mut repositories = self.list()?;
        let index = repositories
            .iter()
            .position(|repo| repo.id == id)
            .ok_or(CatalogError::NotFound)?;
        repositories.remove(index);
        self.write(repositories)
    }

    pub(crate) fn apply_recovery(&self, changes: &[&RepositoryChange]) -> Result<Vec<SkillRepository>, CatalogError> {
        let _lock = SAVE_LOCK
            .lock()
            .map_err(|_| CatalogError::Unreadable("save interrupted".into()))?;
        let mut repositories = self.list()?;
        for change in changes {
            if has_coordinate_conflict(&repositories, &change.replacement) {
                return Err(CatalogError::Conflict);
            }
            let repository = repositories.iter_mut().find(|repo| **repo == change.original)
                .ok_or(CatalogError::Changed)?;
            *repository = change.replacement.clone();
        }
        self.write(repositories.clone())?;
        Ok(repositories)
    }

    fn write(&self, repositories: Vec<SkillRepository>) -> Result<(), CatalogError> {
        let document = CatalogDocument {
            schema_version: SCHEMA_VERSION,
            repositories,
        };
        let json = serde_json::to_string_pretty(&document)
            .map_err(|error| CatalogError::Unreadable(error.to_string()))?;
        if json.len() as u64 > MAX_CATALOG_BYTES {
            return Err(CatalogError::Invalid(
                "catalog exceeds the size limit".into(),
            ));
        }
        write_json_atomic(&self.path, &json).map_err(CatalogError::Unreadable)
    }
}

fn normalize(mut input: SkillRepositoryInput) -> Result<SkillRepositoryInput, CatalogError> {
    if let Some(id) = &input.id {
        validate_id(id)?;
    }
    input.repo = normalize_github_repository(&input.repo)
        .map_err(|error| CatalogError::Invalid(error.to_string()))?;
    input.subpath = input.subpath.trim().to_owned();
    if input.subpath.len() > 1024 {
        return Err(CatalogError::Invalid("subpath exceeds 1024 bytes".into()));
    }
    validate_source_subpath(&input.subpath)
        .map_err(|error| CatalogError::Invalid(error.to_string()))?;
    input.ref_name = input
        .ref_name
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty());
    if let Some(name) = &input.ref_name {
        validate_source_ref(name).map_err(|error| CatalogError::Invalid(error.to_string()))?;
    }
    Ok(input)
}

fn validate_id(id: &str) -> Result<(), CatalogError> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err(CatalogError::Invalid("repository id is invalid".into()));
    }
    Ok(())
}

fn coordinates_match(repository: &SkillRepository, input: &SkillRepositoryInput) -> bool {
    repository.repo == input.repo
        && repository.subpath == input.subpath
        && repository.ref_name == input.ref_name
}

fn validate_stored(repositories: &[SkillRepository]) -> Result<(), CatalogError> {
    if repositories.len() > MAX_REPOSITORIES {
        return Err(CatalogError::Unsupported(
            "too many saved repositories".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut coordinates = BTreeSet::new();
    for repo in repositories {
        let normalized = normalize(SkillRepositoryInput {
            id: Some(repo.id.clone()),
            repo: repo.repo.clone(),
            subpath: repo.subpath.clone(),
            ref_name: repo.ref_name.clone(),
            enabled: repo.enabled,
        })
        .map_err(|error| CatalogError::Unsupported(error.to_string()))?;
        if !coordinates_match(repo, &normalized)
            || !ids.insert(&repo.id)
            || !coordinates.insert((&repo.repo, &repo.subpath, &repo.ref_name))
        {
            return Err(CatalogError::Unsupported(
                "noncanonical or duplicate repository records".into(),
            ));
        }
    }
    Ok(())
}

fn seed() -> Vec<SkillRepository> {
    [
        ("anthropics/skills", "main", ""),
        (COMPOSIO_REPO, "master", COMPOSIO_SUBPATH),
        (MYCLAUDE_REPO, "master", ""),
        ("jimliu/baoyu-skills", "main", ""),
    ]
    .into_iter()
    .map(|(repo, branch, subpath)| SkillRepository {
        id: format!("skill-repo-{}", repo.replace('/', "-")),
        repo: repo.into(),
        subpath: subpath.into(),
        ref_name: Some(branch.into()),
        enabled: true,
    })
    .collect()
}
