//! Claude Code `settings.json` adapter.
//!
//! Pure parse / preview / render over JSON text. Key order is preserved via
//! `serde_json` with the `preserve_order` feature; secrets are never written.
//! Claude Code keeps ownership of its existing login and credential
//! environment.

use crate::adapter::{AdapterError, OverlayEntry};
use crate::contracts::{
    ChangeKind, CommonConfigPatch, KeyChange, ModelOptions, PatchValue, RouteMode, RouteState,
    SwitchPlan, SwitchPreview,
};
use crate::ownership::is_owned;
use crate::redact::redact;
use crate::AppKind;
use serde_json::{Map, Value as Json};

/// Deprecated Claude Code model key superseded by
/// `ANTHROPIC_DEFAULT_HAIKU_MODEL`. The adapter cleans it up on every
/// switch; the ownership table never claims it.
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

fn set(root: &mut Json, path: &str, value: PatchValue) {
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current: &mut Json = root;
    for &seg in parents {
        if !current.is_object() {
            *current = Json::Object(Map::new());
        }
        let map = current.as_object_mut().expect("object");
        current = map
            .entry(seg.to_string())
            .or_insert_with(|| Json::Object(Map::new()));
        if !current.is_object() {
            *current = Json::Object(Map::new());
        }
    }
    if !current.is_object() {
        *current = Json::Object(Map::new());
    }
    current
        .as_object_mut()
        .expect("object")
        .insert(last.to_string(), to_json(value));
}

fn remove(root: &mut Json, path: &str) {
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current: &mut Json = root;
    for &seg in parents {
        let Some(map) = current.as_object_mut() else {
            return;
        };
        match map.get_mut(seg) {
            Some(child) if child.is_object() => current = child,
            _ => return,
        }
    }
    if let Some(map) = current.as_object_mut() {
        map.remove(last);
    }
}

fn to_json(value: PatchValue) -> Json {
    match value {
        PatchValue::Str(s) => Json::String(s),
        PatchValue::Bool(b) => Json::Bool(b),
        PatchValue::Number(n) => serde_json::Number::from_f64(n)
            .map(Json::Number)
            .unwrap_or(Json::Null),
        PatchValue::Array(items) => Json::Array(items.into_iter().map(to_json).collect()),
    }
}

fn tier_entry(key: &str, value: &Option<String>, one_m: bool) -> (String, OverlayEntry) {
    match value {
        Some(v) => (
            key.to_string(),
            OverlayEntry::Set(PatchValue::Str(crate::claude_model::render_model(v, one_m))),
        ),
        None => (key.to_string(), OverlayEntry::RemoveIfPresent),
    }
}

/// Builds the overlay entries implied by `plan`.
fn overlay(plan: &SwitchPlan) -> (Vec<(String, OverlayEntry)>, Vec<String>) {
    let p = &plan.profile;
    let mut warnings = Vec::new();

    // Every profile routes to a custom endpoint; validation rejects a
    // profile without one, the defensive arm keeps old stores loadable.
    let mut entries = vec![
        (
            "env.ANTHROPIC_BASE_URL".to_string(),
            match &p.base_url {
                Some(url) => OverlayEntry::Set(PatchValue::Str(url.clone())),
                None => OverlayEntry::RemoveIfPresent,
            },
        ),
        (
            "env.ANTHROPIC_AUTH_TOKEN".to_string(),
            OverlayEntry::Set(PatchValue::Str(p.api_key.clone())),
        ),
    ];

    // The deprecated key is superseded by ANTHROPIC_DEFAULT_HAIKU_MODEL and
    // is cleaned up on every switch.
    entries.push((
        DEPRECATED_MODEL_KEY.to_string(),
        OverlayEntry::RemoveIfPresent,
    ));

    let claude_settings = match &p.model_options {
        Some(ModelOptions::Claude(settings)) => Some(settings),
        _ => None,
    };
    if let Some(settings) = claude_settings {
        entries.push(tier_entry(
            "env.ANTHROPIC_DEFAULT_HAIKU_MODEL",
            &settings.haiku_model,
            false,
        ));
        entries.push(tier_entry(
            "env.ANTHROPIC_DEFAULT_SONNET_MODEL",
            &settings.sonnet_model,
            settings.sonnet_one_m,
        ));
        entries.push(tier_entry(
            "env.ANTHROPIC_DEFAULT_OPUS_MODEL",
            &settings.opus_model,
            settings.opus_one_m,
        ));
        entries.push(match &settings.available_models {
            Some(models) => (
                "availableModels".to_string(),
                OverlayEntry::Set(PatchValue::Array(
                    models.iter().cloned().map(PatchValue::Str).collect(),
                )),
            ),
            None => ("availableModels".to_string(), OverlayEntry::RemoveIfPresent),
        });
    }

    // env.ANTHROPIC_MODEL silently overrides the top-level `model`; when a
    // profile declares a primary model, the override must go for the switch
    // to take effect.
    entries.push(match &p.model {
        Some(m) => {
            warnings.push(
                "settings.json 的 env.ANTHROPIC_MODEL 会覆盖 model；切换将移除该键".to_string(),
            );
            (
                "model".to_string(),
                OverlayEntry::Set(PatchValue::Str(crate::claude_model::render_model(
                    m,
                    claude_settings.map_or(false, |settings| settings.primary_one_m),
                ))),
            )
        }
        None => ("model".to_string(), OverlayEntry::Leave),
    });
    entries.push((
        ENV_MODEL_KEY.to_string(),
        match &p.model {
            Some(_) => OverlayEntry::RemoveIfPresent,
            None => OverlayEntry::Leave,
        },
    ));

    for entry in &plan.common.entries {
        entries.push((
            entry.key.clone(),
            match &entry.value {
                Some(value) => OverlayEntry::Set(value.clone()),
                None => OverlayEntry::RemoveIfPresent,
            },
        ));
    }
    (entries, warnings)
}

/// Overlay entries for a general-settings-only apply: exactly the patch's
/// own lines, with no profile routing.
pub(crate) fn common_overlay(
    common: &CommonConfigPatch,
) -> (Vec<(String, OverlayEntry)>, Vec<String>) {
    let entries = common
        .entries
        .iter()
        .map(|entry| {
            (
                entry.key.clone(),
                match &entry.value {
                    Some(value) => OverlayEntry::Set(value.clone()),
                    None => OverlayEntry::RemoveIfPresent,
                },
            )
        })
        .collect();
    (entries, vec![])
}

pub(crate) fn check_syntax(text: &str) -> Result<(), AdapterError> {
    parse(text).map(|_| ())
}

/// Textual value at an owned dotted path, for toggle-state reads.
pub(crate) fn parse_owned_scalar(text: &str, key: &str) -> Result<Option<String>, AdapterError> {
    let root = parse(text)?;
    Ok(get(&root, key).and_then(scalar_repr))
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
    let (entries, warnings) = overlay(plan);
    preview_entries(current, entries, warnings, backup_dir)
}

pub(crate) fn preview_entries(
    current: &str,
    entries: Vec<(String, OverlayEntry)>,
    warnings: Vec<String>,
    backup_dir: &str,
) -> Result<SwitchPreview, AdapterError> {
    let root = parse(current)?;

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
            // Owned keys and the deprecated key are removed when present.
            OverlayEntry::RemoveIfPresent | OverlayEntry::RemoveTableIfPresent => {
                if let Some(before) = existing {
                    changes.push(KeyChange {
                        key: key.clone(),
                        kind: ChangeKind::Remove,
                        before: Some(redact(&key, &before)),
                        after: None,
                    });
                }
            }
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
    let (entries, _) = overlay(plan);
    render_entries(current, entries)
}

pub(crate) fn render_entries(
    current: &str,
    entries: Vec<(String, OverlayEntry)>,
) -> Result<String, AdapterError> {
    let mut root = parse(current)?;
    for (key, entry) in entries {
        match entry {
            OverlayEntry::Set(value) => set(&mut root, &key, value),
            OverlayEntry::Leave => {}
            OverlayEntry::RemoveIfPresent | OverlayEntry::RemoveTableIfPresent => {
                remove(&mut root, &key);
            }
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
    use crate::contracts::{
        ClaudeModelSettings, CommonConfigPatch, ModelOptions, PatchEntry, ProviderProfile,
    };
    use crate::test_support::CLAUDE_JSON;

    fn plan_b() -> SwitchPlan {
        SwitchPlan {
            app: AppKind::Claude,
            profile: ProviderProfile {
                id: "c2".into(),
                app: AppKind::Claude,
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
            },
            common: CommonConfigPatch {
                app: AppKind::Claude,
                entries: vec![],
            },
        }
    }

    #[test]
    fn parses_valid_and_rejects_invalid_json_with_line() {
        assert!(parse(CLAUDE_JSON).is_ok());
        let err = parse("{\n  \"model\": oops\n}").unwrap_err();
        assert!(err.line.is_some());
    }

    fn common(entries: Vec<PatchEntry>) -> CommonConfigPatch {
        CommonConfigPatch {
            app: AppKind::Claude,
            entries,
        }
    }

    fn toggle(key: &str, value: Option<PatchValue>) -> PatchEntry {
        PatchEntry {
            key: key.into(),
            value,
        }
    }

    #[test]
    fn common_only_render_sets_and_removes_toggle_lines() {
        let set = crate::adapter::common_render(
            CLAUDE_JSON,
            &common(vec![
                toggle("alwaysThinkingEnabled", Some(PatchValue::Bool(true))),
                toggle("attribution.coAuthoredBy", Some(PatchValue::Bool(false))),
            ]),
        )
        .unwrap();
        assert!(set.contains("\"alwaysThinkingEnabled\": true"));
        assert!(set.contains("\"coAuthoredBy\": false"));
        // Host content outside the patch survives untouched.
        assert!(set.contains("statusLine"));

        let removed = crate::adapter::common_render(
            &set,
            &common(vec![
                toggle("alwaysThinkingEnabled", None),
                toggle("attribution.coAuthoredBy", None),
            ]),
        )
        .unwrap();
        assert!(!removed.contains("alwaysThinkingEnabled"));
        assert!(!removed.contains("coAuthoredBy"));
        assert!(removed.contains("statusLine"));
    }

    #[test]
    fn common_only_preview_reports_only_patch_changes() {
        let preview = crate::adapter::common_preview(
            CLAUDE_JSON,
            &common(vec![toggle(
                "spinnerTipsEnabled",
                Some(PatchValue::Bool(false)),
            )]),
            "F:/backups",
        )
        .unwrap();
        assert_eq!(preview.changes.len(), 1);
        assert_eq!(preview.changes[0].key, "spinnerTipsEnabled");
        assert_eq!(preview.changes[0].after.as_deref(), Some("false"));
        assert_eq!(preview.app, AppKind::Claude);
    }

    #[test]
    fn toggle_is_active_reflects_the_line_value() {
        assert!(crate::adapter::toggle_is_active(
            AppKind::Claude,
            "{ \"spinnerTipsEnabled\": false }",
            "spinnerTipsEnabled",
            false
        ));
        assert!(!crate::adapter::toggle_is_active(
            AppKind::Claude,
            "{ }",
            "spinnerTipsEnabled",
            false
        ));
        assert!(!crate::adapter::toggle_is_active(
            AppKind::Claude,
            "{ broken",
            "spinnerTipsEnabled",
            false
        ));
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
    fn token_key_is_profile_owned_and_switch_writes_it() {
        let current = CLAUDE_JSON.replace(
            "\"ANTHROPIC_MODEL\": \"claude-sonnet-4\"",
            "\"ANTHROPIC_AUTH_TOKEN\": \"sk-live-old-secret\"",
        );
        let mut plan = plan_b();
        plan.common.entries.push(PatchEntry {
            key: "env.ANTHROPIC_AUTH_TOKEN".into(),
            value: Some(PatchValue::Str("sk-live-forbidden".into())),
        });
        let err = crate::adapter::preview(&current, &plan, "/b").unwrap_err();
        assert!(err.message.contains("供应商档案管理"));

        let preview = preview(&current, &plan_b(), "/b").unwrap();
        let change = preview
            .changes
            .iter()
            .find(|change| change.key == "env.ANTHROPIC_AUTH_TOKEN")
            .expect("profile token change");
        assert_eq!(change.before.as_deref(), Some(crate::redact::REDACTED));
        assert_eq!(change.after.as_deref(), Some(crate::redact::REDACTED));
        let rendered = render(&current, &plan_b()).unwrap();
        assert!(rendered.contains("test-api-key"));
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
