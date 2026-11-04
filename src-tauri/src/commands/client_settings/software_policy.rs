//! Read-only projection; the client settings executor remains the only writer.
use crate::{client_software::types::Policy, local_state::LocalState};
use asb_core::contracts::AppKind;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub(crate) fn read(state: &LocalState, app: AppKind) -> Policy {
    if app == AppKind::Codex { return Policy { channel: "latest".into(), ..Default::default() }; }
    match resolve(state) {
        Ok(policy) => policy,
        Err(error) => Policy { channel: "latest".into(), issue: Some(error), ..Default::default() },
    }
}

fn resolve(state: &LocalState) -> Result<Policy, String> {
    let path = state.target(AppKind::Claude)?;
    let mut settings = document(&path)?;
    let managed = managed_sources()?;
    let merge = managed.first().and_then(|s| s.get("managedSourcesBehavior")).and_then(Value::as_str) == Some("merge");
    if merge {
        for source in managed.iter().rev() { merge_settings(&mut settings, source.clone()); }
    } else if let Some(source) = managed.iter().find(|source| source.keys().any(|key|
        !matches!(key.as_str(), "managedSourcesBehavior" | "wslInheritsWindowsSettings"))) {
        merge_settings(&mut settings, source.clone());
    }
    let unknown = settings.contains_key("policyHelper") || settings.contains_key("forceRemoteSettingsRefresh");
    let minimum = bound(setting_version(&settings, "minimumVersion")?, setting_version(&settings, "requiredMinimumVersion")?, true);
    let maximum = setting_version(&settings, "requiredMaximumVersion")?;
    let channel = settings.get("autoUpdatesChannel").and_then(Value::as_str).unwrap_or("latest");
    if !matches!(channel, "latest" | "stable") { return Err("Unrecognized Claude update channel".into()); }
    let policy = Policy { channel: channel.into(), automatic_disabled: disabled(&settings, "DISABLE_AUTOUPDATER")?,
        updates_disabled: disabled(&settings, "DISABLE_UPDATES")?, minimum, maximum,
        issue: unknown.then(|| "Dynamic managed policy must be checked in Claude /status before updating".into()),
        revision: String::new() };
    let mut policy = policy;
    policy.revision = asb_switch::sha256_hex(&serde_json::to_string(&policy).map_err(|e| e.to_string())?);
    Ok(policy)
}

fn merge_settings(settings: &mut Map<String, Value>, mut source: Map<String, Value>) {
    match source.remove("env") {
        Some(Value::Object(environment)) => {
            let current = settings.entry("env").or_insert_with(|| Value::Object(Map::new()));
            if let Some(current) = current.as_object_mut() { current.extend(environment); }
            else { *current = Value::Object(environment); }
        }
        Some(value) => { source.insert("env".into(), value); }
        None => {}
    }
    settings.extend(source);
}

fn disabled(settings: &Map<String, Value>, key: &str) -> Result<bool, String> {
    let process = std::env::var(key).ok();
    let environment = settings.get("env").map(|value| value.as_object().ok_or("Invalid update policy environment")).transpose()?;
    let configured = environment.and_then(|env| env.get(key)).map(|v| v.as_str().ok_or("Invalid update policy flag")).transpose()?;
    let mut disabled = false;
    for value in [process.as_deref(), configured].into_iter().flatten() {
        match value.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" => disabled = true,
            "" | "0" | "false" | "no" => {}
            _ => return Err(format!("Unrecognized {key} update policy value")),
        }
    }
    Ok(disabled)
}

fn setting_version(settings: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    let Some(value) = settings.get(key) else { return Ok(None); };
    let version = value.as_str().ok_or_else(|| format!("Invalid {key}"))?;
    semver::Version::parse(version).map_err(|_| format!("Invalid {key}"))?;
    Ok(Some(version.into()))
}

fn bound(current: Option<String>, next: Option<String>, floor: bool) -> Option<String> {
    match (current, next) {
        (Some(a), Some(b)) => {
            let newer = semver::Version::parse(&a).unwrap() > semver::Version::parse(&b).unwrap();
            Some(if newer == floor { a } else { b })
        }
        (a, b) => a.or(b),
    }
}

fn document(path: &Path) -> Result<Map<String, Value>, String> {
    let Some(value) = crate::client_software::discovery::json(path)? else { return Ok(Map::new()); };
    value.as_object().cloned().ok_or_else(|| format!("Invalid settings object: {}", path.display()))
}

fn managed_sources() -> Result<Vec<Map<String, Value>>, String> {
    let mut sources = Vec::new();
    #[cfg(windows)] { if let Some(value) = registry(true)? { sources.push(value); } }
    #[cfg(target_os = "macos")] {
        let path = Path::new("/Library/Managed Preferences/com.anthropic.claudecode.plist");
        if path.exists() { return Err("Claude has an MDM policy; verify effective update policy with its administrator".into()); }
    }
    let root = managed_directory();
    let mut files = document(&root.join("managed-settings.json"))?;
    let fragments = root.join("managed-settings.d");
    match std::fs::read_dir(&fragments) {
        Ok(entries) => {
            let mut paths: Vec<_> = entries.map(|entry| entry.map(|entry| entry.path())).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
            paths.sort();
            for path in paths {
                if path.extension().is_some_and(|ext| ext == "json") && !path.file_name().unwrap().to_string_lossy().starts_with('.') {
                    merge_settings(&mut files, document(&path)?);
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("Managed policy directory: {error}")),
    }
    if !files.is_empty() { sources.push(files); }
    #[cfg(windows)] { if sources.is_empty() { if let Some(value) = registry(false)? { sources.push(value); } } }
    Ok(sources)
}

fn managed_directory() -> PathBuf {
    #[cfg(windows)] { std::env::var_os("ProgramFiles").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Program Files")).join("ClaudeCode") }
    #[cfg(target_os = "macos")] { PathBuf::from("/Library/Application Support/ClaudeCode") }
    #[cfg(target_os = "linux")] { PathBuf::from("/etc/claude-code") }
}

#[cfg(windows)]
fn registry(machine: bool) -> Result<Option<Map<String, Value>>, String> {
    use winreg::{enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE}, RegKey};
    let hive = if machine { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER };
    let key = match RegKey::predef(hive).open_subkey(r"SOFTWARE\Policies\ClaudeCode") {
        Ok(key) => key,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Managed registry policy: {e}")),
    };
    let raw: String = match key.get_value("Settings") {
        Ok(value) => value,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Managed registry settings: {e}")),
    };
    serde_json::from_str(&raw).map(Some).map_err(|_| "Invalid managed registry settings".into())
}
