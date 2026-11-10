use super::{process, types::Owner};
use crate::outbound_proxy::OutboundProxySettings;
use std::{path::Path, process::Command, time::Duration};

fn runtime(node: &Path, npm: &Path, prefix: &Path, proxy: &OutboundProxySettings) -> Result<Command, String> {
    let mut command = Command::new(node);
    command.arg(process::command_path(npm)).arg("--global").arg("--prefix").arg(process::command_path(prefix))
        .current_dir(crate::local_state::user_home_dir()?);
    let mut paths = vec![node.parent().ok_or("Node directory unavailable")?.to_path_buf()];
    paths.extend(std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default());
    command.env("PATH", std::env::join_paths(paths).map_err(|e| e.to_string())?);
    super::updater::apply_proxy(&mut command, proxy);
    Ok(command)
}

pub fn registry(node: &Path, npm: &Path, prefix: &Path, package: &str) -> Result<String, String> {
    let proxy = OutboundProxySettings::default();
    let scope = package.split('/').next().ok_or("npm scope unavailable")?;
    let mut command = runtime(node, npm, prefix, &proxy)?;
    let scoped = process::output(command.args(["config", "get", &format!("{scope}:registry")]), Duration::from_secs(10))?;
    let registry = if matches!(scoped.trim(), "undefined" | "null" | "") {
        process::output(runtime(node, npm, prefix, &proxy)?.args(["config", "get", "registry"]), Duration::from_secs(10))?
    } else { scoped };
    let url = reqwest::Url::parse(registry.trim()).map_err(|_| "npm registry is not a valid URL")?;
    if !matches!(url.scheme(), "https" | "http") || url.host_str().is_none() {
        return Err("npm registry protocol is unsupported".into());
    }
    Ok(url.to_string())
}

pub fn command(owner: &Owner, proxy: &OutboundProxySettings) -> Result<Command, String> {
    let Owner::Npm { node, npm, prefix, registry, .. } = owner else { return Err("npm owner unavailable".into()); };
    let mut command = runtime(node, npm, prefix, proxy)?;
    command.arg("--registry").arg(registry.as_ref().ok_or("The original npm registry could not be confirmed")?);
    Ok(command)
}

pub fn metadata(owner: &Owner, version: &str, proxy: &OutboundProxySettings) -> Result<serde_json::Value, String> {
    let Owner::Npm { package, .. } = owner else { return Err("npm owner unavailable".into()); };
    let output = process::output(command(owner, proxy)?.args(["view", &format!("{package}@{version}"), "name", "version", "engines", "--json"]), Duration::from_secs(20))?;
    serde_json::from_str(&output).map_err(|_| "npm returned invalid version metadata".into())
}

pub fn source(owner: &Owner) -> String {
    let Owner::Npm { registry, package, .. } = owner else { return "npm".into(); };
    let Some(registry) = registry else { return "npm".into(); };
    let Ok(mut url) = reqwest::Url::parse(registry) else { return "npm".into(); };
    let _ = url.set_username(""); let _ = url.set_password(None);
    format!("{url} · {package}")
}

pub fn check_node(owner: &Owner, target: &str, proxy: &OutboundProxySettings) -> Result<(), String> {
    let Owner::Npm { node, package, .. } = owner else { return Err("npm owner unavailable".into()); };
    let metadata = metadata(owner, target, proxy)?;
    if metadata["name"].as_str() != Some(package) || metadata["version"].as_str() != Some(target) {
        return Err("Target npm package metadata does not match the preview".into());
    }
    let requirement = metadata["engines"]["node"].as_str().ok_or("Target Node requirement unavailable")?;
    let constraint = semver::VersionReq::parse(requirement).map_err(|_| "The target Node requirement needs manual verification")?;
    let output = process::output(Command::new(node).arg("--version"), Duration::from_secs(5))?;
    let version = process::version(&output).ok_or("Cannot read the owning Node version")?;
    if !constraint.matches(&semver::Version::parse(&version).map_err(|e| e.to_string())?) {
        return Err(format!("Update the owning Node runtime first: installed {version}, required {requirement}"));
    }
    Ok(())
}
