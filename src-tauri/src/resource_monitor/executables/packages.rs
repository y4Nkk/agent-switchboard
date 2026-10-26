use super::{io_issue, named_group, normalize};
use crate::resource_monitor::types::{Group, Issue};
use serde::{de::DeserializeOwned, Deserialize};
use std::{fs, io::Read, path::Path};

#[derive(Deserialize)]
struct Package { #[serde(default)] name: String, #[serde(default)] publisher: Option<String> }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativePackage { layout_version: u32, variant: String, entrypoint: String }

pub fn classify(path: &Path) -> Result<Option<Group>, Issue> {
    let Some(filename) = path.file_name().and_then(|name| name.to_str()) else { return Ok(None); };
    let Some(group) = named_group(filename) else { return Ok(None); };
    let Some(directory) = path.parent() else { return Ok(None); };
    for root in directory.ancestors().take(6) {
        let relative = normalize(path.strip_prefix(root).map_err(|_| Issue::IdentityUnavailable)?);
        if group == Group::Codex && matches!(relative.as_str(), "bin/codex" | "bin/codex.exe") {
            if let Some(native) = manifest::<NativePackage>(&root.join("codex-package.json"))? {
                return if native.layout_version == 1 && native.variant == "codex"
                    && normalize(Path::new(&native.entrypoint)) == relative { Ok(Some(group)) }
                    else { Err(Issue::UnrecognizedExecutable) };
            }
        }
        let Some(package) = manifest::<Package>(&root.join("package.json"))? else { continue; };
        if owns_entry(&package, &relative, group) { return Ok(Some(group)); }
    }
    Ok(None)
}

fn manifest<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, Issue> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_issue(error)),
    };
    let mut bytes = Vec::new();
    file.take(262_145).read_to_end(&mut bytes).map_err(io_issue)?;
    if bytes.len() > 262_144 { return Err(Issue::IdentityUnavailable); }
    serde_json::from_slice(&bytes).map(Some).map_err(|_| Issue::IdentityUnavailable)
}

fn owns_entry(package: &Package, relative: &str, group: Group) -> bool {
    let parts: Vec<_> = relative.split('/').collect();
    let publisher = package.publisher.as_deref().unwrap_or_default();
    match group {
        Group::Codex if package.name == "@openai/codex" => matches!(parts.as_slice(),
            ["vendor", _, "bin", _]),
        Group::Codex if package.name == "chatgpt" && publisher.eq_ignore_ascii_case("openai") =>
            matches!(parts.as_slice(), ["bin", _, _]),
        Group::Claude if package.name == "@anthropic-ai/claude-code" =>
            matches!(parts.as_slice(), ["bin", _]),
        Group::Claude if claude_platform_package(&package.name) => parts.len() == 1,
        Group::Claude if package.name == "claude-code" && publisher.eq_ignore_ascii_case("anthropic") =>
            matches!(parts.as_slice(), ["resources", "native-binary", _]),
        _ => false,
    }
}

fn claude_platform_package(name: &str) -> bool {
    name.strip_prefix("@anthropic-ai/claude-code-").is_some_and(|platform| matches!(platform,
        "darwin-arm64" | "darwin-x64" | "linux-x64" | "linux-arm64" | "linux-x64-musl"
        | "linux-arm64-musl" | "win32-x64" | "win32-arm64"))
}
