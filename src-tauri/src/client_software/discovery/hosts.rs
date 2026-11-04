use super::{identity_id, json};
use crate::client_software::types::{Form, Installation, Manager};
use asb_core::contracts::AppKind;
use std::path::{Path, PathBuf};

pub fn collect(items: &mut Vec<Installation>, issues: &mut Vec<String>) {
    if let Ok(home) = crate::local_state::user_home_dir() {
        for editor in [".vscode", ".vscode-insiders", ".cursor", ".windsurf"] {
            let directory = home.join(editor).join("extensions");
            if let Err(error) = extensions(&directory, items) { issues.push(format!("{}: {error}", directory.display())); }
        }
        #[cfg(target_os = "macos")]
        for directory in [PathBuf::from("/Applications"), home.join("Applications")] {
            macos_apps(&directory, items);
        }
    }
    #[cfg(windows)] {
        if let Err(error) = windows_apps(items) { issues.push(format!("Desktop packages: {error}")); }
        windows_uninstall(items);
    }
    let mut ids = std::collections::HashSet::new();
    items.retain(|item| ids.insert(item.id.clone()));
}

fn extensions(directory: &Path, items: &mut Vec<Installation>) -> Result<(), String> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    for entry in entries.take(2048) {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
        if !name.starts_with("openai.chatgpt-") && !name.starts_with("anthropic.claude-code-") { continue; }
        let Some(manifest) = json(&path.join("package.json"))? else { continue; };
        let publisher = manifest["publisher"].as_str().unwrap_or_default().to_ascii_lowercase();
        let app = match (publisher.as_str(), manifest["name"].as_str()) {
            ("openai", Some("chatgpt")) => AppKind::Codex,
            ("anthropic", Some("claude-code")) => AppKind::Claude,
            _ => continue,
        };
        items.push(host(app, Form::Extension, path, manifest["version"].as_str().map(str::to_string)));
    }
    Ok(())
}

fn host(app: AppKind, form: Form, root: PathBuf, version: Option<String>) -> Installation {
    Installation { id: identity_id(app, form, &root), app, form, manager: Manager::Host,
        launcher: root.clone(), executable: root.clone(), root, version, version_text: None, active: false,
        channel: "host".into(), detail: None, owner: None }
}

#[cfg(windows)]
fn windows_apps(items: &mut Vec<Installation>) -> Result<(), String> {
    use windows::{core::HSTRING, Management::Deployment::PackageManager};
    let _com = crate::client_software::winget::Apartment::new()?;
    let packages = PackageManager::new().and_then(|manager|
        manager.FindPackagesByUserSecurityId(&HSTRING::new())).map_err(|e| e.to_string())?;
    for package in packages {
        let id = package.Id().map_err(|e| e.to_string())?;
        let name = id.Name().map_err(|e| e.to_string())?.to_string();
        let app = match name.as_str() { "OpenAI.Codex" => AppKind::Codex, "Claude" => AppKind::Claude, _ => continue };
        let root = package.InstalledLocation().and_then(|p| p.Path()).map_err(|e| e.to_string())?;
        let v = id.Version().map_err(|e| e.to_string())?;
        items.push(host(app, Form::Desktop, PathBuf::from(root.to_string()), Some(format!("{}.{}.{}.{}", v.Major, v.Minor, v.Build, v.Revision))));
    }
    Ok(())
}

#[cfg(windows)]
fn windows_uninstall(items: &mut Vec<Installation>) {
    use winreg::{enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE}, RegKey};
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let Ok(parent) = RegKey::predef(hive).open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall") else { continue; };
        for name in parent.enum_keys().flatten() {
            let Ok(key) = parent.open_subkey(name) else { continue; };
            let Ok(display) = key.get_value::<String, _>("DisplayName") else { continue; };
            let app = match display.as_str() { "Claude" => AppKind::Claude, "Codex" => AppKind::Codex, _ => continue };
            let Ok(root) = key.get_value::<String, _>("InstallLocation") else { continue; };
            if !root.is_empty() { items.push(host(app, Form::Desktop, root.into(), key.get_value("DisplayVersion").ok())); }
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_apps(directory: &Path, items: &mut Vec<Installation>) {
    use crate::client_software::process;
    use std::{process::Command, time::Duration};
    for (name, app) in [("Codex.app", AppKind::Codex), ("Claude.app", AppKind::Claude)] {
        let path = directory.join(name);
        if !path.is_dir() { continue; }
        let version = process::output(Command::new("/usr/libexec/PlistBuddy")
            .args(["-c", "Print :CFBundleShortVersionString"]).arg(path.join("Contents/Info.plist")), Duration::from_secs(3))
            .ok().map(|s| s.trim().to_string());
        items.push(host(app, Form::Desktop, path, version));
    }
}
