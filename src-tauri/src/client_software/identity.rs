pub(crate) mod installations;
pub(crate) mod packages;

use asb_core::contracts::AppKind;
use std::{fs, io::Read, path::Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityIssue { AccessDenied, IdentityUnavailable, CollectionFailed, UnrecognizedExecutable }

pub fn named_group(name: &str) -> Option<AppKind> {
    let name = name.to_ascii_lowercase();
    match name.strip_suffix(".exe").unwrap_or(&name) {
        "codex" => Some(AppKind::Codex),
        "claude" | "claude-code" => Some(AppKind::Claude),
        _ => None,
    }
}

pub fn candidate(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    named_group(&name).is_some() || version_name(name.strip_suffix(".exe").unwrap_or(&name))
}

pub fn normalize(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let path = path.strip_prefix("//?/").unwrap_or(&path).trim_end_matches('/');
    if cfg!(windows) { path.to_lowercase() } else { path.to_string() }
}

pub fn canonical_root(path: &Path) -> String {
    normalize(&fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()))
}

pub fn version_name(name: &str) -> bool {
    let parts: Vec<&str> = name.split('.').collect();
    parts.len() == 3 && parts.iter().all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

pub fn io_issue(error: std::io::Error) -> IdentityIssue {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => IdentityIssue::AccessDenied,
        std::io::ErrorKind::NotFound => IdentityIssue::IdentityUnavailable,
        _ => IdentityIssue::CollectionFailed,
    }
}

pub fn is_native(path: &Path) -> Result<bool, IdentityIssue> {
    let mut file = fs::File::open(path).map_err(io_issue)?;
    let mut magic = [0; 4];
    file.read_exact(&mut magic).map_err(|_| IdentityIssue::IdentityUnavailable)?;
    Ok(magic.starts_with(b"MZ") || magic == *b"\x7fELF" || matches!(magic,
        [0xfe, 0xed, 0xfa, 0xce] | [0xce, 0xfa, 0xed, 0xfe] | [0xfe, 0xed, 0xfa, 0xcf]
        | [0xcf, 0xfa, 0xed, 0xfe] | [0xca, 0xfe, 0xba, 0xbe] | [0xbe, 0xba, 0xfe, 0xca]
        | [0xca, 0xfe, 0xba, 0xbf] | [0xbf, 0xba, 0xfe, 0xca]))
}
