use super::{directories, json, name};
use crate::client_software::{identity, types::{Form, Installation, Manager, Owner}};
use asb_core::contracts::AppKind;
use std::path::{Path, PathBuf};
#[cfg(test)] mod tests;

pub fn identify(item: &mut Installation) -> Result<(), String> {
    let normalized = identity::normalize(&item.executable);
    if identity::installations::Installations::new().is_host(&normalized)
        || identity::packages::editor_root(&item.executable).map_err(|e| format!("{e:?}"))?.is_some() {
        item.form = Form::Bundled;
        item.manager = Manager::Host;
        item.detail = Some("Update this engine through its desktop application or editor".into());
        return Ok(());
    }
    if npm(item)? || brew(item)? || native(item)? { return Ok(()); }
    #[cfg(windows)]
    if super::super::winget::attribute(item)? { return Ok(()); }
    if normalized.starts_with("/usr/") || normalized.starts_with("/opt/") {
        item.manager = Manager::System;
        item.detail = Some("Use the system package manager with the original installation scope".into());
    }
    Ok(())
}

fn npm(item: &mut Installation) -> Result<bool, String> {
    let package = match item.app { AppKind::Codex => "@openai/codex", AppKind::Claude => "@anthropic-ai/claude-code" };
    let mut candidates: Vec<_> = item.executable.ancestors().skip(1).take(8).map(Path::to_path_buf).collect();
    candidates.extend(item.executable.ancestors().skip(1).take(8).filter(|root|
        root.file_name().is_some_and(|name| name == "node_modules")).map(|root| root.join(package)));
    if let Some(parent) = item.launcher.parent() { candidates.push(parent.join("node_modules").join(package)); }
    for root in candidates {
        let Some(manifest) = json(&root.join("package.json"))? else { continue; };
        if manifest["name"].as_str() != Some(package) { continue; }
        let root = std::fs::canonicalize(&root).map_err(|e| e.to_string())?;
        let Some(modules) = root.parent().and_then(Path::parent).filter(|p| p.file_name().is_some_and(|n| n == "node_modules")) else { continue; };
        let Some(parent) = modules.parent() else { continue; };
        let prefix = if parent.file_name().is_some_and(|n| n == "lib") { parent.parent().unwrap_or(parent) } else { parent };
        let entry = manifest["bin"][name(item.app)].as_str().or_else(|| manifest["bin"].as_str())
            .ok_or("Package has no declared CLI entry")?;
        let executable = std::fs::canonicalize(root.join(entry)).map_err(|e| e.to_string())?;
        if !executable.starts_with(&root) && !declared_native_entry(&manifest, modules, &executable, item.app)? {
            return Err("Package entry is outside its installation".into());
        }
        item.root = root.clone();
        item.executable = executable;
        item.manager = Manager::Npm;
        if another_package_manager(&root, modules) {
            item.manager = Manager::Manual;
            item.detail = Some("This package belongs to another package manager; update it through that manager".into());
            return Ok(true);
        }
        if let Some((node, npm)) = npm_runtime(prefix) {
            item.owner = Some(Owner::Npm { node, npm, prefix: prefix.into(), package: package.into(), registry: None });
        }
        if item.owner.is_none() { item.detail = Some("The owning Node/npm runtime could not be resolved".into()); }
        return Ok(true);
    }
    Ok(false)
}

fn declared_native_entry(manifest: &serde_json::Value, modules: &Path, entry: &Path, app: AppKind) -> Result<bool, String> {
    let Some(dependencies) = manifest["optionalDependencies"].as_object() else { return Ok(false); };
    for package in dependencies.keys().filter(|name| identity::packages::platform_package(name, app)) {
        let root = modules.join(package);
        if root.is_dir() && entry.starts_with(std::fs::canonicalize(root).map_err(|e| e.to_string())?) {
            return Ok(identity::packages::classify(entry).ok().flatten() == Some(app));
        }
    }
    Ok(false)
}

fn another_package_manager(root: &Path, modules: &Path) -> bool {
    let path = identity::normalize(root);
    modules.join(".modules.yaml").exists() || ["/.pnpm/", "/pnpm/global/", "/.bun/", "/yarn/global/", "/.yarn/", "/.vite-plus/"].iter().any(|part| path.contains(part))
}

fn npm_runtime(prefix: &Path) -> Option<(PathBuf, PathBuf)> {
    let mut candidates = vec![prefix.to_path_buf(), prefix.join("bin")];
    candidates.extend(directories());
    for directory in candidates {
        let node = directory.join(if cfg!(windows) { "node.exe" } else { "node" });
        if !node.is_file() { continue; }
        let roots = [directory.join("node_modules/npm"), directory.join("../lib/node_modules/npm")];
        for root in roots {
            let npm = root.join("bin/npm-cli.js");
            if npm.is_file() { return Some((std::fs::canonicalize(node).ok()?, std::fs::canonicalize(npm).ok()?)); }
        }
    }
    None
}

fn native(item: &mut Installation) -> Result<bool, String> {
    if item.app == AppKind::Claude {
        return Ok(native_claude(item, &crate::local_state::user_home_dir()?));
    }
    let Some(root) = item.executable.ancestors().find(|path| path.file_name().is_some_and(|s| s == "standalone")
        && path.parent().and_then(Path::file_name).is_some_and(|s| s == "packages")) else { return Ok(false); };
    if identity::packages::classify(&item.executable).ok().flatten() != Some(AppKind::Codex) { return Ok(false); }
    item.root = root.into();
    item.manager = Manager::Native;
    native_owner(item);
    Ok(true)
}

fn native_claude(item: &mut Installation, home: &Path) -> bool {
    let versions = home.join(".local/share/claude/versions");
    let root = identity::canonical_root(&versions);
    let executable = identity::canonical_root(&item.executable);
    let version_entry = identity::installations::claude_version_entry(&executable, &root);
    if executable.starts_with(&format!("{root}/")) && !version_entry { return false; }
    let standard = home.join(".local/bin").join(if cfg!(windows) { "claude.exe" } else { "claude" });
    if !version_entry && identity::normalize(&item.launcher) != identity::normalize(&standard) { return false; }
    if !identity::is_native(&item.executable).unwrap_or(false) { return false; }
    item.root = versions;
    item.manager = Manager::Native;
    native_owner(item);
    true
}

fn native_owner(item: &mut Installation) {
    let root = identity::canonical_root(&item.root);
    // Resolve directory aliases without following the stable launcher's own link.
    let payload = item.launcher.parent().is_none_or(|directory| {
        let directory = identity::canonical_root(directory);
        directory == root || directory.starts_with(&format!("{root}/"))
    });
    if payload {
        item.owner = None;
        item.detail = Some("Select the stable installed launcher, not a version-specific package payload".into());
    } else { item.owner = Some(Owner::Native); item.detail = None; }
}

fn brew(item: &mut Installation) -> Result<bool, String> {
    for root in item.executable.ancestors() {
        if root.file_name().is_none_or(|n| n != "Caskroom") { continue; }
        let prefix = root.parent().ok_or("Missing Homebrew prefix")?;
        let relative = item.executable.strip_prefix(root).map_err(|e| e.to_string())?;
        let cask = relative.components().next().ok_or("Missing cask")?.as_os_str().to_string_lossy();
        let allowed = match item.app { AppKind::Codex => cask == "codex", AppKind::Claude => matches!(cask.as_ref(), "claude-code" | "claude-code@latest") };
        if !allowed { return Ok(false); }
        let brew = prefix.join("bin/brew");
        item.root = root.join(cask.as_ref());
        item.manager = Manager::Homebrew;
        item.channel = if cask == "claude-code" { "stable" } else { "latest" }.into();
        let receipt = json(&item.root.join(".metadata/INSTALL_RECEIPT.json"))?;
        let installed = relative.components().nth(1).map(|part| part.as_os_str().to_string_lossy().into_owned());
        let owned = receipt.as_ref().is_some_and(|receipt| receipt["source"]["tap"].as_str() == Some("homebrew/cask")
            && receipt["source"]["version"].as_str() == installed.as_deref());
        if brew.is_file() && owned { item.owner = Some(Owner::Homebrew { brew, cask: cask.into_owned() }); }
        else { item.detail = Some("Homebrew's installed cask receipt could not confirm the original source and version".into()); }
        return Ok(true);
    }
    Ok(false)
}
