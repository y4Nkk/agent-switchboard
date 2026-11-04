use super::{plan::Prepared, process, releases, types::{Installation, Manager, Owner, UpdateTask}};
use crate::{local_state::LocalState, outbound_proxy::{OutboundProxyMode, OutboundProxySettings}};
use asb_core::contracts::AppKind;
use std::{path::{Path, PathBuf}, process::Command};

pub enum Updater { Process(Command), Winget { id: String, scope: String, root: PathBuf } }

pub fn prepare(state: &LocalState, plan: &Prepared) -> Result<Updater, String> {
    let item = &plan.installation;
    let mut command = match item.owner.as_ref().ok_or("Installation manager unavailable")? {
        owner @ Owner::Npm { package, .. } => {
            super::npm::check_node(owner, &plan.view.target_version, &plan.proxy)?;
            let mut command = super::npm::command(owner, &plan.proxy)?;
            command.args(["install", "--engine-strict", "--no-audit", "--no-fund"])
                .arg(format!("{package}@{}", plan.view.target_version));
            command
        }
        Owner::Homebrew { brew, cask } => {
            let mut command = Command::new(brew);
            command.args(["upgrade", "--cask", "--greedy", cask]);
            command.env("HOMEBREW_NO_INSTALL_CLEANUP", "1");
            command
        }
        Owner::Native if item.app == AppKind::Claude => {
            let mut command = super::discovery::launch::command(item)?;
            command.arg("update");
            command
        }
        Owner::Native => codex(state, plan)?,
        Owner::Winget { package, scope } => return Ok(Updater::Winget { id: package.clone(), scope: scope.clone(), root: item.root.clone() }),
    };
    if item.manager == Manager::Native {
        let mut paths = vec![item.launcher.parent().ok_or("Launcher directory unavailable")?.to_path_buf()];
        paths.extend(std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default());
        command.env("PATH", std::env::join_paths(paths).map_err(|e| e.to_string())?);
    }
    command.current_dir(crate::local_state::user_home_dir()?);
    apply_proxy(&mut command, &plan.proxy);
    process::quiet(&mut command);
    Ok(Updater::Process(command))
}

fn codex(state: &LocalState, plan: &Prepared) -> Result<Command, String> {
    let directory = state.root().join("client-software").join("jobs").join(&plan.view.id);
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let name = if cfg!(windows) { "install.ps1" } else { "install.sh" };
    let bytes = releases::download(&format!("https://chatgpt.com/codex/{name}"), 1_048_576)?;
    let path = directory.join(name);
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    let mut command = if cfg!(windows) {
        let system = std::env::var_os("SystemRoot").ok_or("Windows system directory unavailable")?;
        let mut command = Command::new(PathBuf::from(system).join("System32/WindowsPowerShell/v1.0/powershell.exe"));
        let wrapper = directory.join("invoke.ps1");
        std::fs::write(&wrapper, include_str!("updater/invoke-codex.ps1")).map_err(|e| e.to_string())?;
        command.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"]).arg(wrapper)
            .env("ASB_CODEX_INSTALLER", &path).env("ASB_CODEX_RELEASE", &plan.view.target_version)
            .env("ASB_UPDATE_PROXY_MODE", format!("{:?}", plan.proxy.mode).to_lowercase())
            .env("ASB_UPDATE_PROXY_URL", plan.proxy.url.as_deref().unwrap_or_default());
        command
    } else {
        let mut command = Command::new("/bin/sh");
        command.arg(&path).arg("--release").arg(&plan.view.target_version);
        command
    };
    let codex_home = plan.installation.root.parent().and_then(Path::parent).ok_or("Codex root unavailable")?;
    command.env("CODEX_NON_INTERACTIVE", "1").env("CODEX_HOME", codex_home)
        .env("CODEX_INSTALL_DIR", plan.installation.launcher.parent().ok_or("Codex launcher directory unavailable")?)
        .env_remove("CODEX_INSTALL_DAEMON_ONLY").env_remove("CODEX_INSTALL_DEFER_SELECTION")
        .env_remove("CODEX_INSTALL_IF_CURRENT").env_remove("CODEX_INSTALL_IF_LATEST");
    Ok(command)
}

pub fn check_proxy(item: &Installation, proxy: &OutboundProxySettings) -> Result<(), String> {
    if cfg!(windows) && item.app == AppKind::Codex && item.manager == Manager::Native
        && proxy.mode == OutboundProxyMode::Manual
        && proxy.url.as_deref().is_some_and(|url| url.starts_with("socks")) {
        return Err("The official Windows Codex installer supports an HTTP proxy. Select an HTTP proxy or update it through the official instructions.".into());
    }
    Ok(())
}

pub(super) fn apply_proxy(command: &mut Command, proxy: &OutboundProxySettings) {
    let keys = ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"];
    match proxy.mode {
        OutboundProxyMode::System => {}
        OutboundProxyMode::None => {
            for key in keys { command.env_remove(key); }
            command.env("NO_PROXY", "*").env("no_proxy", "*")
                .env("npm_config_proxy", "").env("npm_config_https_proxy", "").env("npm_config_noproxy", "*");
        }
        OutboundProxyMode::Manual => {
            for key in keys { command.env(key, proxy.url.as_deref().unwrap_or_default()); }
            command.env("NO_PROXY", "").env("no_proxy", "")
                .env("npm_config_proxy", proxy.url.as_deref().unwrap_or_default())
                .env("npm_config_https_proxy", proxy.url.as_deref().unwrap_or_default()).env("npm_config_noproxy", "");
        }
    }
}

pub fn log(task: &mut UpdateTask, text: &str, proxy: &OutboundProxySettings) {
    let redacted = if let Some(url) = &proxy.url { process::redact_url(text, url) } else { text.into() };
    let clean = process::scrub(&redacted);
    for line in clean.lines().filter(|line| !line.trim().is_empty()) { task.log.push(line.chars().take(512).collect()); }
    if task.log.len() > 100 { task.log.drain(..task.log.len() - 100); }
}

pub fn cleanup(state: &LocalState, id: &str) {
    if uuid::Uuid::parse_str(id).is_err() { return; }
    let directory = state.root().join("client-software/jobs").join(id);
    for name in ["install.ps1", "install.sh", "invoke.ps1"] {
        if let Err(error) = std::fs::remove_file(directory.join(name)) {
            if error.kind() != std::io::ErrorKind::NotFound { log::warn!("Cannot remove completed updater input: {error}"); }
        }
    }
    if directory.is_dir() { if let Err(error) = std::fs::remove_dir(directory) { log::warn!("Cannot remove updater work directory: {error}"); } }
}
