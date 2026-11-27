use super::{ConfigurationSources, Source, app_server::AppServer, observable_keys, observation, path_display, supports};
use crate::{client_software::types::Installation, local_state::LocalState};
use asb_core::{AppKind, adapter::codex::{BUILTIN_PERMISSION_PROFILES, DEFAULT_PERMISSIONS_KEY}};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::{BTreeMap, BTreeSet}, path::Path};

const REQUIREMENT_KEYS: &[&str] = &[
    "allowedApprovalPolicies", "allowedApprovalsReviewers", "allowedSandboxModes",
    "allowedPermissionProfiles", "defaultPermissions", "allowedWindowsSandboxImplementations",
    "allowMxc", "requireMxc", "featureRequirements",
];

#[derive(Deserialize)]
struct Config {
    config: Value,
    origins: BTreeMap<String, Origin>,
    layers: Vec<Layer>,
}

#[derive(Deserialize)]
struct Origin { name: Value }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Layer { name: Value, config: Value, disabled_reason: Option<String> }

struct Policy {
    allowed: Option<BTreeMap<String, bool>>,
    locked: Option<String>,
}

impl Policy {
    fn read(value: &Value) -> Result<Self, String> {
        let requirements = value.get("requirements").ok_or("Codex response has no requirements field")?;
        let field = |name: &str| requirements.get(name).filter(|entry| !entry.is_null()).cloned();
        let allowed = field("allowedPermissionProfiles").map(serde_json::from_value)
            .transpose().map_err(|_| "Invalid Codex permission profile allowlist")?;
        let locked = field("defaultPermissions").map(serde_json::from_value)
            .transpose().map_err(|_| "Invalid Codex managed default permissions")?;
        Ok(Self { allowed, locked })
    }

    fn permits(&self, name: &str) -> bool {
        self.locked.as_deref().is_none_or(|locked| locked == name)
            && self.allowed.as_ref().is_none_or(|list| list.get(name) == Some(&true))
    }
}

pub(super) fn inspect(state: &LocalState, item: &Installation, cwd: Option<&Path>, report: &mut ConfigurationSources) -> Result<(), String> {
    let mut server = start(state, item)?;
    let config = read_config(&mut server, cwd)?;
    report.sources = config.layers.iter().map(|layer| Source {
        label: layer_type(&layer.name).into(),
        path: layer_path(&layer.name),
        state: if layer.disabled_reason.is_some() { "disabled" } else { "loaded" },
    }).collect();
    report.values = observable_keys(AppKind::Codex).iter().filter_map(|key| {
        let origin = config.origins.get(key);
        observation(&config.config, key, origin.map(|origin| layer_path(&origin.name).unwrap_or_else(|| layer_type(&origin.name).into())))
            .or_else(|| profile_observation(&config, key, origin))
    }).collect();
    let requirements = server.call("configRequirements/read", json!({}));
    if cfg!(windows) {
        report.windows_sandbox = Some(match server.call("windowsSandbox/readiness", Value::Null) {
            Ok(value) => match value.get("status").and_then(Value::as_str) {
                Some(status @ ("ready" | "notConfigured" | "updateRequired")) => status.into(),
                _ => "unavailable".into(),
            },
            Err(_) => "unavailable".into(),
        });
    }
    let requirements = requirements?;
    report.requirements = REQUIREMENT_KEYS.iter().filter_map(|key| {
        observation(requirements.get("requirements")?, key, None)
    }).collect();
    report.profile_owned_by_fragment = fragment_selects_profile(state)?;
    if supports_profiles(item) {
        let policy = Policy::read(&requirements)?;
        report.profile_options = profile_options(&config, &policy);
        report.profiles_verified = true;
    }
    Ok(())
}

pub(super) fn fragment_selects_profile(state: &LocalState) -> Result<bool, String> {
    let fragment = crate::codex_common::view_fragment(state.root())?;
    let parsed = fragment.text.parse::<toml_edit::DocumentMut>().map_err(|_| "Codex common fragment is invalid")?;
    Ok(parsed.contains_key(DEFAULT_PERMISSIONS_KEY))
}

pub(super) fn validate_profile(state: &LocalState, item: &Installation, cwd: Option<&Path>, name: &str) -> Result<(), String> {
    if !supports_profiles(item) { return Err("Codex ≥ 0.138.0 is required to verify permission profile policy".into()); }
    let mut server = start(state, item)?;
    let config = read_config(&mut server, cwd)?;
    let policy = Policy::read(&server.call("configRequirements/read", json!({}))?)?;
    if !profile_options(&config, &policy).iter().any(|allowed| allowed == name) {
        return Err("The profile is unavailable outside project scope or restricted by Codex managed policy".into());
    }
    if config.layers.iter().any(has_other_legacy_sandbox) && policy.allowed.is_none() {
        return Err("An active Codex configuration layer outside the user file sets a legacy sandbox. Remove that setting before selecting a permission profile.".into());
    }
    Ok(())
}

fn start(state: &LocalState, item: &Installation) -> Result<AppServer, String> {
    let target = state.target(AppKind::Codex)?;
    let safe = target.ancestors().skip(1).find(|path| path.is_dir()).ok_or("Codex configuration directory is unavailable")?;
    AppServer::start(item, safe)
}

fn read_config(server: &mut AppServer, cwd: Option<&Path>) -> Result<Config, String> {
    let result = server.call("config/read", json!({"cwd": cwd.map(path_display), "includeLayers": true}))?;
    serde_json::from_value(result).map_err(|error| format!("Codex config/read schema: {error}"))
}

fn supports_profiles(item: &Installation) -> bool { supports(item, &semver::Version::new(0, 138, 0)) }

fn is_base_layer(layer: &Layer) -> bool {
    layer.disabled_reason.is_none() && !matches!(layer_type(&layer.name), "project" | "sessionFlags")
        && layer.name.get("profile").is_none_or(Value::is_null)
}

fn profile_options(config: &Config, policy: &Policy) -> Vec<String> {
    let mut names: BTreeSet<String> = BUILTIN_PERMISSION_PROFILES.iter().map(|name| (*name).into()).collect();
    for layer in config.layers.iter().filter(|layer| is_base_layer(layer)) {
        if let Some(profiles) = layer.config.get("permissions").and_then(Value::as_object) {
            names.extend(profiles.iter().filter(|(_, value)| value.is_object()).map(|(name, _)| name.clone()));
        }
    }
    names.into_iter().filter(|name| policy.permits(name)).collect()
}

fn has_other_legacy_sandbox(layer: &Layer) -> bool {
    let user_base = layer_type(&layer.name) == "user" && layer.name.get("profile").is_none_or(Value::is_null);
    !user_base && layer.disabled_reason.is_none()
        && ["sandbox_mode", "sandbox_workspace_write"].iter().any(|key| layer.config.get(*key).is_some_and(|value| !value.is_null()))
}

fn profile_observation(config: &Config, key: &str, origin: Option<&Origin>) -> Option<super::ObservedSetting> {
    if key != DEFAULT_PERMISSIONS_KEY { return None; }
    let origin = origin?;
    let layer = config.layers.iter().find(|layer| layer.name == origin.name && layer.disabled_reason.is_none())?;
    observation(&layer.config, key, layer_path(&layer.name).or_else(|| Some(layer_type(&layer.name).into())))
}

fn layer_type(name: &Value) -> &str { name.get("type").and_then(Value::as_str).unwrap_or("unknown") }

fn layer_path(name: &Value) -> Option<String> {
    if let Some(file) = name.get("file").and_then(Value::as_str) { return Some(file.into()); }
    if let Some(folder) = name.get("dotCodexFolder").and_then(Value::as_str) {
        return Some(path_display(&Path::new(folder).join("config.toml")));
    }
    name.get("name").or_else(|| name.get("domain")).and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_options_require_both_known_global_names_and_managed_permission() {
        let config: Config = serde_json::from_value(json!({"config":{},"origins":{},"layers":[
            {"name":{"type":"user"},"config":{"permissions":{"mine":{},"denied":{}}}},
            {"name":{"type":"project"},"config":{"permissions":{"project-only":{}},"sandbox_mode":"read-only"}},
            {"name":{"type":"system"},"config":{"permissions":{"inactive":{}}},"disabledReason":"test"}
        ]})).unwrap();
        let policy = Policy::read(&json!({"requirements":{"allowedPermissionProfiles":{
            "mine":true, "denied":false, "project-only":true, "inactive":true, "unknown":true, ":workspace":true
        }}})).unwrap();
        assert_eq!(profile_options(&config, &policy), [":workspace".to_string(), "mine".to_string()]);
        assert!(config.layers.iter().any(has_other_legacy_sandbox));
    }
}
