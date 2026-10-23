use super::{observation::Observation, types::{Group, Issue}};
use std::{collections::HashMap, fs, io::Read, path::Path};

pub struct Executables { paths: HashMap<String, Group>, claude_versions: Option<String> }

pub fn candidate(name: &str) -> Option<Group> {
    let name = name.to_ascii_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    if name == "codex" || name.starts_with("codex-") { return Some(Group::Codex); }
    if name == "claude" || name == "claude-code" || version_name(name) { return Some(Group::Claude); }
    None
}

pub fn outside_scope(process: &Observation) -> bool {
    process.executable.as_deref().is_some_and(|path| desktop_shell(&normalize(path)))
}

impl Executables {
    pub fn new() -> Self {
        let mut paths = HashMap::new();
        if let Some(path) = std::env::var_os("PATH") {
            for directory in std::env::split_paths(&path).filter(|path| path.is_absolute()) {
                for (name, group) in [("codex", Group::Codex), ("claude", Group::Claude)] {
                    let filename = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
                    if let Ok(path) = fs::canonicalize(directory.join(filename)) {
                        paths.insert(normalize(&path), group);
                    }
                }
            }
        }
        let claude_versions = dirs::home_dir().map(|home| normalize(&home.join(".local/share/claude/versions")));
        Self { paths, claude_versions }
    }

    pub fn classify(&self, process: &Observation) -> Result<Option<Group>, Issue> {
        let Some(group) = candidate(&process.name) else { return Ok(None); };
        let path = process.executable.as_deref().ok_or(Issue::IdentityUnavailable)?;
        let normalized = normalize(path);
        if desktop_shell(&normalized) { return Ok(None); }
        let known = self.paths.get(&normalized).copied().or_else(|| packaged(&normalized))
            .or_else(|| self.claude_versions.as_ref().and_then(|root| {
                normalized.strip_prefix(&format!("{root}/"))
                    .filter(|suffix| !suffix.contains('/'))
                    .filter(|suffix| version_name(suffix.trim_end_matches(".exe")))
                    .map(|_| Group::Claude)
            }));
        if known != Some(group) { return Err(Issue::UnrecognizedExecutable); }
        if !is_native(path)? { return Err(Issue::UnrecognizedExecutable); }
        Ok(known)
    }
}

fn normalize(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let normalized = normalized.strip_prefix("//?/").unwrap_or(&normalized).trim_end_matches('/');
    if cfg!(windows) { normalized.to_lowercase() } else { normalized.to_string() }
}

fn version_name(name: &str) -> bool {
    let parts: Vec<&str> = name.split('.').collect();
    parts.len() == 3 && parts.iter().all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn packaged(path: &str) -> Option<Group> {
    let lower = path.to_ascii_lowercase();
    let segments: Vec<&str> = lower.split('/').collect();
    let name = segments.last().copied()?.trim_end_matches(".exe");
    for pair in segments.windows(2) {
        if pair[0] == "@openai" && (pair[1] == "codex" || pair[1].starts_with("codex-")) && name == "codex" {
            return Some(Group::Codex);
        }
        if pair[0] == "@anthropic-ai" && (pair[1] == "claude-code" || pair[1].starts_with("claude-code-"))
            && name == "claude" { return Some(Group::Claude); }
    }
    let bundled_codex = name == "codex" && (lower.contains("/codex.app/contents/resources/")
        || (lower.contains("/windowsapps/openai.codex_") && lower.contains("/resources/")));
    if bundled_codex { return Some(Group::Codex); }
    let bundled_claude = matches!(name, "claude" | "claude-code")
        && lower.contains("/claude.app/contents/resources/");
    if bundled_claude { return Some(Group::Claude); }
    None
}

fn desktop_shell(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with("/codex.app/contents/macos/codex")
        || lower.ends_with("/claude.app/contents/macos/claude")
        || (lower.contains("/windowsapps/openai.codex_") && lower.ends_with("/app/codex.exe"))
}

fn is_native(path: &Path) -> Result<bool, Issue> {
    let mut file = fs::File::open(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::PermissionDenied => Issue::AccessDenied,
        std::io::ErrorKind::NotFound => Issue::IdentityUnavailable,
        _ => Issue::CollectionFailed,
    })?;
    let mut magic = [0; 4];
    file.read_exact(&mut magic).map_err(|_| Issue::IdentityUnavailable)?;
    Ok(magic.starts_with(b"MZ") || magic == *b"\x7fELF" || matches!(magic,
        [0xfe, 0xed, 0xfa, 0xce] | [0xce, 0xfa, 0xed, 0xfe] | [0xfe, 0xed, 0xfa, 0xcf]
        | [0xcf, 0xfa, 0xed, 0xfe] | [0xca, 0xfe, 0xba, 0xbe] | [0xbe, 0xba, 0xfe, 0xca]
        | [0xca, 0xfe, 0xba, 0xbf] | [0xbf, 0xba, 0xfe, 0xca]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_packages_and_desktop_shell_have_distinct_scope() {
        assert_eq!(packaged("/tools/node_modules/@openai/codex-linux-x64/vendor/bin/codex"), Some(Group::Codex));
        assert_eq!(packaged("/tools/node_modules/@anthropic-ai/claude-code/claude.exe"), Some(Group::Claude));
        assert!(desktop_shell("/Applications/Codex.app/Contents/MacOS/Codex"));
        assert_eq!(candidate("node.exe"), None);
        assert_eq!(candidate("2.1.275.exe"), Some(Group::Claude));
        assert_eq!(packaged("/unrelated/@openai/codex-fake/not-codex"), None);
    }
}
