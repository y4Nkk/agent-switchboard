mod installations;
mod packages;

use super::{observation::Observation, types::{Group, Issue}};
use installations::Installations;
use std::{collections::HashMap, fs, io::Read, path::Path};

pub struct Executables { paths: HashMap<String, Group>, installations: Installations }

pub fn named_group(name: &str) -> Option<Group> {
    let name = name.to_ascii_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    match name {
        "codex" => Some(Group::Codex),
        "claude" | "claude-code" => Some(Group::Claude),
        _ => None,
    }
}

pub fn candidate(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    named_group(&name).is_some() || version_name(name.strip_suffix(".exe").unwrap_or(&name))
}

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
        let known = self.paths.get(&normalized).copied().or_else(|| self.installations.classify(&normalized));
        let known = match known {
            Some(group) => Some(group),
            None => packages::classify(path)?,
        };
        let Some(group) = known else {
            return if named_group(&process.name).is_some() { Err(Issue::UnrecognizedExecutable) } else { Ok(None) };
        };
        if !is_native(path)? { return Err(Issue::UnrecognizedExecutable); }
        Ok(Some(group))
    }
}

fn normalize(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let normalized = normalized.strip_prefix("//?/").unwrap_or(&normalized).trim_end_matches('/');
    if cfg!(windows) { normalized.to_lowercase() } else { normalized.to_string() }
}

fn canonical_root(path: &Path) -> String {
    normalize(&fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
}

fn version_name(name: &str) -> bool {
    let parts: Vec<&str> = name.split('.').collect();
    parts.len() == 3 && parts.iter().all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn io_issue(error: std::io::Error) -> Issue {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => Issue::AccessDenied,
        std::io::ErrorKind::NotFound => Issue::IdentityUnavailable,
        _ => Issue::CollectionFailed,
    }
}

fn is_native(path: &Path) -> Result<bool, Issue> {
    let mut file = fs::File::open(path).map_err(io_issue)?;
    let mut magic = [0; 4];
    file.read_exact(&mut magic).map_err(|_| Issue::IdentityUnavailable)?;
    Ok(magic.starts_with(b"MZ") || magic == *b"\x7fELF" || matches!(magic,
        [0xfe, 0xed, 0xfa, 0xce] | [0xce, 0xfa, 0xed, 0xfe] | [0xfe, 0xed, 0xfa, 0xcf]
        | [0xcf, 0xfa, 0xed, 0xfe] | [0xca, 0xfe, 0xba, 0xbe] | [0xbe, 0xba, 0xfe, 0xca]
        | [0xca, 0xfe, 0xba, 0xbf] | [0xbf, 0xba, 0xfe, 0xca]))
}

#[cfg(test)]
mod tests;
