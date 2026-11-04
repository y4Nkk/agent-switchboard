mod hosts;
mod ownership;
pub(crate) mod launch;

use super::{identity, process, types::{Form, Installation, Manager}};
use asb_core::contracts::AppKind;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs, path::{Path, PathBuf}, time::Duration};

pub fn scan(selected: &[(AppKind, PathBuf)]) -> (Vec<Installation>, Vec<String>) {
    let mut installations = Vec::new();
    let mut issues = Vec::new();
    let mut seen = HashSet::new();
    for directory in directories() {
        for app in [AppKind::Codex, AppKind::Claude] {
            for filename in filenames(app) {
                let launcher = directory.join(filename);
                if !launcher.is_file() { continue; }
                observe(app, &launcher, &mut seen, &mut installations, &mut issues);
            }
        }
    }
    for (app, path) in selected { observe(*app, path, &mut seen, &mut installations, &mut issues); }
    hosts::collect(&mut installations, &mut issues);
    for app in [AppKind::Codex, AppKind::Claude] {
        if let Ok(selected) = resolve(app) {
            for item in installations.iter_mut().filter(|i| i.app == app) { item.active = item.id == selected.id; }
        }
    }
    (installations, issues)
}

fn observe(app: AppKind, launcher: &Path, seen: &mut HashSet<String>, installations: &mut Vec<Installation>, issues: &mut Vec<String>) {
    match inspect_identity(app, launcher) {
        Ok(mut item) if seen.insert(item.id.clone()) => {
            read_version(&mut item);
            if let Err(error) = check_source(&mut item) { item.detail = Some(process::scrub(&error)); }
            installations.push(item);
        }
        Ok(_) => {}
        Err(error) => issues.push(format!("{}: {}", launcher.display(), process::scrub(&error))),
    }
}

pub fn check_source(item: &mut Installation) -> Result<(), String> {
    if let Some(super::types::Owner::Npm { node, npm, prefix, package, registry }) = item.owner.as_mut() {
        *registry = Some(super::npm::registry(node, npm, prefix, package)?);
    }
    Ok(())
}

pub fn validate_selected(app: AppKind, path: &Path) -> Result<(), String> {
    let item = inspect_identity(app, path)?;
    let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    if item.owner.is_none() && identity::named_group(filename) != Some(app) {
        return Err("Select the requested client's verified executable or launcher".into());
    }
    Ok(())
}

pub fn inspect(app: AppKind, launcher: &Path) -> Result<Installation, String> {
    let mut item = inspect_identity(app, launcher)?;
    read_version(&mut item);
    Ok(item)
}

fn read_version(item: &mut Installation) {
    if item.form == Form::Cli {
        let output = launch::command(item).and_then(|mut command| {
            command.arg("--version");
            if let Ok(home) = crate::local_state::user_home_dir() { command.current_dir(home); }
            process::output(&mut command, Duration::from_secs(5))
        });
        match output.and_then(|text| {
            item.version_text = Some(process::scrub(&text).chars().take(512).collect());
            process::version(&text).ok_or_else(|| "Unrecognized version response".to_string())
        }) {
            Ok(version) => item.version = Some(version),
            Err(error) => item.detail = Some(process::scrub(&error)),
        }
    }
}

pub fn process_roots(item: &Installation) -> Result<Vec<PathBuf>, String> {
    let mut roots = vec![item.root.clone()];
    if item.manager != Manager::Npm { return Ok(roots); }
    let manifest = json(&item.root.join("package.json"))?.ok_or("Owning package metadata disappeared")?;
    let modules = item.root.parent().and_then(Path::parent).ok_or("Package scope unavailable")?;
    if let Some(dependencies) = manifest["optionalDependencies"].as_object() {
        for name in dependencies.keys().filter(|name| identity::packages::platform_package(name, item.app)) {
            for path in [item.root.join("node_modules").join(name), modules.join(name)] {
                if path.is_dir() { roots.push(std::fs::canonicalize(path).map_err(|e| e.to_string())?); }
            }
        }
    }
    Ok(roots)
}

pub fn resolve(app: AppKind) -> Result<Installation, String> {
    #[cfg(test)]
    if app == AppKind::Codex {
        if let Some(path) = std::env::var_os("ASB_TEST_CODEX_BIN") {
            let path = PathBuf::from(path);
            let mut item = inspect_identity(app, &path)?;
            item.active = true;
            return Ok(item);
        }
    }
    for directory in directories() {
        for filename in filenames(app) {
            let path = directory.join(filename);
            if path.is_file() {
                let mut item = inspect_identity(app, &path)?;
                if item.form != Form::Cli { continue; }
                item.active = true;
                return Ok(item);
            }
        }
    }
    Err(format!("{} CLI is not installed in the ASB launch environment", name(app)))
}

fn inspect_identity(app: AppKind, launcher: &Path) -> Result<Installation, String> {
    let mut item = Installation { id: String::new(), app, form: Form::Cli, manager: Manager::Manual,
        launcher: launcher.into(), executable: fs::canonicalize(launcher).map_err(|e| e.to_string())?,
        root: launcher.parent().ok_or("Missing parent")?.into(), version: None, version_text: None, active: false,
        channel: "latest".into(), detail: None, owner: None };
    if identity::installations::desktop_shell(&identity::normalize(&item.executable)) {
        item.form = Form::Desktop; item.manager = Manager::Host;
        item.id = identity_id(app, item.form, &item.root);
        return Ok(item);
    }
    ownership::identify(&mut item)?;
    item.id = identity_id(app, item.form, &item.root);
    Ok(item)
}

pub fn directories() -> Vec<PathBuf> {
    let mut result: Vec<_> = std::env::var_os("PATH").map(|path|
        std::env::split_paths(&path).filter(|p| p.is_absolute()).collect()).unwrap_or_default();
    result.extend(identity::installations::launcher_directories());
    if let Some(path) = std::env::var_os("CODEX_INSTALL_DIR").map(PathBuf::from).filter(|p| p.is_absolute()) { result.push(path); }
    if let Some(local) = dirs::data_local_dir() {
        result.push(local.join("Programs/OpenAI/Codex/bin"));
    }
    if let Some(roaming) = dirs::data_dir() { if cfg!(windows) { result.push(roaming.join("npm")); } }
    let mut seen = HashSet::new();
    result.retain(|p| seen.insert(identity::normalize(p)));
    result
}

fn filenames(app: AppKind) -> Vec<String> {
    if cfg!(windows) { vec![format!("{}.exe", name(app)), format!("{}.cmd", name(app))] }
    else { vec![name(app).into()] }
}

pub fn name(app: AppKind) -> &'static str {
    match app { AppKind::Codex => "codex", AppKind::Claude => "claude" }
}

pub fn identity_id(app: AppKind, form: Form, root: &Path) -> String {
    format!("{:x}", Sha256::digest(format!("{app:?}:{form:?}:{}", identity::normalize(root))))
}

pub fn json(path: &Path) -> Result<Option<serde_json::Value>, String> {
    use std::io::Read;
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(1_048_577).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > 1_048_576 { return Err("Metadata exceeds size limit".into()); }
    serde_json::from_slice(&bytes).map(Some).map_err(|e| e.to_string())
}
