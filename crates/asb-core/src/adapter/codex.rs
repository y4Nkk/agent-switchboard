//! Codex `config.toml` adapter.
//!
//! Pure parse / preview / render over configuration text. Host-owned keys,
//! comments and layout are preserved by editing the document through
//! `toml_edit` instead of re-serializing from a typed mirror.

use crate::adapter::{AdapterError, OverlayEntry};
use crate::contracts::{
    ChangeKind, CodexModelSettings, CommonSettingValue, CommonSettings, ConfigValue, KeyChange,
    ModelOptions, RouteMode, RouteState, SwitchPlan, SwitchPreview,
};
use crate::ownership::{
    is_owned, provider_absent_action, setting_specs, ProviderAbsentAction, SettingOwner,
};
use crate::redact::redact;
use crate::AppKind;
use toml_edit::{DocumentMut, Item, TableLike, Value as TomlValue};

/// Codex's built-in provider id.
pub const OFFICIAL_PROVIDER: &str = "openai";

fn parse(text: &str) -> Result<DocumentMut, AdapterError> {
    text.parse::<DocumentMut>()
        .map_err(|e: toml_edit::TomlError| {
            let line = e
                .span()
                .map(|span| text[..span.start].matches('\n').count() + 1);
            AdapterError {
                message: "TOML 格式无效".to_string(),
                line,
            }
        })
}

/// Textual repr of a scalar TOML value for diff comparison/display.
fn scalar_repr(value: &TomlValue) -> Option<String> {
    match value {
        TomlValue::String(s) => Some(s.value().clone()),
        // Decoded values only: Display carries the decor (surrounding
        // whitespace), which would break exact value comparisons.
        TomlValue::Integer(i) => Some(i.value().to_string()),
        TomlValue::Float(f) => Some(f.value().to_string()),
        TomlValue::Boolean(b) => Some(b.value().to_string()),
        _ => None,
    }
}

fn item_repr(item: &Item) -> Option<String> {
    item.as_value().and_then(scalar_repr)
}

fn item_at<'a>(doc: &'a DocumentMut, path: &str) -> Option<&'a Item> {
    let mut item = doc.as_item();
    for seg in path.split('.') {
        item = item.as_table_like()?.get(seg)?;
    }
    Some(item)
}

fn parent_conflict(path: &str, segment: &str) -> AdapterError {
    AdapterError {
        message: format!("受管路径 {path} 的父节点 {segment} 不是 TOML 表，无法安全修改"),
        line: None,
    }
}

fn leaf_conflict(path: &str) -> AdapterError {
    AdapterError {
        message: format!("受管键 {path} 当前是 TOML 表，无法覆盖其宿主内容"),
        line: None,
    }
}

fn validate_path(doc: &DocumentMut, path: &str) -> Result<(), AdapterError> {
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current = doc.as_item();
    let mut parent_path = String::new();
    for &seg in parents {
        let table = current
            .as_table_like()
            .ok_or_else(|| parent_conflict(path, &parent_path))?;
        if !parent_path.is_empty() {
            parent_path.push('.');
        }
        parent_path.push_str(seg);
        match table.get(seg) {
            Some(item) if item.as_table_like().is_some() => current = item,
            Some(_) => return Err(parent_conflict(path, &parent_path)),
            None => return Ok(()),
        }
    }
    let table = current
        .as_table_like()
        .ok_or_else(|| parent_conflict(path, &parent_path))?;
    if table
        .get(last)
        .is_some_and(|item| item.as_table_like().is_some())
    {
        return Err(leaf_conflict(path));
    }
    Ok(())
}

fn set_path(doc: &mut DocumentMut, path: &str, value: ConfigValue) -> Result<(), AdapterError> {
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current = doc.as_item_mut();
    let mut parent_path = String::new();
    for &seg in parents {
        let table = current
            .as_table_like_mut()
            .ok_or_else(|| parent_conflict(path, &parent_path))?;
        if !parent_path.is_empty() {
            parent_path.push('.');
        }
        parent_path.push_str(seg);
        if !table.contains_key(seg) {
            table.insert(seg, Item::Table(toml_edit::Table::new()));
        }
        let next = table
            .get_mut(seg)
            .expect("inserted or pre-existing path segment");
        if next.as_table_like().is_none() {
            return Err(parent_conflict(path, &parent_path));
        }
        current = next;
    }
    let table = current
        .as_table_like_mut()
        .ok_or_else(|| parent_conflict(path, &parent_path))?;
    if table
        .get(last)
        .is_some_and(|item| item.as_table_like().is_some())
    {
        return Err(leaf_conflict(path));
    }
    table.insert(last, Item::Value(to_toml_value(value)));
    Ok(())
}

fn to_toml_value(value: ConfigValue) -> TomlValue {
    match value {
        ConfigValue::Str(s) => TomlValue::from(s),
        ConfigValue::Bool(b) => TomlValue::from(b),
        ConfigValue::Number(n) => {
            if n.fract() == 0.0 && n.abs() < 9.0e15 {
                TomlValue::from(n as i64)
            } else {
                TomlValue::from(n)
            }
        }
        ConfigValue::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(to_toml_value(item));
            }
            TomlValue::Array(array)
        }
    }
}

/// Builds the overlay entries implied by `plan`. A custom profile routes
/// through Codex's built-in OpenAI provider with an explicit base-URL
/// override. An official profile removes every application-owned routing key
/// and leaves Codex's native login cache untouched.
fn absent_provider_entry(key: &str) -> OverlayEntry {
    match provider_absent_action(AppKind::Codex, key) {
        Some(ProviderAbsentAction::Remove) => OverlayEntry::RemoveIfPresent,
        None => unreachable!("Codex provider mapping must be declared in the ownership directory"),
    }
}

fn provider_value(profile: &crate::contracts::ProviderProfile, key: &str) -> Option<ConfigValue> {
    if profile.route_mode == crate::contracts::RouteMode::Official {
        return None;
    }
    match key {
        "model" => profile.model.clone().map(ConfigValue::Str),
        "model_provider" => Some(ConfigValue::Str(OFFICIAL_PROVIDER.into())),
        "openai_base_url" => profile.base_url.clone().map(ConfigValue::Str),
        "experimental_bearer_token" => Some(ConfigValue::Str(profile.api_key.clone())),
        "model_context_window" => match &profile.model_options {
            Some(ModelOptions::Codex(settings)) => settings
                .context_window
                .map(|tokens| ConfigValue::Number(tokens as f64)),
            None => None,
            Some(ModelOptions::Claude(_)) => {
                unreachable!("profile validation rejects mismatched options")
            }
        },
        _ => unreachable!("Codex provider mapping must be declared in the ownership directory"),
    }
}

/// One common setting's overlay entry: an explicit non-default value is
/// written, while the directory default is expressed by omitting the line.
/// Common-setting intent is explicit: automatic keys leave the host value
/// alone only after removing a previously managed line; explicit values are
/// always written, even when they resemble a documented client default.
fn common_entry(value: &CommonSettingValue) -> OverlayEntry {
    match value {
        CommonSettingValue::Automatic => OverlayEntry::RemoveIfPresent,
        CommonSettingValue::Explicit { value } => OverlayEntry::Set(value.clone()),
    }
}

fn common_overlay(common: &CommonSettings) -> Vec<(String, OverlayEntry)> {
    setting_specs(AppKind::Codex)
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

/// Derives all changes by iterating the ownership directory. The only literal
/// names below map declared slots to plan data; no second managed-key list
/// exists in the adapter.
fn overlay(plan: &SwitchPlan) -> Vec<(String, OverlayEntry)> {
    setting_specs(AppKind::Codex)
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

/// Collects every owned scalar path and its textual value.
fn collect_owned_scalars(
    table: &dyn TableLike,
    prefix: &str,
    out: &mut std::collections::BTreeMap<String, String>,
) {
    for (key, item) in table.iter() {
        let path = if prefix.is_empty() {
            key.to_string()
        } else {
            format!("{prefix}.{key}")
        };
        if let Some(value) = item_repr(item) {
            if is_owned(AppKind::Codex, &path) {
                out.insert(path.clone(), value);
            }
        }
        if let Some(sub) = item.as_table_like() {
            collect_owned_scalars(sub, &path, out);
        }
    }
}

/// Owned-key diff between the live text and a previous copy.
pub(crate) fn owned_diff(current: &str, previous: &str) -> Result<Vec<KeyChange>, AdapterError> {
    let mut current_values = std::collections::BTreeMap::new();
    collect_owned_scalars(parse(current)?.as_table(), "", &mut current_values);
    let mut previous_values = std::collections::BTreeMap::new();
    collect_owned_scalars(parse(previous)?.as_table(), "", &mut previous_values);
    Ok(crate::adapter::diff_owned_maps(
        &current_values,
        &previous_values,
    ))
}

/// Scope-of-effect warnings for facts inside this file that a `--profile`
/// launch or other override can shadow.
fn scope_warnings(doc: &DocumentMut) -> Vec<String> {
    let profiles = doc
        .as_table()
        .get("profiles")
        .and_then(Item::as_table_like)
        .map(|table| table.len())
        .unwrap_or(0);
    if profiles > 0 {
        vec![format!(
            "config.toml 定义了 {profiles} 个配置档；使用 --profile 启动 Codex 时会覆盖这里的用户级设置"
        )]
    } else {
        vec![]
    }
}

/// Reads the active routing facts from Codex configuration text.
pub fn route_state(text: &str) -> RouteState {
    let doc = parse(text).expect("caller validates syntax first");
    let get = |path: &str| item_at(&doc, path).and_then(item_repr);
    let provider_id = get("model_provider").unwrap_or_else(|| OFFICIAL_PROVIDER.to_string());
    let openai_base_url = (provider_id == OFFICIAL_PROVIDER)
        .then(|| get("openai_base_url"))
        .flatten();
    let wire_api = openai_base_url.as_ref().map(|_| "responses".to_string());
    RouteState {
        app: AppKind::Codex,
        route_mode: if provider_id != OFFICIAL_PROVIDER || openai_base_url.is_some() {
            RouteMode::Custom
        } else {
            RouteMode::Official
        },
        provider_name: None,
        model: get("model"),
        base_url: openai_base_url,
        wire_api,
        codex_model_options: Some(CodexModelSettings {
            context_window: item_at(&doc, "model_context_window")
                .and_then(|item| item.as_value())
                .and_then(|value| value.as_integer())
                .and_then(|value| u64::try_from(value).ok()),
        }),
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        available_models: None,
        scope_warnings: scope_warnings(&doc),
    }
}

pub(crate) fn preview(
    current: &str,
    plan: &SwitchPlan,
    backup_dir: &str,
) -> Result<SwitchPreview, AdapterError> {
    preview_entries(current, overlay(plan), backup_dir)
}

pub(crate) fn preview_entries(
    current: &str,
    entries: Vec<(String, OverlayEntry)>,
    backup_dir: &str,
) -> Result<SwitchPreview, AdapterError> {
    let doc = parse(current)?;
    for (key, entry) in &entries {
        if !matches!(entry, OverlayEntry::Leave) {
            validate_path(&doc, key)?;
        }
    }
    let mut changes = Vec::new();
    for (key, entry) in entries {
        let existing = item_at(&doc, &key).and_then(item_repr);
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
            OverlayEntry::Leave => {}
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
        }
    }

    Ok(SwitchPreview {
        app: AppKind::Codex,
        target: AppKind::Codex.config_label().to_string(),
        changes,
        warnings: vec![],
        backup_dir: backup_dir.to_string(),
    })
}

pub(crate) fn render(current: &str, plan: &SwitchPlan) -> Result<String, AdapterError> {
    render_entries(current, overlay(plan))
}

pub(crate) fn render_common_settings(common: &CommonSettings) -> Result<String, AdapterError> {
    let rendered = render_entries("", common_overlay(common))?;
    Ok(if rendered.trim().is_empty() {
        "# 所有通用设置均为自动\n".to_string()
    } else {
        rendered
    })
}

pub(crate) fn render_entries(
    current: &str,
    entries: Vec<(String, OverlayEntry)>,
) -> Result<String, AdapterError> {
    let mut doc = parse(current)?;
    for (key, entry) in entries {
        match entry {
            OverlayEntry::Set(value) => set_path(&mut doc, &key, value)?,
            OverlayEntry::Leave => {}
            OverlayEntry::RemoveIfPresent => {
                remove_path(&mut doc, &key)?;
            }
        }
    }
    Ok(doc.to_string())
}

fn remove_path(doc: &mut DocumentMut, path: &str) -> Result<(), AdapterError> {
    validate_path(doc, path)?;
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current = doc.as_item_mut();
    for &seg in parents {
        let Some(table) = current.as_table_like_mut() else {
            return Err(parent_conflict(path, seg));
        };
        current = match table.get_mut(seg) {
            Some(item) => item,
            None => return Ok(()),
        };
    }
    if let Some(table) = current.as_table_like_mut() {
        table.remove(last);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{CodexModelSettings, ModelOptions, ProviderProfile};
    use crate::ownership::default_common_settings;
    use crate::test_support::CODEX_TOML;

    fn plan_b() -> SwitchPlan {
        SwitchPlan {
            profile: ProviderProfile {
                id: "p2".into(),
                app: AppKind::Codex,
                route_mode: crate::contracts::RouteMode::Custom,
                name: "Relay B".into(),
                model: Some("gpt-5.2".into()),
                base_url: Some("https://relay-b.internal/v1".into()),
                api_key: "CODEX_RELAY_B_KEY".into(),
                model_options: Some(ModelOptions::Codex(CodexModelSettings {
                    context_window: Some(272_000),
                })),
                notes: None,
                website_url: None,
                usage_query: None,
            },
            common: common_with("model_reasoning_effort", ConfigValue::Str("xhigh".into())),
        }
    }

    fn common_with(key: &str, value: ConfigValue) -> crate::contracts::CommonSettings {
        let mut common = default_common_settings(AppKind::Codex);
        common
            .settings
            .insert(key.to_string(), CommonSettingValue::Explicit { value });
        common
    }

    #[test]
    fn parses_valid_and_rejects_invalid_toml_with_line() {
        assert!(parse(CODEX_TOML).is_ok());
        let err = parse("model = \"x\"\nthreads = [unclosed\n").unwrap_err();
        assert!(err.line.is_some());
    }

    #[test]
    fn preview_names_changed_keys_and_backup_target() {
        let preview = preview(CODEX_TOML, &plan_b(), "F:/backups").unwrap();

        let changed: Vec<&str> = preview.changes.iter().map(|c| c.key.as_str()).collect();
        assert!(changed.contains(&"model"));
        // model_provider is already the built-in OpenAI id; an unchanged key
        // produces no diff entry.
        assert!(!changed.contains(&"model_provider"));
        assert!(changed.contains(&"openai_base_url"));
        assert!(changed.contains(&"model_context_window"));
        assert_eq!(preview.backup_dir, "F:/backups");
        assert_eq!(preview.target, "~/.codex/config.toml");
        // Host keys stay untouched — asserted on the rendered text in
        // render_preserves_host_content_and_comments.
    }

    #[test]
    fn official_route_removes_managed_custom_keys_and_keeps_host_keys() {
        let mut plan = plan_b();
        plan.profile.route_mode = RouteMode::Official;
        plan.profile.model = None;
        plan.profile.base_url = None;
        plan.profile.api_key.clear();
        plan.profile.model_options = None;

        let rendered = render(CODEX_TOML, &plan).expect("official render");
        assert!(!rendered.contains("experimental_bearer_token"));
        assert!(!rendered.contains("openai_base_url"));
        assert!(!rendered.contains("model_provider"));
        assert!(rendered.contains("threads = 8"));
    }

    #[test]
    fn explicit_general_settings_write_their_value() {
        let current = "";
        let preview = preview(current, &plan_b(), "/b").unwrap();
        let change = preview
            .changes
            .iter()
            .find(|c| c.key == "model_reasoning_effort")
            .expect("explicit common value must be written");
        assert_eq!(change.kind, ChangeKind::Set);
    }

    #[test]
    fn automatic_general_settings_remove_hand_set_lines() {
        let mut plan = plan_b();
        plan.common = default_common_settings(AppKind::Codex);
        let current = "model_verbosity = \"low\"\n";
        let preview = preview(current, &plan, "/b").unwrap();
        let change = preview
            .changes
            .iter()
            .find(|c| c.key == "model_verbosity")
            .expect("automatic behavior removes the app-owned line");
        assert_eq!(change.kind, ChangeKind::Remove);
        let rendered = render(current, &plan).unwrap();
        assert!(!rendered.contains("model_verbosity"));

        // With the line already absent automatic behavior produces no diff entry.
        let clean = super::preview("", &plan, "/b").unwrap();
        assert!(!clean.changes.iter().any(|c| c.key == "model_verbosity"));
    }

    #[test]
    fn preview_replaces_bearer_token_and_redacts_it() {
        let current = format!("{CODEX_TOML}\nexperimental_bearer_token = \"sk-live-old-secret\"\n");

        let preview = preview(&current, &plan_b(), "/b").unwrap();
        let change = preview
            .changes
            .iter()
            .find(|c| c.key == "experimental_bearer_token")
            .expect("bearer token must be replaced");
        assert_eq!(change.kind, ChangeKind::Set);
        assert_eq!(change.before.as_deref(), Some(crate::redact::REDACTED));
        assert_eq!(change.after.as_deref(), Some(crate::redact::REDACTED));
        assert!(!format!("{preview:?}").contains("sk-live-old-secret"));
    }

    #[test]
    fn preview_is_pure_input_text_untouched() {
        let input = CODEX_TOML.to_string();
        let _ = preview(&input, &plan_b(), "/b").unwrap();
        assert_eq!(input, CODEX_TOML);
    }

    #[test]
    fn render_preserves_host_content_and_comments() {
        let rendered = render(CODEX_TOML, &plan_b()).unwrap();

        assert!(rendered.contains("# host-owned Codex configuration (test sample)"));
        assert!(rendered.contains("threads = 8"));
        assert!(rendered.contains("history_persistence = \"save-all\""));
        assert!(rendered.contains("trusted = true"));
        assert!(rendered.contains("model = \"gpt-5.2\""));
        assert!(rendered.contains("model_provider = \"openai\""));
        assert!(rendered.contains("openai_base_url = \"https://relay-b.internal/v1\""));
        assert!(!rendered.contains("[model_providers"));
        assert!(!rendered.contains("wire_api"));
        // Output stays valid TOML.
        rendered
            .parse::<DocumentMut>()
            .expect("rendered document must be valid TOML");
    }

    #[test]
    fn render_writes_profile_bearer_token() {
        let rendered = render(CODEX_TOML, &plan_b()).unwrap();
        assert!(rendered.contains("experimental_bearer_token = \"CODEX_RELAY_B_KEY\""));
    }

    #[test]
    fn dotted_parent_conflict_is_rejected_without_overwriting_host_content() {
        let current = "tui = \"host-owned scalar\"\nthreads = 8\n";
        let mut plan = plan_b();
        plan.common = common_with("tui.animations", ConfigValue::Bool(false));

        let preview_error = preview(current, &plan, "/b").unwrap_err();
        assert!(preview_error.message.contains("父节点 tui"));
        let render_error = render(current, &plan).unwrap_err();
        assert!(render_error.message.contains("父节点 tui"));
        assert_eq!(current, "tui = \"host-owned scalar\"\nthreads = 8\n");
    }

    #[test]
    fn render_preserves_host_owned_provider_tables() {
        let current = format!(
            "{CODEX_TOML}\n[model_providers.gateway]\nbase_url = \"https://gateway.internal/v1\"\n"
        );
        let rendered = render(&current, &plan_b()).unwrap();
        assert!(rendered.contains("[model_providers.gateway]"));
        assert!(rendered.contains("base_url = \"https://gateway.internal/v1\""));
    }

    #[test]
    fn render_is_idempotent() {
        let once = render(CODEX_TOML, &plan_b()).unwrap();
        let twice = render(&once, &plan_b()).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn parse_error_messages_are_scrubbed() {
        let err = parse("api_key = \"sk-live-0123456789abcdefghij\"\nx = [oops\n").unwrap_err();
        assert!(!err.message.contains("sk-live"));
    }

    #[test]
    fn route_state_reads_builtin_openai_override_and_model() {
        let state = route_state(CODEX_TOML);
        assert_eq!(state.route_mode, RouteMode::Custom);
        assert!(state.provider_name.is_none());
        assert_eq!(state.model.as_deref(), Some("gpt-5.1"));
        assert_eq!(
            state.base_url.as_deref(),
            Some("https://relay-a.internal/v1")
        );
        assert!(state.scope_warnings.is_empty());
    }

    #[test]
    fn route_state_defaults_to_openai_when_model_provider_is_omitted() {
        let implicit = CODEX_TOML.replace("model_provider = \"openai\"\n", "");
        let state = route_state(&implicit);
        assert_eq!(state.route_mode, RouteMode::Custom);
        assert_eq!(
            state.base_url.as_deref(),
            Some("https://relay-a.internal/v1")
        );
        assert_eq!(state.wire_api.as_deref(), Some("responses"));
    }

    #[test]
    fn route_state_does_not_interpret_custom_provider_tables() {
        let external = r#"
model_provider = "gateway"

[model_providers.gateway]
name = "Gateway"
base_url = "https://gateway.internal/v1"
wire_api = "responses"
"#;
        let state = route_state(external);
        assert_eq!(state.route_mode, RouteMode::Custom);
        assert!(state.provider_name.is_none());
        assert!(state.base_url.is_none());
        assert!(state.wire_api.is_none());
    }

    #[test]
    fn route_state_reports_official_without_an_openai_base_url() {
        let official =
            CODEX_TOML.replace("openai_base_url = \"https://relay-a.internal/v1\"\n", "");
        assert_eq!(route_state(&official).route_mode, RouteMode::Official);

        let no_provider = official.replace("model_provider = \"openai\"\n", "");
        assert_eq!(route_state(&no_provider).route_mode, RouteMode::Official);
    }

    #[test]
    fn route_state_warns_when_profiles_could_override_user_config() {
        let with_profiles = format!("{CODEX_TOML}\n[profiles.dev]\nmodel = \"gpt-4o\"\n");
        let state = route_state(&with_profiles);
        assert_eq!(state.scope_warnings.len(), 1);
        assert!(state.scope_warnings[0].contains("--profile"));
    }
}
