use super::{database::{self, Row}, transcript, Error, Preview, ProviderGroup, Result, Scope, TARGET};
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, BTreeSet}, fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct FileState { pub path: PathBuf, pub id: String, pub provider: Option<String>, pub hash: String }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Snapshot { pub rows: Option<Vec<Row>>, pub files: Vec<FileState>, pub missing: usize }

pub(super) fn absolute(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() { return Err("Codex history location must be absolute".into()); }
    if path.try_exists()? { Ok(path.canonicalize()?) } else { Ok(path.into()) }
}

pub(super) fn regular(path: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || super::super::sources::is_link(&metadata) || !super::super::record_file::transcript(path) {
        return Err(format!("Unsupported or linked Codex transcript: {}", path.display()).into());
    }
    absolute(path)
}

pub(super) fn scan(scope: &Scope) -> Result<Snapshot> {
    let rows = database::rows(scope)?;
    let mut paths = BTreeSet::new();
    let mut expected: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    let mut missing = 0;
    for row in rows.iter().flatten() {
        let path = Path::new(&row.rollout);
        if !path.is_absolute() || !path.try_exists()? {
            if row.provider != TARGET { missing += 1; }
            continue;
        }
        let path = regular(path)?;
        expected.entry(path.clone()).or_default().insert(row.id.clone());
        paths.insert(path);
    }
    for root in crate::local_state::codex_paths::session_roots(&scope.home) {
        let found = super::super::sources::collect(&root);
        if !found.issues.is_empty() { return Err(found.issues.join("; ").into()); }
        for path in found.paths { paths.insert(regular(&path)?); }
    }
    let mut files = Vec::new();
    for path in paths {
        let meta = transcript::inspect(&path)?;
        if let Some(ids) = expected.get(&path) {
            if ids.len() != 1 || !ids.contains(&meta.id) { return Err(format!("Codex transcript identity conflicts with its registration: {}", path.display()).into()); }
        }
        if meta.provider.as_deref() != Some(TARGET) {
            files.push(FileState { hash: transcript::digest(&path)?, path, id: meta.id, provider: meta.provider });
        }
    }
    Ok(Snapshot { rows, files, missing })
}

pub(super) fn summary(id: &str, scope: &Scope, snapshot: &Snapshot) -> Preview {
    let mut groups: BTreeMap<Option<String>, (usize, usize)> = BTreeMap::new();
    let mut ids = BTreeSet::new();
    let mut database_rows = 0;
    for row in snapshot.rows.iter().flatten().filter(|row| row.provider != TARGET) {
        groups.entry(Some(row.provider.clone())).or_default().0 += 1;
        ids.insert(row.id.clone()); database_rows += 1;
    }
    for file in &snapshot.files {
        groups.entry(file.provider.clone()).or_default().1 += 1;
        ids.insert(file.id.clone());
    }
    let roots = crate::local_state::codex_paths::session_roots(&scope.home);
    Preview { id: id.into(), target_provider: TARGET.into(), sessions: ids.len(), database_rows,
        history_root: scope.home.display().to_string(),
        database_path: snapshot.rows.as_ref().map(|_| scope.database().display().to_string()),
        external_files: snapshot.files.iter().filter(|file| !roots.iter().any(|root| file.path.starts_with(root)))
            .map(|file| file.path.display().to_string()).collect(),
        files: snapshot.files.len(), missing_originals: snapshot.missing,
        providers: groups.into_iter().map(|(provider, (database_rows, files))|
            ProviderGroup { provider, database_rows, files }).collect() }
}

pub(super) fn validate_targets(scope: &Scope, snapshot: &Snapshot) -> Result<()> {
    let registered = database::rows(scope)?;
    if registered.is_some() != snapshot.rows.is_some() {
        return Err(Error::Conflict(format!("Native index presence changed: {}", scope.database().display())));
    }
    let roots = crate::local_state::codex_paths::session_roots(&scope.home);
    for file in &snapshot.files {
        let path = regular(&file.path).map_err(|error| Error::Conflict(format!("{}: {error}", file.path.display())))?;
        if path != file.path { return Err(Error::Conflict(format!("Transcript location changed: {}", file.path.display()))); }
        let allowed = roots.iter().any(|root| file.path.starts_with(root)) || registered.iter().flatten().any(|row|
            row.id == file.id && Path::new(&row.rollout).canonicalize().ok().as_ref() == Some(&file.path));
        let header = transcript::inspect(&file.path).map_err(|error| Error::Conflict(format!("{}: {error}", file.path.display())))?;
        if !allowed || header.id != file.id {
            return Err(Error::Conflict(format!("Transcript identity or registration changed: {}", file.path.display())));
        }
    }
    Ok(())
}
