use toml_edit::{DocumentMut, Item, TableLike};

use crate::adapter::AdapterError;
use crate::contracts::{CodexModelSettings, KeyChange, RouteMode, RouteState, SwitchPlan};
use crate::ownership::{is_owned, CODEX_PROVIDER_ID};
use crate::AppKind;

use crate::adapter::codex::document::{item_at, item_repr, parse};

/// Codex's built-in provider id.
pub const OFFICIAL_PROVIDER: &str = CODEX_PROVIDER_ID;

pub(crate) fn matches_provider_settings(
    text: &str,
    plan: &SwitchPlan,
) -> Result<bool, AdapterError> {
    let doc = parse(text)?;
    let provider = item_at(&doc, "model_provider")
        .and_then(item_repr)
        .unwrap_or_else(|| OFFICIAL_PROVIDER.to_string());
    Ok(provider == CODEX_PROVIDER_ID
        && (plan.profile.route_mode == RouteMode::Official
            || plan.is_gateway()
            || (plan.profile.upstream_protocol
                == Some(crate::contracts::UpstreamProtocol::Responses)
                && plan.client_base_url().is_some())))
}

/// Collects every scalar path accepted by `keep`, rendered by `repr`.
fn collect_scalars(
    table: &dyn TableLike,
    prefix: &str,
    out: &mut std::collections::BTreeMap<String, String>,
    keep: &dyn Fn(&str) -> bool,
    repr: &dyn Fn(&Item) -> Option<String>,
) {
    for (key, item) in table.iter() {
        let path = crate::config_path::append_key(prefix, key);
        collect_item(item, &path, out, keep, repr);
    }
}

fn collect_item(
    item: &Item,
    path: &str,
    out: &mut std::collections::BTreeMap<String, String>,
    keep: &dyn Fn(&str) -> bool,
    repr: &dyn Fn(&Item) -> Option<String>,
) {
    if let Some(value) = repr(item) {
        if keep(path) { out.insert(path.to_string(), value); }
    }
    if let Some(table) = item.as_table_like() {
        collect_scalars(table, path, out, keep, repr);
    } else if let Some(tables) = item.as_array_of_tables() {
        for (index, table) in tables.iter().enumerate() {
            collect_scalars(table, &format!("{path}[{index}]"), out, keep, repr);
        }
    } else if let Some(array) = item.as_value().and_then(toml_edit::Value::as_array) {
        for (index, value) in array.iter().enumerate() {
            collect_item(&Item::Value(value.clone()), &format!("{path}[{index}]"), out, keep, repr);
        }
    }
}

fn collect_with(
    table: &dyn TableLike,
    keep: &dyn Fn(&str) -> bool,
    repr: &dyn Fn(&Item) -> Option<String>,
) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    collect_scalars(table, "", &mut out, keep, repr);
    out
}

/// Textual repr for diff purposes. Inline tables are containers (the walk
/// recurses into them, so their children get individual leaf paths); arrays
/// and arrays-of-tables get a size summary plus separately collected children.
fn leaf_repr(item: &Item) -> Option<String> {
    if item.as_table_like().is_some_and(|table| table.is_empty()) {
        return Some("{}".into());
    }
    if let Some(value) = item_repr(item) {
        return Some(value);
    }
    if let Some(tables) = item.as_array_of_tables() {
        return Some(format!("[{} 个表]", tables.len()));
    }
    item.as_value().and_then(|value| match value {
        toml_edit::Value::Array(array) => Some(format!("[{} 项]", array.len())),
        toml_edit::Value::Datetime(value) => Some(value.value().to_string()),
        _ => None,
    })
}

/// Runtime agent keys have their own typed owner. Include them and retired
/// max_threads so every managed removal remains visible in the shared preview.
fn diff_keep(path: &str) -> bool {
    is_owned(AppKind::Codex, path)
        || crate::contracts::CodexSubagentKey::ALL.iter().any(|key| key.path() == path)
        || path == "agents.max_threads"
}

/// Owned-key diff between the live text and a previous copy.
pub(crate) fn owned_diff(current: &str, previous: &str) -> Result<Vec<KeyChange>, AdapterError> {
    Ok(crate::adapter::diff_owned_maps(
        &collect_with(parse(current)?.as_table(), &diff_keep, &|item| item_repr(item)),
        &collect_with(parse(previous)?.as_table(), &diff_keep, &|item| item_repr(item)),
    ))
}

/// Every-leaf diff, including host-owned keys: the deep reset preview must
/// list unmanaged removals, which [`owned_diff`] deliberately hides.
pub(crate) fn full_diff(current: &str, previous: &str) -> Result<Vec<KeyChange>, AdapterError> {
    Ok(crate::adapter::diff_owned_maps(
        &collect_with(parse(current)?.as_table(), &|_| true, &leaf_repr),
        &collect_with(parse(previous)?.as_table(), &|_| true, &leaf_repr),
    ))
}

/// Scope-of-effect warnings for facts inside this file that a `--profile`
/// launch or other override can shadow.
fn scope_warnings(doc: &DocumentMut) -> Vec<crate::contracts::LocalizedMessage> {
    let profiles = doc
        .as_table()
        .get("profiles")
        .and_then(Item::as_table_like)
        .map(|table| table.len())
        .unwrap_or(0);
    if profiles > 0 {
        vec![crate::contracts::LocalizedMessage::new(
            "warnings.codex.profileShadowing",
            serde_json::json!({ "profiles": profiles }),
            format!(
                "config.toml 定义了 {profiles} 个配置档；使用 --profile 启动 Codex 时会覆盖这里的用户级设置"
            ),
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
    let builtin_openai = provider_id == OFFICIAL_PROVIDER;
    let provider_table = (!builtin_openai)
        .then(|| doc.get("model_providers").and_then(Item::as_table_like))
        .flatten()
        .and_then(|providers| providers.get(&provider_id))
        .and_then(Item::as_table_like);
    let declared_name = provider_table
        .and_then(|table| table.get("name"))
        .and_then(item_repr)
        .filter(|value| !value.trim().is_empty());
    let base_url = if builtin_openai {
        get("openai_base_url")
    } else {
        provider_table
            .and_then(|table| table.get("base_url"))
            .and_then(item_repr)
    }
    .filter(|value| !value.trim().is_empty());
    let wire_api = provider_table
        .and_then(|table| table.get("wire_api"))
        .and_then(item_repr)
        .filter(|value| !value.trim().is_empty());
    let custom = !builtin_openai || base_url.is_some();
    RouteState {
        app: AppKind::Codex,
        route_mode: if custom {
            RouteMode::Custom
        } else {
            RouteMode::Official
        },
        provider_name: declared_name.or(Some(provider_id)),
        model: get("model"),
        base_url,
        wire_api,
        codex_model_options: Some(CodexModelSettings {
            context_window: item_at(&doc, "model_context_window")
                .and_then(|item| item.as_value())
                .and_then(|value| value.as_integer())
                .and_then(|value| u64::try_from(value).ok()),
            subagent_route: item_at(&doc, "agents.default_subagent_model")
                .and_then(|item| item.as_value())
                .and_then(|value| value.as_str())
                .and_then(crate::contracts::CodexSubagentRoute::parse_wire_id),
        }),
        haiku_model: None,
        sonnet_model: None,
        opus_model: None,
        fable_model: None,
        available_models: None,
        scope_warnings: scope_warnings(&doc),
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retired_max_threads_appears_as_an_explicit_removal_in_the_diff() {
        let changes = owned_diff("", "[agents]\nmax_threads = 4\n").expect("diff");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].key, "agents.max_threads");
        assert_eq!(changes[0].after, None);
    }

    #[test]
    fn full_diff_reports_host_removals_that_owned_diff_hides() {
        let live = "[tools]\nflag = true\n";
        assert!(owned_diff("", live).expect("owned diff").is_empty());
        let changes = full_diff("", live).expect("full diff");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].key, "tools.flag");
        assert_eq!(changes[0].kind, crate::contracts::ChangeKind::Remove);
        assert_eq!(changes[0].after, None);
    }

    #[test]
    fn full_diff_reports_array_of_tables_removals_with_a_summary() {
        let changes = full_diff("", "[[host_rules]]\nname = \"a\"\n").expect("full diff");
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].key, "host_rules");
        assert_eq!(changes[0].before.as_deref(), Some("[1 个表]"));
        assert_eq!(changes[1].key, "host_rules[0].name");
    }

    #[test]
    fn full_diff_includes_empty_tables_and_changes_inside_equal_length_arrays() {
        let before = "values = [{ api_key = 'private', name = 'old' }]\n[empty]\n";
        let after = "values = [{ api_key = 'changed', name = 'new' }]\n";
        let changes = full_diff(after, before).unwrap();
        assert!(changes.iter().any(|change| change.key == "empty"));
        assert!(changes.iter().any(|change| change.key == "values[0].name" && change.after.as_deref() == Some("new")));
        let secret = changes.iter().find(|change| change.key == "values[0].api_key").unwrap();
        assert_eq!(secret.before.as_deref(), Some(crate::redact::REDACTED));
        assert_eq!(secret.after.as_deref(), Some(crate::redact::REDACTED));
    }
}
