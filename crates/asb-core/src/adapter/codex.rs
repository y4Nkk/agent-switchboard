//! Codex `config.toml` adapter.
//!
//! Pure parse / preview / render over configuration text. Host-owned keys,
//! comments and layout are preserved by editing the document through
//! `toml_edit` instead of re-serializing from a typed mirror.

use crate::adapter::{AdapterError, OverlayEntry};
use crate::contracts::{
    ChangeKind, CodexModelSettings, CommonConfigPatch, KeyChange, ModelOptions, PatchValue,
    RouteMode, RouteState, SwitchPlan, SwitchPreview,
};
use crate::ownership::is_owned;
use crate::redact::redact;
use crate::AppKind;
use toml_edit::{DocumentMut, Item, TableLike, Value as TomlValue};

/// The single provider table Agent Switchboard manages.
pub const MANAGED_TABLE: &str = "model_providers.asb";
pub const MANAGED_NAME: &str = "asb";
/// The built-in provider Codex routes to when no custom service is active.
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

fn set_path(doc: &mut DocumentMut, path: &str, value: PatchValue) {
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current = doc.as_item_mut();
    for &seg in parents {
        let table = current.as_table_like_mut().expect("parent is a table");
        if !table.contains_key(seg) {
            table.insert(seg, Item::Table(toml_edit::Table::new()));
        }
        current = table.get_mut(seg).expect("just inserted");
    }
    let table = current.as_table_like_mut().expect("parent is a table");
    table.insert(last, Item::Value(to_toml_value(value)));
}

fn to_toml_value(value: PatchValue) -> TomlValue {
    match value {
        PatchValue::Str(s) => TomlValue::from(s),
        PatchValue::Bool(b) => TomlValue::from(b),
        PatchValue::Number(n) => {
            if n.fract() == 0.0 && n.abs() < 9.0e15 {
                TomlValue::from(n as i64)
            } else {
                TomlValue::from(n)
            }
        }
        PatchValue::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(to_toml_value(item));
            }
            TomlValue::Array(array)
        }
    }
}

/// Builds the overlay entries implied by `plan`. Every profile routes to a
/// custom endpoint; validation rejects a draft without a base URL.
fn overlay(plan: &SwitchPlan) -> Vec<(String, OverlayEntry)> {
    let p = &plan.profile;
    let mut entries: Vec<(String, OverlayEntry)> = {
        let mut table = vec![
            (
                "model_provider".into(),
                OverlayEntry::Set(PatchValue::Str(MANAGED_NAME.into())),
            ),
            (
                format!("{MANAGED_TABLE}.name"),
                OverlayEntry::Set(PatchValue::Str(p.name.clone())),
            ),
            (
                format!("{MANAGED_TABLE}.wire_api"),
                OverlayEntry::Set(PatchValue::Str("responses".into())),
            ),
            (
                format!("{MANAGED_TABLE}.base_url"),
                OverlayEntry::Set(PatchValue::Str(
                    p.base_url.clone().expect("validated custom base_url"),
                )),
            ),
            (
                "experimental_bearer_token".into(),
                OverlayEntry::Set(PatchValue::Str(p.api_key.clone())),
            ),
        ];
        table.append(&mut run_parameter_entries(p));
        table
    };
    // Top-level owned keys hold possible host values: absent overlay fields
    // leave them untouched.
    entries.push((
        "model".into(),
        match &p.model {
            Some(m) => OverlayEntry::Set(PatchValue::Str(m.clone())),
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
    entries
}

/// Overlay entries for a general-settings-only apply: exactly the patch's
/// own lines, with no profile routing.
pub(crate) fn common_overlay(common: &CommonConfigPatch) -> Vec<(String, OverlayEntry)> {
    common
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
        .collect()
}

/// Codex run parameters declared by the profile's model options. Absent
/// fields leave existing host values untouched. Effort, summary, and
/// verbosity are general settings and never come from a profile.
fn run_parameter_entries(p: &crate::contracts::ProviderProfile) -> Vec<(String, OverlayEntry)> {
    let Some(ModelOptions::Codex(settings)) = &p.model_options else {
        return vec![];
    };
    match settings.context_window {
        Some(tokens) => vec![(
            "model_context_window".to_string(),
            OverlayEntry::Set(PatchValue::Number(tokens as f64)),
        )],
        None => vec![],
    }
}

pub(crate) fn check_syntax(text: &str) -> Result<(), AdapterError> {
    parse(text).map(|_| ())
}

/// Textual value at an owned dotted path, for toggle-state reads.
pub(crate) fn parse_owned_scalar(text: &str, key: &str) -> Result<Option<String>, AdapterError> {
    let doc = parse(text)?;
    Ok(item_at(&doc, key).and_then(item_repr))
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

fn active_provider_name(doc: &DocumentMut) -> Option<String> {
    doc.as_table()
        .get("model_provider")
        .and_then(item_repr)
        .filter(|name| name != OFFICIAL_PROVIDER)
}

fn active_provider_table<'a>(doc: &'a DocumentMut) -> Option<&'a dyn TableLike> {
    let active = active_provider_name(doc)?;
    doc.as_table()
        .get("model_providers")
        .and_then(Item::as_table_like)?
        .get(&active)
        .and_then(Item::as_table_like)
}

fn table_value(table: &dyn TableLike, key: &str) -> Option<String> {
    table.get(key).and_then(item_repr)
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
    let provider = active_provider_table(&doc);
    RouteState {
        app: AppKind::Codex,
        route_mode: match active_provider_name(&doc) {
            Some(_) => RouteMode::Custom,
            None => RouteMode::Official,
        },
        provider_name: provider.and_then(|table| table_value(table, "name")),
        model: get("model"),
        base_url: provider.and_then(|table| table_value(table, "base_url")),
        wire_api: provider
            .and_then(|table| table_value(table, "wire_api"))
            .or_else(|| provider.map(|_| "responses".to_string())),
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
            OverlayEntry::RemoveTableIfPresent => {
                if item_at(&doc, &key).is_some_and(Item::is_table_like) {
                    changes.push(KeyChange {
                        key: key.clone(),
                        kind: ChangeKind::Remove,
                        before: Some("（托管供应商表）".to_string()),
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

pub(crate) fn render_entries(
    current: &str,
    entries: Vec<(String, OverlayEntry)>,
) -> Result<String, AdapterError> {
    let mut doc = parse(current)?;
    for (key, entry) in entries {
        match entry {
            OverlayEntry::Set(value) => set_path(&mut doc, &key, value),
            OverlayEntry::Leave => {}
            OverlayEntry::RemoveIfPresent | OverlayEntry::RemoveTableIfPresent => {
                remove_path(&mut doc, &key);
            }
        }
    }
    Ok(doc.to_string())
}

fn remove_path(doc: &mut DocumentMut, path: &str) {
    let segs: Vec<&str> = path.split('.').collect();
    let (&last, parents) = segs.split_last().expect("non-empty path");
    let mut current = doc.as_item_mut();
    for &seg in parents {
        let Some(table) = current.as_table_like_mut() else {
            return;
        };
        current = match table.get_mut(seg) {
            Some(item) => item,
            None => return,
        };
    }
    if let Some(table) = current.as_table_like_mut() {
        table.remove(last);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{
        CodexModelSettings, CommonConfigPatch, ModelOptions, PatchEntry, ProviderProfile,
    };
    use crate::test_support::CODEX_TOML;

    fn plan_b() -> SwitchPlan {
        SwitchPlan {
            app: AppKind::Codex,
            profile: ProviderProfile {
                id: "p2".into(),
                app: AppKind::Codex,
                name: "Relay B".into(),
                model: Some("gpt-5.2".into()),
                base_url: Some("https://relay-b.internal/v1".into()),
                api_key: "CODEX_RELAY_B_KEY".into(),
                model_options: Some(ModelOptions::Codex(CodexModelSettings {
                    context_window: Some(272_000),
                })),
                notes: None,
                website_url: None,
            },
            common: CommonConfigPatch {
                app: AppKind::Codex,
                entries: vec![PatchEntry {
                    key: "disable_response_storage".into(),
                    value: Some(PatchValue::Bool(true)),
                }],
            },
        }
    }

    #[test]
    fn parses_valid_and_rejects_invalid_toml_with_line() {
        assert!(parse(CODEX_TOML).is_ok());
        let err = parse("model = \"x\"\nthreads = [unclosed\n").unwrap_err();
        assert!(err.line.is_some());
    }

    fn common(entries: Vec<PatchEntry>) -> CommonConfigPatch {
        CommonConfigPatch {
            app: AppKind::Codex,
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
            CODEX_TOML,
            &common(vec![toggle(
                "hide_agent_reasoning",
                Some(PatchValue::Bool(true)),
            )]),
        )
        .unwrap();
        assert!(set.contains("hide_agent_reasoning = true"));
        // Host content outside the patch survives untouched.
        assert!(set.contains("threads = 8"));
        assert!(set.contains("[model_providers.openai]"));

        let removed = crate::adapter::common_render(
            &set,
            &common(vec![toggle("hide_agent_reasoning", None)]),
        )
        .unwrap();
        assert!(!removed.contains("hide_agent_reasoning"));
        assert!(removed.contains("threads = 8"));
    }

    #[test]
    fn common_only_preview_reports_only_patch_changes() {
        let preview = crate::adapter::common_preview(
            CODEX_TOML,
            &common(vec![
                toggle("disable_response_storage", Some(PatchValue::Bool(true))),
                toggle("hide_agent_reasoning", None),
            ]),
            "F:/backups",
        )
        .unwrap();
        assert_eq!(preview.changes.len(), 1);
        assert_eq!(preview.changes[0].key, "disable_response_storage");
        assert_eq!(preview.app, AppKind::Codex);
    }

    #[test]
    fn toggle_is_active_reflects_the_line_value() {
        assert!(crate::adapter::toggle_is_active(
            AppKind::Codex,
            "hide_agent_reasoning = true\nthreads = 8\n",
            "hide_agent_reasoning",
            true
        ));
        assert!(!crate::adapter::toggle_is_active(
            AppKind::Codex,
            "threads = 8\n",
            "hide_agent_reasoning",
            true
        ));
        assert!(!crate::adapter::toggle_is_active(
            AppKind::Codex,
            "not toml [",
            "hide_agent_reasoning",
            true
        ));
    }

    #[test]
    fn preview_names_changed_keys_and_backup_target() {
        let preview = preview(CODEX_TOML, &plan_b(), "F:/backups").unwrap();

        let changed: Vec<&str> = preview.changes.iter().map(|c| c.key.as_str()).collect();
        assert!(changed.contains(&"model"));
        // model_provider is already "asb"; an unchanged key produces no diff
        // entry.
        assert!(!changed.contains(&"model_provider"));
        assert!(changed.contains(&"model_providers.asb.base_url"));
        assert!(changed.contains(&"model_context_window"));
        assert_eq!(preview.backup_dir, "F:/backups");
        assert_eq!(preview.target, "~/.codex/config.toml");
        // Host keys stay untouched — asserted on the rendered text in
        // render_preserves_host_content_and_comments.
    }

    #[test]
    fn undeclared_run_parameters_leave_host_values_untouched() {
        let mut plan = plan_b();
        plan.profile.model_options = None;
        let current = "model_verbosity = \"low\"\n";
        let preview = preview(current, &plan, "/b").unwrap();
        assert!(!preview.changes.iter().any(|c| c.key == "model_verbosity"));
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
        assert!(rendered.contains("[model_providers.openai]"));
        assert!(rendered.contains("trusted = true"));
        assert!(rendered.contains("base_url = \"https://relay-b.internal/v1\""));
        assert!(rendered.contains("model = \"gpt-5.2\""));
        assert!(rendered.contains("model_provider = \"asb\""));
        assert!(rendered.contains("wire_api = \"responses\""));
        // Output stays valid TOML.
        rendered
            .parse::<DocumentMut>()
            .expect("rendered document must be valid TOML");
    }

    #[test]
    fn render_writes_profile_bearer_token() {
        let rendered = render(CODEX_TOML, &plan_b()).unwrap();
        assert!(rendered.contains("experimental_bearer_token = \"CODEX_RELAY_B_KEY\""));
        // Host provider table survives.
        assert!(rendered.contains("[model_providers.openai]"));
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
    fn route_state_reads_active_provider_and_model() {
        let state = route_state(CODEX_TOML);
        assert_eq!(state.route_mode, RouteMode::Custom);
        assert_eq!(state.provider_name.as_deref(), Some("中继 A"));
        assert_eq!(state.model.as_deref(), Some("gpt-5.1"));
        assert_eq!(
            state.base_url.as_deref(),
            Some("https://relay-a.internal/v1")
        );
        assert!(state.scope_warnings.is_empty());
    }

    #[test]
    fn route_state_reports_official_when_routing_to_openai_or_nothing() {
        let official_table =
            CODEX_TOML.replace("model_provider = \"asb\"", "model_provider = \"openai\"");
        assert_eq!(route_state(&official_table).route_mode, RouteMode::Official);

        let no_provider = CODEX_TOML.replace("model_provider = \"asb\"\n", "");
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
