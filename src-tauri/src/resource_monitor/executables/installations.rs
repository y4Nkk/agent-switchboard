use super::{canonical_root, named_group, version_name};
use crate::resource_monitor::types::Group;
use std::path::PathBuf;

pub struct Installations {
    codex_bins: Vec<String>,
    claude_downloads: Vec<String>,
    claude_versions: Option<String>,
}

impl Installations {
    pub fn new() -> Self {
        let local = dirs::data_local_dir();
        let data = dirs::data_dir();
        let codex_bins = local.iter().map(|root| canonical_root(&root.join("OpenAI/Codex/bin"))).collect();
        let mut claude_downloads = Vec::new();
        for root in data.iter().chain(local.iter()) {
            claude_downloads.push(canonical_root(&root.join("Claude/claude-code")));
            if cfg!(windows) { claude_downloads.push(canonical_root(&root.join("Claude-3p/claude-code"))); }
        }
        if cfg!(windows) {
            if let Some(root) = local {
                claude_downloads.push(canonical_root(&root.join(
                    "Packages/Claude_pzs8sxrjxfjjc/LocalCache/Roaming/Claude/claude-code")));
            }
        }
        claude_downloads.sort();
        claude_downloads.dedup();
        let claude_versions = dirs::home_dir().map(|home| canonical_root(&home.join(".local/share/claude/versions")));
        Self { codex_bins, claude_downloads, claude_versions }
    }

    pub fn classify(&self, path: &str) -> Option<Group> {
        let filename = path.rsplit('/').next()?;
        if self.claude_versions.as_ref().and_then(|root| below(path, root))
            .is_some_and(|suffix| version_name(suffix.strip_suffix(".exe").unwrap_or(suffix))) {
            return Some(Group::Claude);
        }
        let group = named_group(filename)?;
        if group == Group::Codex && self.codex_bins.iter().filter_map(|root| below(path, root))
            .any(|suffix| download_entry(suffix, false)) { return Some(group); }
        if group == Group::Claude && self.claude_downloads.iter().filter_map(|root| below(path, root))
            .any(|suffix| download_entry(suffix, true)) { return Some(group); }
        bundled(path, group).then_some(group)
    }
}

pub fn launcher_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(home) = dirs::home_dir() { directories.push(home.join(".local/bin")); }
    if cfg!(windows) {
        if let Some(local) = dirs::data_local_dir() { directories.push(local.join("Microsoft/WinGet/Links")); }
    } else {
        directories.extend(["/usr/local/bin", "/opt/homebrew/bin", "/usr/bin"].map(PathBuf::from));
    }
    directories
}

fn below<'a>(path: &'a str, root: &str) -> Option<&'a str> {
    path.strip_prefix(root)?.strip_prefix('/')
}

fn hash_component(value: &str) -> bool {
    (8..=128).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn download_entry(suffix: &str, claude: bool) -> bool {
    let parts: Vec<_> = suffix.split('/').collect();
    match parts.as_slice() {
        [version, _] => if claude { version_name(version) } else { hash_component(version) },
        [version, hash, _] if claude => version_name(version) && hash_component(hash),
        _ => false,
    }
}

fn bundled(path: &str, group: Group) -> bool {
    let parts: Vec<_> = path.split('/').collect();
    let bundle = match group { Group::Codex => "Codex.app", Group::Claude => "Claude.app", _ => return false };
    let bundle_parts = if cfg!(windows) { [bundle.to_ascii_lowercase(), "contents".into(), "resources".into()] }
        else { [bundle.into(), "Contents".into(), "Resources".into()] };
    if parts.windows(3).any(|parts| parts.iter().zip(&bundle_parts).all(|(part, expected)| *part == expected.as_str())) {
        return true;
    }
    if !cfg!(windows) || group != Group::Codex { return false; }
    parts.windows(2).any(|parts| parts[0] == "windowsapps" && parts[1].starts_with("openai.codex_"))
        && parts.contains(&"resources")
}

pub fn desktop_shell(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with("/codex.app/contents/macos/codex")
        || lower.ends_with("/claude.app/contents/macos/claude") { return true; }
    let parts: Vec<_> = lower.split('/').collect();
    for (index, pair) in parts.windows(2).enumerate() {
        if pair[0] == "windowsapps" {
            let tail = &parts[index + 2..];
            if pair[1].starts_with("openai.codex_") && tail == ["app", "codex.exe"] { return true; }
            if pair[1].starts_with("claude_") && (tail == ["app", "claude.exe"] || tail == ["claude.exe"]) {
                return true;
            }
        }
        if pair[0] == "claude" && pair[1].starts_with("app-") && parts[index + 2..] == ["claude.exe"] {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests;
