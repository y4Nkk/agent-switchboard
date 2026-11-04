use super::{observation::Observation, types::{Group, Issue}};
use crate::client_software::identity::{self, installations, installations::Installations};
use std::{collections::HashMap, fs, path::Path};

pub struct Executables { paths: HashMap<String, Group>, installations: Installations }

fn group(app: asb_core::contracts::AppKind) -> Group { match app { asb_core::contracts::AppKind::Codex => Group::Codex, asb_core::contracts::AppKind::Claude => Group::Claude } }
pub fn named_group(name: &str) -> Option<Group> { identity::named_group(name).map(group) }
pub fn candidate(name: &str) -> bool { identity::candidate(name) }

pub fn outside_scope(process: &Observation) -> bool {
    process.executable.as_deref().is_some_and(|path| installations::desktop_shell(&normalize(path)))
}

impl Executables {
    pub fn new() -> Self {
        let mut directories: Vec<_> = std::env::var_os("PATH").map(|path|
            std::env::split_paths(&path).filter(|directory| directory.is_absolute()).collect()).unwrap_or_default();
        directories.extend(installations::launcher_directories());
        let mut paths = HashMap::new();
        for directory in directories {
            for (name, group) in [("codex", Group::Codex), ("claude", Group::Claude)] {
                let filename = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
                if let Ok(path) = fs::canonicalize(directory.join(filename)) {
                    paths.insert(normalize(&path), group);
                }
            }
        }
        Self { paths, installations: Installations::new() }
    }

    pub fn classify(&self, process: &Observation) -> Result<Option<Group>, Issue> {
        let path = process.executable.as_deref().ok_or(Issue::IdentityUnavailable)?;
        let normalized = normalize(path);
        if installations::desktop_shell(&normalized) { return Ok(None); }
        let known = self.paths.get(&normalized).copied().or_else(|| self.installations.classify(&normalized).map(group));
        let known = match known {
            Some(group) => Some(group),
            None => identity::packages::classify(path).map_err(identity_issue)?.map(group),
        };
        let Some(group) = known else {
            return if named_group(&process.name).is_some() { Err(Issue::UnrecognizedExecutable) } else { Ok(None) };
        };
        if !is_native(path)? { return Err(Issue::UnrecognizedExecutable); }
        Ok(Some(group))
    }
}

use identity::{normalize, is_native as native};
fn identity_issue(issue: identity::IdentityIssue) -> Issue { match issue { identity::IdentityIssue::AccessDenied => Issue::AccessDenied, identity::IdentityIssue::IdentityUnavailable => Issue::IdentityUnavailable, identity::IdentityIssue::CollectionFailed => Issue::CollectionFailed, identity::IdentityIssue::UnrecognizedExecutable => Issue::UnrecognizedExecutable } }
fn is_native(path: &Path) -> Result<bool, Issue> { native(path).map_err(identity_issue) }

#[cfg(test)]
mod tests;
