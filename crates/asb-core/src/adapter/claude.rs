//! Claude Code `settings.json` adapter.
//!
//! Pure parse / preview / render over JSON text. Key order is preserved via
//! `serde_json` with the `preserve_order` feature; secrets are never written.
//! Claude Code keeps ownership of its existing login and credential
//! environment.

use crate::adapter::{AdapterError, OverlayEntry};
use crate::contracts::{
    ChangeKind, CommonSettingValue, CommonSettings, ConfigValue, KeyChange, ModelOptions,
    RouteMode, RouteState, SwitchPlan, SwitchPreview,
};
use crate::ownership::{
    is_owned, provider_absent_action, setting_specs, ProviderAbsentAction, SettingOwner,
};
use crate::redact::redact;
use crate::AppKind;
use serde_json::{Map, Value as Json};

/// Deprecated Claude Code model key superseded by
/// `ANTHROPIC_DEFAULT_HAIKU_MODEL`. It remains a profile-owned cleanup key
/// in the ownership directory so it cannot survive a provider switch.
const DEPRECATED_MODEL_KEY: &str = "env.ANTHROPIC_SMALL_FAST_MODEL";
/// Environment model key that silently overrides the top-level `model`;
/// removed whenever a profile declares a primary model.
const ENV_MODEL_KEY: &str = "env.ANTHROPIC_MODEL";

fn parse(text: &str) -> Result<Json, AdapterError> {
    let value = serde_json::from_str::<Json>(text).map_err(|e| AdapterError {
        message: "JSON 格式无效".to_string(),
        line: Some(e.line()),
    })?;
    if !value.is_object() {
        return Err(AdapterError {
            message: "settings.json 根节点必须是对象".to_string(),
            line: Some(1),
        });
    }
    Ok(value)
}

fn pointer(path: &str) -> String {
    let mut ptr = String::new();
    for seg in path.split('.') {
        ptr.push('/');
        // JSON Pointer escaping per RFC 6901.
        ptr.push_str(&seg.replace('~', "~0").replace('/', "~1"));
    }
    ptr
}

fn scalar_repr(value: &Json) -> Option<String> {
    match value {
        Json::String(s) => Some(s.clone()),
        Json::Bool(b) => Some(b.to_string()),
        Json::Number(n) => Some(n.to_string()),
        Json::Array(items) => {
            let parts: Vec<String> = items.iter().filter_map(scalar_repr).collect();
            Some(format!("[{}]", parts.join(", ")))
        }
        Json::Null => None,
        _ => None,
    }
}

fn get<'a>(root: &'a Json, path: &str) -> Option<&'a Json> {
    root.pointer(&pointer(path))
}

fn parent_conflict(path: &str, segment: &str) -> AdapterError {
    AdapterError {
        message: format!("受管路径 {path} 的父节点 {segment} 不是 JSON 对象，无法安全修改"),
        line: None,
    }
}

fn leaf_conflict(path: &str) -> AdapterError {
    AdapterError {
        message: format!("受管键 {path} 当前是 JSON 对象，无法覆盖其宿主内容"),
        line: None,
    }
}

fn validate_path(root: &Json, path: &str) -> Result<(), AdapterError> {
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current = root;
    let mut parent_path = String::new();
    for &seg in parents {
        let map = current
            .as_object()
            .ok_or_else(|| parent_conflict(path, &parent_path))?;
        if !parent_path.is_empty() {
            parent_path.push('.');
        }
        parent_path.push_str(seg);
        match map.get(seg) {
            Some(child) if child.is_object() => current = child,
            Some(_) => return Err(parent_conflict(path, &parent_path)),
            None => return Ok(()),
        }
    }
    let map = current
        .as_object()
        .ok_or_else(|| parent_conflict(path, &parent_path))?;
    if map.get(last).is_some_and(Json::is_object) {
        return Err(leaf_conflict(path));
    }
    Ok(())
}

fn set(root: &mut Json, path: &str, value: ConfigValue) -> Result<(), AdapterError> {
    validate_path(root, path)?;
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current: &mut Json = root;
    let mut parent_path = String::new();
    for &seg in parents {
        let map = current
            .as_object_mut()
            .ok_or_else(|| parent_conflict(path, &parent_path))?;
        if !parent_path.is_empty() {
            parent_path.push('.');
        }
        parent_path.push_str(seg);
        current = map
            .entry(seg.to_string())
            .or_insert_with(|| Json::Object(Map::new()));
        if !current.is_object() {
            return Err(parent_conflict(path, &parent_path));
        }
    }
    current
        .as_object_mut()
        .ok_or_else(|| parent_conflict(path, &parent_path))?
        .insert(last.to_string(), to_json(value));
    Ok(())
}

fn remove(root: &mut Json, path: &str) -> Result<(), AdapterError> {
    validate_path(root, path)?;
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current: &mut Json = root;
    for &seg in parents {
        let Some(map) = current.as_object_mut() else {
            return Err(parent_conflict(path, seg));
        };
        match map.get_mut(seg) {
            Some(child) if child.is_object() => current = child,
            Some(_) => return Err(parent_conflict(path, seg)),
            None => return Ok(()),
        }
    }
    if let Some(map) = current.as_object_mut() {
        map.remove(last);
    }
    Ok(())
}

fn to_json(value: ConfigValue) -> Json {
    match value {
        ConfigValue::Str(s) => Json::String(s),
        ConfigValue::Bool(b) => Json::Bool(b),
        ConfigValue::Number(n) => serde_json::Number::from_f64(n)
            .map(Json::Number)
            .unwrap_or(Json::Null),
        ConfigValue::Array(items) => Json::Array(items.into_iter().map(to_json).collect()),
    }
}

fn absent_provider_entry(key: &str) -> OverlayEntry {
    match provider_absent_action(AppKind::Claude, key) {
        Some(ProviderAbsentAction::Remove) => OverlayEntry::RemoveIfPresent,
        None => unreachable!("Claude provider mapping must be declared in the ownership directory"),
    }
}

fn claude_settings(
    profile: &crate::contracts::ProviderProfile,
) -> Option<&crate::contracts::ClaudeModelSettings> {
    match &profile.model_options {
        Some(ModelOptions::Claude(settings)) => Some(settings),
        None => None,
        Some(ModelOptions::Codex(_)) => {
            unreachable!("profile validation rejects mismatched options")
        }
    }
}

fn rendered_model(value: Option<&String>, one_m: bool) -> Option<ConfigValue> {
    value.map(|model| ConfigValue::Str(crate::claude_model::render_model(model, one_m)))
}

fn provider_value(profile: &crate::contracts::ProviderProfile, key: &str) -> Option<ConfigValue> {
    if profile.route_mode == crate::contracts::RouteMode::Official {
        return None;
    }
    let settings = claude_settings(profile);
    match key {
        "model" => rendered_model(
            profile.model.as_ref(),
            settings.is_some_and(|value| value.primary_one_m),
        ),
        "availableModels" => settings.and_then(|value| {
            value.available_models.as_ref().map(|models| {
                ConfigValue::Array(models.iter().cloned().map(ConfigValue::Str).collect())
            })
        }),
        "env.ANTHROPIC_BASE_URL" => profile.base_url.clone().map(ConfigValue::Str),
        "env.ANTHROPIC_AUTH_TOKEN" => Some(ConfigValue::Str(profile.api_key.clone())),
        // This older override must be removed for any current provider.
        ENV_MODEL_KEY | DEPRECATED_MODEL_KEY => None,
        "env.ANTHROPIC_DEFAULT_HAIKU_MODEL" => {
            rendered_model(settings.and_then(|value| value.haiku_model.as_ref()), false)
        }
        "env.ANTHROPIC_DEFAULT_SONNET_MODEL" => rendered_model(
            settings.and_then(|value| value.sonnet_model.as_ref()),
            settings.is_some_and(|value| value.sonnet_one_m),
        ),
        "env.ANTHROPIC_DEFAULT_OPUS_MODEL" => rendered_model(
            settings.and_then(|value| value.opus_model.as_ref()),
            settings.is_some_and(|value| value.opus_one_m),
        ),
        _ => unreachable!("Claude provider mapping must be declared in the ownership directory"),
    }
}

/// Common-setting intent is explicit: automatic removes a previously managed
/// key, while an explicit value is always written.
fn common_entry(value: &CommonSettingValue) -> OverlayEntry {
    match value {
        CommonSettingValue::Automatic => OverlayEntry::RemoveIfPresent,
        CommonSettingValue::Explicit { value } => OverlayEntry::Set(value.clone()),
    }
}

fn common_overlay(common: &CommonSettings) -> Vec<(String, OverlayEntry)> {
    setting_specs(AppKind::Claude)
        .into_iter()
        .filter(|spec| spec.owner == SettingOwner::Common)
        .map(|spec| {
            let value = common
                .value(spec.key)
                .expect("common-settings validation guarantees every catalog key");
            (spec.key.to_string(), common_entry(value))
        })
        .collect()
}

/// Builds all changes by iterating the ownership directory. Literal keys only
/// map the declared slots to plan data; ownership and cleanup actions
/// themselves remain owned by that directory.
fn overlay(plan: &SwitchPlan) -> Vec<(String, OverlayEntry)> {
    setting_specs(AppKind::Claude)
        .into_iter()
        .map(|spec| {
            let entry = match spec.owner {
                SettingOwner::Provider => provider_value(&plan.profile, spec.key)
                    .map(OverlayEntry::Set)
                    .unwrap_or_else(|| absent_provider_entry(spec.key)),
                SettingOwner::Common => {
                    let value = plan
                        .common
                        .value(spec.key)
                        .expect("plan validation guarantees complete common settings");
                    common_entry(value)
                }
                SettingOwner::Host => unreachable!("host keys never appear in the directory"),
            };
            (spec.key.to_string(), entry)
        })
        .collect()
}

pub(crate) fn check_syntax(text: &str) -> Result<(), AdapterError> {
    parse(text).map(|_| ())
}

/// Collects every owned scalar or array path and its textual value.
fn collect_owned_scalars(
    value: &Json,
    prefix: &str,
    out: &mut std::collections::BTreeMap<String, String>,
) {
    let Json::Object(map) = value else {
        return;
    };
    for (key, child) in map {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        if let Some(repr) = scalar_repr(child) {
            if is_owned(AppKind::Claude, &path) {
                out.insert(path.clone(), repr);
            }
        }
        collect_owned_scalars(child, &path, out);
    }
}

/// Owned-key diff between the live text and a previous copy.
pub(crate) fn owned_diff(current: &str, previous: &str) -> Result<Vec<KeyChange>, AdapterError> {
    let mut current_values = std::collections::BTreeMap::new();
    collect_owned_scalars(&parse(current)?, "", &mut current_values);
    let mut previous_values = std::collections::BTreeMap::new();
    collect_owned_scalars(&parse(previous)?, "", &mut previous_values);
    Ok(crate::adapter::diff_owned_maps(
        &current_values,
        &previous_values,
    ))
}

/// Reads the active routing facts from Claude settings text.
pub fn route_state(text: &str) -> RouteState {
    let root = parse(text).expect("caller validates syntax first");
    let string_at = |path: &str| get(&root, path).and_then(|v| v.as_str().map(str::to_string));
    let base_url = string_at("env.ANTHROPIC_BASE_URL");
    // env.ANTHROPIC_MODEL overrides the top-level `model` when present, so it
    // is the model that actually takes effect.
    let model = string_at(ENV_MODEL_KEY).or_else(|| string_at("model"));
    // The Haiku tier falls back to the deprecated key so old files import
    // cleanly; the adapter removes the deprecated key on the next switch.
    let haiku_model =
        string_at("env.ANTHROPIC_DEFAULT_HAIKU_MODEL").or_else(|| string_at(DEPRECATED_MODEL_KEY));
    let available_models = get(&root, "availableModels").and_then(|v| {
        v.as_array().map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<String>>()
        })
    });
    RouteState {
        app: AppKind::Claude,
        route_mode: if base_url.is_some() {
            RouteMode::Custom
        } else {
            RouteMode::Official
        },
        provider_name: None,
        model,
        base_url,
        wire_api: None,
        codex_model_options: None,
        haiku_model,
        sonnet_model: string_at("env.ANTHROPIC_DEFAULT_SONNET_MODEL"),
        opus_model: string_at("env.ANTHROPIC_DEFAULT_OPUS_MODEL"),
        available_models,
        scope_warnings: vec![],
    }
}

pub(crate) fn preview(
    current: &str,
    plan: &SwitchPlan,
    backup_dir: &str,
) -> Result<SwitchPreview, AdapterError> {
    let mut warnings = Vec::new();
    let root = parse(current)?;
    if plan.profile.model.is_some() && get(&root, ENV_MODEL_KEY).is_some() {
        warnings
            .push("settings.json 的 env.ANTHROPIC_MODEL 会覆盖 model；切换将移除该键".to_string());
    }
    preview_entries_from_root(root, overlay(plan), warnings, backup_dir)
}

fn preview_entries_from_root(
    root: Json,
    entries: Vec<(String, OverlayEntry)>,
    warnings: Vec<String>,
    backup_dir: &str,
) -> Result<SwitchPreview, AdapterError> {
    for (key, entry) in &entries {
        if !matches!(entry, OverlayEntry::Leave) {
            validate_path(&root, key)?;
        }
    }

    let mut changes = Vec::new();
    for (key, entry) in entries {
        let existing = get(&root, &key).and_then(scalar_repr);
        match entry {
            OverlayEntry::Set(value) => {
                let after = value.display();
                if existing.as_deref() != Some(after.as_str()) {
                    changes.push(KeyChange {
                        key: key.clone(),
                        kind: ChangeKind::Set,
                        before: existing.map(|b| redact(&key, &b)),
                        after: Some(redact(&key, &after)),
                    });
                }
            }
            // Absent overlay fields for the shared `env` object only leave
            // keys that this overlay never claims.
            OverlayEntry::Leave => {}
            // Owned keys are removed when present.
            OverlayEntry::RemoveIfPresent => {
                if let Some(before) = existing {
                    changes.push(KeyChange {
                        key: key.clone(),
                        kind: ChangeKind::Remove,
                        before: Some(redact(&key, &before)),
                        after: None,
                    });
                }
            }
            OverlayEntry::RemoveTableIfEmpty => {}
        }
    }

    Ok(SwitchPreview {
        app: AppKind::Claude,
        target: AppKind::Claude.config_label().to_string(),
        changes,
        warnings,
        backup_dir: backup_dir.to_string(),
    })
}

pub(crate) fn render(current: &str, plan: &SwitchPlan) -> Result<String, AdapterError> {
    render_entries(current, overlay(plan))
}

pub(crate) fn render_common_settings(common: &CommonSettings) -> Result<String, AdapterError> {
    render_entries("{}", common_overlay(common))
}

pub(crate) fn render_entries(
    current: &str,
    entries: Vec<(String, OverlayEntry)>,
) -> Result<String, AdapterError> {
    let mut root = parse(current)?;
    for (key, entry) in entries {
        match entry {
            OverlayEntry::Set(value) => set(&mut root, &key, value)?,
            OverlayEntry::Leave => {}
            OverlayEntry::RemoveIfPresent => {
                remove(&mut root, &key)?;
            }
            OverlayEntry::RemoveTableIfEmpty => {}
        }
    }
    serde_json::to_string_pretty(&root).map_err(|_| AdapterError {
        message: "配置序列化失败".to_string(),
        line: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{ClaudeModelSettings, ModelOptions, ProviderProfile};
    use crate::ownership::default_common_settings;
    use crate::test_support::CLAUDE_JSON;

    fn plan_b() -> SwitchPlan {
        SwitchPlan {
            profile: ProviderProfile {
                id: "c2".into(),
                app: AppKind::Claude,
                route_mode: crate::contracts::RouteMode::Custom,
                name: "Relay C".into(),
                model: Some("claude-opus-4".into()),
                base_url: Some("https://relay-c.internal".into()),
                api_key: "test-api-key".into(),
                model_options: Some(ModelOptions::Claude(ClaudeModelSettings {
                    primary_one_m: false,
                    haiku_model: Some("claude-haiku-4".into()),
                    sonnet_model: None,
                    sonnet_one_m: false,
                    opus_model: None,
                    opus_one_m: false,
                    available_models: Some(vec![
                        "claude-opus-4".to_string(),
                        "claude-sonnet-4".to_string(),
                    ]),
                })),
                notes: None,
                website_url: None,
                usage_query: None,
            },
            common: default_common_settings(AppKind::Claude),
        }
    }

    #[test]
    fn parses_valid_and_rejects_invalid_json_with_line() {
        assert!(parse(CLAUDE_JSON).is_ok());
        let err = parse("{\n  \"model\": oops\n}").unwrap_err();
        assert!(err.line.is_some());
    }

    #[test]
    fn rejects_non_object_root() {
        let err = parse("[1, 2]").unwrap_err();
        assert!(err.message.contains("根节点"));
    }

    #[test]
    fn preview_names_changes_warning_and_backup() {
        let preview = preview(CLAUDE_JSON, &plan_b(), "/backups").unwrap();

        let changed: Vec<&str> = preview.changes.iter().map(|c| c.key.as_str()).collect();
        assert!(changed.contains(&"model"));
        assert!(changed.contains(&"env.ANTHROPIC_BASE_URL"));
        assert!(changed.contains(&"env.ANTHROPIC_DEFAULT_HAIKU_MODEL"));
        assert!(changed.contains(&"availableModels"));
        assert!(changed.contains(&"env.ANTHROPIC_SMALL_FAST_MODEL"));
        assert_eq!(preview.backup_dir, "/backups");
        // Host keys stay untouched — asserted on the rendered text in
        // render_updates_owned_keys_and_keeps_host_blocks.
    }

    #[test]
    fn official_route_removes_managed_custom_keys_and_keeps_host_keys() {
        let mut plan = plan_b();
        plan.profile.route_mode = RouteMode::Official;
        plan.profile.model = None;
        plan.profile.base_url = None;
        plan.profile.api_key.clear();
        plan.profile.model_options = None;

        let rendered = render(CLAUDE_JSON, &plan).expect("official render");
        assert!(!rendered.contains("ANTHROPIC_AUTH_TOKEN"));
        assert!(!rendered.contains("ANTHROPIC_BASE_URL"));
        assert!(!rendered.contains("ANTHROPIC_MODEL"));
        assert!(rendered.contains("permissions"));
    }

    #[test]
    fn undeclared_tiers_are_removed_and_undeclared_lists_go_away() {
        let mut plan = plan_b();
        plan.profile.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
            primary_one_m: false,
            haiku_model: None,
            sonnet_model: Some("claude-sonnet-4".into()),
            sonnet_one_m: false,
            opus_model: None,
            opus_one_m: false,
            available_models: None,
        }));
        let current = r#"{
  "env": {
    "ANTHROPIC_BASE_URL": "https://relay-c.internal",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "old-sonnet",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "old-opus"
  },
  "availableModels": ["old-model"]
}"#;
        let rendered = render(current, &plan).unwrap();
        let parsed: Json = serde_json::from_str(&rendered).unwrap();
        assert_eq!(
            parsed["env"]["ANTHROPIC_DEFAULT_SONNET_MODEL"],
            Json::String("claude-sonnet-4".into())
        );
        assert!(parsed["env"].get("ANTHROPIC_DEFAULT_OPUS_MODEL").is_none());
        assert!(parsed.get("availableModels").is_none());
    }

    #[test]
    fn deprecated_fast_model_key_is_cleaned_up() {
        let rendered = render(CLAUDE_JSON, &plan_b()).unwrap();
        assert!(!rendered.contains("ANTHROPIC_SMALL_FAST_MODEL"));
        assert!(!rendered.contains("claude-3-5-haiku-latest"));
    }

    #[test]
    fn token_key_is_provider_owned_and_common_settings_cannot_claim_it() {
        let current = CLAUDE_JSON.replace(
            "\"ANTHROPIC_MODEL\": \"claude-sonnet-4\"",
            "\"ANTHROPIC_AUTH_TOKEN\": \"sk-live-old-secret\"",
        );
        let mut plan = plan_b();
        plan.common.settings.insert(
            "env.ANTHROPIC_AUTH_TOKEN".into(),
            CommonSettingValue::Explicit {
                value: ConfigValue::Str("sk-live-forbidden".into()),
            },
        );
        let err = crate::adapter::preview(&current, &plan, "/b").unwrap_err();
        assert!(err.message.contains("不是"));

        let preview = preview(&current, &plan_b(), "/b").unwrap();
        let change = preview
            .changes
            .iter()
            .find(|change| change.key == "env.ANTHROPIC_AUTH_TOKEN")
            .expect("provider token change");
        assert_eq!(change.before.as_deref(), Some(crate::redact::REDACTED));
        assert_eq!(change.after.as_deref(), Some(crate::redact::REDACTED));
        let rendered = render(&current, &plan_b()).unwrap();
        assert!(rendered.contains("test-api-key"));
    }

    #[test]
    fn explicit_ultracode_writes_its_own_key_and_automatic_removes_it() {
        let mut plan = plan_b();
        plan.common.settings.insert(
            "ultracode".into(),
            CommonSettingValue::Explicit {
                value: ConfigValue::Bool(true),
            },
        );
        let current = "{}";
        let rendered = render(current, &plan).unwrap();
        let parsed: Json = serde_json::from_str(&rendered).unwrap();
        assert_eq!(parsed["ultracode"], Json::Bool(true));

        // Automatic behavior leaves the file without the line.
        let mut defaulted = plan_b();
        defaulted
            .common
            .settings
            .insert("ultracode".into(), CommonSettingValue::Automatic);
        let with_line = "{\"ultracode\": true}";
        let rendered = render(with_line, &defaulted).unwrap();
        let parsed: Json = serde_json::from_str(&rendered).unwrap();
        assert!(parsed.get("ultracode").is_none());
    }

    #[test]
    fn env_parent_conflict_is_rejected_without_overwriting_host_content() {
        let current = r#"{
  "env": "host-owned scalar",
  "permissions": { "allow": ["Bash(*)"] }
}"#;

        let preview_error = preview(current, &plan_b(), "/b").unwrap_err();
        assert!(preview_error.message.contains("父节点 env"));
        let render_error = render(current, &plan_b()).unwrap_err();
        assert!(render_error.message.contains("父节点 env"));
        assert!(current.contains("host-owned scalar"));
    }

    #[test]
    fn preview_is_pure_input_text_untouched() {
        let input = CLAUDE_JSON.to_string();
        let _ = preview(&input, &plan_b(), "/b").unwrap();
        assert_eq!(input, CLAUDE_JSON);
    }

    #[test]
    fn render_updates_owned_keys_and_keeps_host_blocks() {
        let rendered = render(CLAUDE_JSON, &plan_b()).unwrap();
        let parsed: Json = serde_json::from_str(&rendered).unwrap();

        assert_eq!(parsed["model"], Json::String("claude-opus-4".into()));
        assert_eq!(
            parsed["env"]["ANTHROPIC_BASE_URL"],
            Json::String("https://relay-c.internal".into())
        );
        assert_eq!(
            parsed["env"]["ANTHROPIC_DEFAULT_HAIKU_MODEL"],
            Json::String("claude-haiku-4".into())
        );
        assert_eq!(
            parsed["availableModels"],
            Json::Array(vec![
                Json::String("claude-opus-4".into()),
                Json::String("claude-sonnet-4".into())
            ])
        );
        // Host blocks survive.
        assert_eq!(
            parsed["permissions"]["allow"][0],
            Json::String("Bash(npm run test:*)".into())
        );
        assert!(parsed["statusLine"]["command"].is_string());
        // No credential material was written.
        let text = rendered.to_ascii_lowercase();
        assert!(!text.contains("sk-"));
        assert!(!text.contains("claude-relay-c"));
    }

    #[test]
    fn render_writes_canonical_one_m_markers_for_supported_model_slots() {
        let mut plan = plan_b();
        plan.profile.model = Some("claude-opus-4-7".into());
        plan.profile.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
            primary_one_m: true,
            haiku_model: Some("claude-haiku-4".into()),
            sonnet_model: Some("claude-sonnet-4-6".into()),
            sonnet_one_m: true,
            opus_model: Some("claude-opus-4-7".into()),
            opus_one_m: true,
            available_models: None,
        }));

        let rendered = render(CLAUDE_JSON, &plan).unwrap();
        let parsed: Json = serde_json::from_str(&rendered).unwrap();
        assert_eq!(parsed["model"], Json::String("claude-opus-4-7[1m]".into()));
        assert_eq!(
            parsed["env"]["ANTHROPIC_DEFAULT_SONNET_MODEL"],
            Json::String("claude-sonnet-4-6[1m]".into())
        );
        assert_eq!(
            parsed["env"]["ANTHROPIC_DEFAULT_OPUS_MODEL"],
            Json::String("claude-opus-4-7[1m]".into())
        );
        assert_eq!(
            parsed["env"]["ANTHROPIC_DEFAULT_HAIKU_MODEL"],
            Json::String("claude-haiku-4".into())
        );
    }

    #[test]
    fn render_removes_env_model_override_when_primary_model_is_set() {
        let rendered = render(CLAUDE_JSON, &plan_b()).unwrap();
        let parsed: Json = serde_json::from_str(&rendered).unwrap();
        assert!(parsed["env"].get("ANTHROPIC_MODEL").is_none());
        assert_eq!(parsed["model"], Json::String("claude-opus-4".into()));
    }

    #[test]
    fn render_creates_env_object_when_missing() {
        let current = "{\n  \"model\": \"m\"\n}";
        let rendered = render(current, &plan_b()).unwrap();
        let parsed: Json = serde_json::from_str(&rendered).unwrap();
        assert!(parsed["env"]["ANTHROPIC_BASE_URL"].is_string());
        assert_eq!(parsed["model"], Json::String("claude-opus-4".into()));
    }

    #[test]
    fn render_is_idempotent() {
        let once = render(CLAUDE_JSON, &plan_b()).unwrap();
        let twice = render(&once, &plan_b()).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn route_state_reads_model_base_url_and_mode() {
        let state = route_state(CLAUDE_JSON);
        assert_eq!(state.route_mode, RouteMode::Custom);
        assert_eq!(state.model.as_deref(), Some("claude-sonnet-4"));
        assert_eq!(state.base_url.as_deref(), Some("https://relay-a.internal"));
        assert!(state.provider_name.is_none());
    }

    #[test]
    fn route_state_prefers_env_model_and_reports_official_without_endpoint() {
        let with_env_model = CLAUDE_JSON.replace(
            "\"ANTHROPIC_MODEL\": \"claude-sonnet-4\"",
            "\"ANTHROPIC_MODEL\": \"claude-opus-4\"",
        );
        assert_eq!(
            route_state(&with_env_model).model.as_deref(),
            Some("claude-opus-4")
        );

        let official = CLAUDE_JSON.replace(
            "    \"ANTHROPIC_BASE_URL\": \"https://relay-a.internal\",\n",
            "",
        );
        let state = route_state(&official);
        assert_eq!(state.route_mode, RouteMode::Official);
        assert!(state.base_url.is_none());
    }
}
