//! Read-only update guard, using the monitor's native process identity owner.
use super::{observation::ProcessUser, platform::Reader, types::Issue};
use crate::client_software::{discovery, identity, types::Installation};

pub fn ensure_idle(installation: &Installation) -> Result<(), String> {
    let reader = Reader::new()?;
    let (entries, issues) = reader.entries()?;
    if !issues.is_empty() { return Err(format!("Cannot verify running clients: {issues:?}")); }
    let target = identity::canonical_root(&installation.executable);
    let roots: Vec<_> = discovery::process_roots(installation)?.iter()
        .map(|root| format!("{}/", identity::canonical_root(root))).collect();
    for entry in entries {
        if entry.user == ProcessUser::Other { continue; }
        let name = entry.name.to_ascii_lowercase();
        let candidate = identity::candidate(&name) || name.contains("codex") || name.contains("claude")
            || installation.executable.file_name().is_some_and(|filename| filename.to_string_lossy().eq_ignore_ascii_case(&name));
        if !candidate { continue; }
        let observation = match reader.read(&entry) {
            Ok(Some(process)) => process,
            Ok(None) | Err(Issue::ProcessExited) => continue,
            Err(issue) => return Err(format!("Cannot verify process {}: {issue:?}", entry.pid)),
        };
        let path = observation.executable.as_deref().ok_or("A client process path is unavailable")?;
        let normalized = identity::normalize(path);
        let same = normalized == target || roots.iter().any(|root| normalized.starts_with(root));
        if same && !identity::installations::desktop_shell(&normalized) {
            return Err(format!("Close {} process {} before updating", installation.app.label(), entry.pid));
        }
    }
    Ok(())
}

#[cfg(windows)]
pub fn birth(pid: u32) -> Result<Option<String>, String> {
    match super::platform::process_key(pid) {
        Ok(key) => Ok(Some(key.birth.to_string())),
        Err(Issue::ProcessExited) => Ok(None),
        Err(issue) => Err(format!("Process identity unavailable: {issue:?}")),
    }
}

#[cfg(not(windows))]
pub fn birth(pid: u32) -> Result<Option<String>, String> {
    let reader = Reader::new()?;
    let (entries, _) = reader.entries()?;
    let Some(entry) = entries.iter().find(|entry| entry.pid == pid) else { return Ok(None); };
    match reader.read(entry) {
        Ok(Some(process)) => Ok(Some(process.key.birth.to_string())),
        Ok(None) | Err(Issue::ProcessExited) => Ok(None),
        Err(issue) => Err(format!("Process identity unavailable: {issue:?}")),
    }
}
