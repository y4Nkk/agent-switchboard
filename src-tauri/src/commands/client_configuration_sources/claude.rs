use super::{ConfigurationSources, Source, observable_keys, observation, path_display};
use crate::local_state::LocalState;
use asb_core::AppKind;
use serde_json::Value;
use std::{fs, io::{self, Read}, path::{Path, PathBuf}};

const FILE_LIMIT: u64 = 2 * 1024 * 1024;

pub(super) fn inspect(state: &LocalState, cwd: Option<&Path>, report: &mut ConfigurationSources) -> Result<(), String> {
    let user = state.target(AppKind::Claude)?;
    let keys = observable_keys(AppKind::Claude);
    let mut sources = vec![("user", user.clone())];
    if let Some(cwd) = cwd {
        sources.push(("project", cwd.join(".claude/settings.json")));
        sources.push(("local", cwd.join(".claude/settings.local.json")));
        if !cfg!(windows) {
            if let Some(root) = repository_checkout(cwd).filter(|root| root.as_path() != cwd && !user.starts_with(root)) {
                sources.push(("local", root.join(".claude/settings.local.json")));
            }
        }
    }
    let managed = crate::commands::client_settings::software_policy::managed_directory();
    sources.push(("managedFile", managed.join("managed-settings.json")));
    let fragments = managed.join("managed-settings.d");
    match fs::read_dir(&fragments) {
        Ok(entries) => {
            let mut files: Vec<_> = entries.filter_map(Result::ok).map(|entry| entry.path()).filter(|path| {
                path.extension().is_some_and(|ext| ext == "json")
                    && path.file_name().is_some_and(|name| !name.to_string_lossy().starts_with('.'))
            }).collect();
            files.sort();
            sources.extend(files.into_iter().map(|path| ("managedFile", path)));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => report.sources.push(Source { label: "managedFile".into(), path: Some(path_display(&fragments)), state: "unreadable" }),
    }
    for (label, path) in sources { inspect_file(&path, label, &keys, report); }
    report.sources.push(Source { label: "sessionManaged".into(), path: None, state: "unverified" });
    Ok(())
}

fn inspect_file(path: &Path, label: &str, keys: &[String], report: &mut ConfigurationSources) {
    let display = path_display(path);
    let state = match read_file(path) {
        Ok(Some(bytes)) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(document) if document.is_object() => {
                report.values.extend(keys.iter().filter_map(|key| observation(&document, key, Some(display.clone()))));
                "read"
            }
            _ => "invalid",
        },
        Ok(None) => "missing",
        Err(_) => "unreadable",
    };
    report.sources.push(Source { label: label.into(), path: Some(display), state });
}

fn read_file(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() || metadata.len() > FILE_LIMIT { return Err(io::ErrorKind::InvalidData.into()); }
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(FILE_LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > FILE_LIMIT { return Err(io::ErrorKind::InvalidData.into()); }
    Ok(Some(bytes))
}

fn repository_checkout(cwd: &Path) -> Option<PathBuf> {
    let root = cwd.ancestors().find(|path| path.join(".git").exists())?;
    let dot_git = root.join(".git");
    if dot_git.is_dir() { return Some(root.into()); }
    let git_file = String::from_utf8(read_file(&dot_git).ok()??).ok()?;
    let git_dir = root.join(git_file.trim().strip_prefix("gitdir: ")?);
    let common_file = String::from_utf8(read_file(&git_dir.join("commondir")).ok()??).ok()?;
    let common = git_dir.join(common_file.trim()).canonicalize().ok()?;
    if common.file_name().is_some_and(|name| name == ".git") { return Some(common.parent()?.into()); }
    Some(root.into())
}
