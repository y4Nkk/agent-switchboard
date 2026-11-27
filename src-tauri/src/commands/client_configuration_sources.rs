//! Read-only settings evidence from the selected local client and directory.
mod app_server;
mod claude;
mod codex;

use super::error::{blocking, state, CommandError};
use crate::{client_software::{discovery, types::Installation}, local_state::LocalState};
use asb_core::{AppKind, adapter::ClientConfigurationChanges, contracts::SettingValue};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationSources {
    directory: Option<String>,
    installation: Option<String>,
    version: Option<String>,
    supported_settings: Vec<String>,
    sources: Vec<Source>,
    values: Vec<ObservedSetting>,
    requirements: Vec<ObservedSetting>,
    profile_options: Vec<String>,
    profiles_verified: bool,
    profile_owned_by_fragment: bool,
    windows_sandbox: Option<String>,
    issue: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    label: String,
    path: Option<String>,
    state: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ObservedSetting {
    key: String,
    value: String,
    source: Option<String>,
}

#[tauri::command]
pub async fn inspect_client_configuration_sources(
    app: AppHandle, target: AppKind, directory: Option<String>,
) -> Result<ConfigurationSources, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let cwd = directory.as_deref().map(project_directory).transpose().map_err(source_error)?;
        let mut report = ConfigurationSources { directory: cwd.as_deref().map(path_display), ..Default::default() };
        let installation = resolve_installation(target);
        if let Ok(installation) = &installation {
            report.installation = Some(path_display(&installation.launcher));
            report.version.clone_from(&installation.version);
            report.supported_settings = observable_keys(target).into_iter().filter(|key| {
                asb_core::ownership::minimum_client_version(target, key)
                    .and_then(|version| semver::Version::parse(version).ok())
                    .is_some_and(|minimum| supports(installation, &minimum))
            }).collect();
        }
        let result = match target {
            AppKind::Codex => installation.and_then(|item| codex::inspect(&state, &item, cwd.as_deref(), &mut report)),
            AppKind::Claude => {
                claude::inspect(&state, cwd.as_deref(), &mut report).map_err(source_error)?;
                installation.map(|_| ())
            }
        };
        if let Err(error) = result { report.issue = Some(error); }
        if report.issue.is_none() {
            report.issue = report.version.is_none().then(|| "The local CLI did not report a recognizable version".into());
        }
        Ok(report)
    }).await
}

pub(super) fn validate_client_changes(
    state: &LocalState, target: AppKind, changes: &ClientConfigurationChanges, directory: Option<&str>,
) -> Result<(), CommandError> {
    if changes.permission_profile.is_some() && codex::fragment_selects_profile(state).map_err(source_error)? {
        return Err(source_error("The saved Codex common fragment owns default_permissions".into()));
    }
    if let Some(SettingValue::Explicit { value: asb_core::contracts::ConfigValue::Str(name) }) = &changes.permission_profile {
        let cwd = directory.map(project_directory).transpose().map_err(source_error)?;
        let installation = resolve_installation(AppKind::Codex).map_err(source_error)?;
        return codex::validate_profile(state, &installation, cwd.as_deref(), name).map_err(source_error);
    }
    let minimum = changes.settings.iter().filter_map(|(key, value)| {
        if matches!(value, SettingValue::Automatic) { return None; }
        asb_core::ownership::minimum_client_version(target, key)
    }).filter_map(|version| semver::Version::parse(version).ok()).max();
    if let Some(minimum) = minimum {
        let installation = resolve_installation(target).map_err(source_error)?;
        if !supports(&installation, &minimum) {
            return Err(source_error(format!("Claude Code ≥ {minimum} is required for the selected file permission settings")));
        }
    }
    Ok(())
}

fn resolve_installation(app: AppKind) -> Result<Installation, String> {
    let mut item = discovery::resolve(app)?;
    discovery::read_version(&mut item);
    Ok(item)
}

fn supports(installation: &Installation, minimum: &semver::Version) -> bool {
    installation.version.as_deref().and_then(|value| semver::Version::parse(value).ok())
        .is_some_and(|version| &version >= minimum)
}

fn project_directory(path: &str) -> Result<PathBuf, String> {
    let path = Path::new(path);
    if !path.is_absolute() || !path.is_dir() { return Err("Choose an existing absolute working directory".into()); }
    path.canonicalize().map(|path| crate::client_software::command_path(&path))
        .map_err(|error| format!("Working directory: {error}"))
}

fn path_display(path: &Path) -> String { path.to_string_lossy().into_owned() }

fn source_error(message: String) -> CommandError { CommandError::new("client-config-sources", message) }

fn value_at<'a>(document: &'a Value, key: &str) -> Option<&'a Value> {
    key.split('.').try_fold(document, |value, segment| value.get(segment)).filter(|value| !value.is_null())
}

fn observable_keys(app: AppKind) -> Vec<String> {
    let mut keys: Vec<String> = asb_core::ownership::setting_specs(app).into_iter()
        .filter(|spec| spec.owner == asb_core::ownership::SettingOwner::Client)
        .map(|spec| spec.key.into()).collect();
    if app == AppKind::Codex {
        keys.extend(asb_core::CodexSubagentKey::ALL.into_iter().map(|key| key.path()));
        keys.push(asb_core::adapter::codex::DEFAULT_PERMISSIONS_KEY.into());
    }
    keys.sort_unstable();
    keys
}

fn observation(document: &Value, key: &str, source: Option<String>) -> Option<ObservedSetting> {
    let value = value_at(document, key)?;
    let mut wrapped = serde_json::Map::new();
    wrapped.insert(key.into(), value.clone());
    let safe = asb_switch::display_content(AppKind::Claude, &Value::Object(wrapped).to_string());
    let redacted: Value = serde_json::from_str(&safe).ok()?;
    Some(ObservedSetting { key: key.into(), value: redacted.get(key)?.to_string(), source })
}
