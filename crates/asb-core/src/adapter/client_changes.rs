use super::{AdapterError, OverlayEntry, claude, codex, scrub_message};
use crate::contracts::{AppKind, CodexSubagentKey, CodexSubagentSettings, SettingValue, SettingsValues};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// An omitted key preserves the user file. An automatic key in this request
/// explicitly removes its user-level value.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientConfigurationChanges {
    pub settings: BTreeMap<String, SettingValue>,
    #[serde(default)]
    pub keep_file_values: BTreeSet<String>,
    pub claude_extra: Option<crate::claude_common::Extra>,
    #[serde(default)]
    pub subagents: BTreeMap<CodexSubagentKey, SettingValue>,
    pub permission_profile: Option<SettingValue>,
}

pub(super) fn setting_entry(value: &SettingValue) -> OverlayEntry {
    match value {
        SettingValue::Automatic => OverlayEntry::RemoveIfPresent,
        SettingValue::Explicit { value } => OverlayEntry::Set(value.clone()),
    }
}

fn rejected(message: impl Into<String>) -> AdapterError {
    AdapterError { message: scrub_message(message), line: None }
}

pub fn render_client_changes(
    app: AppKind,
    current: &str,
    changes: &ClientConfigurationChanges,
) -> Result<String, AdapterError> {
    if (app == AppKind::Codex && changes.claude_extra.is_some())
        || (app == AppKind::Claude && (!changes.subagents.is_empty() || changes.permission_profile.is_some()))
    {
        return Err(rejected("当前客户端不支持提交的配置分区"));
    }
    let values = SettingsValues {
        settings: changes.settings.clone(),
        claude_extra: changes.claude_extra.clone().unwrap_or_default(),
    };
    values.validate_client_settings(app).map_err(|error| rejected(error.to_string()))?;
    for key in &changes.keep_file_values {
        if changes.settings.contains_key(key) || !crate::ownership::setting_spec(app, key)
            .is_some_and(|spec| spec.owner == crate::ownership::SettingOwner::Client)
        {
            return Err(rejected("沿用文件值的字段必须属于客户端，且不能同时写入或移除"));
        }
    }
    let entries = changes.settings.iter().map(|(key, value)| (key.clone(), setting_entry(value))).collect();
    match app {
        AppKind::Codex => render_codex(current, changes, entries),
        AppKind::Claude => {
            let rendered = if changes.settings.is_empty() { current.to_string() }
                else { claude::render_entries(current, entries)? };
            if let Some(extra) = &changes.claude_extra {
                crate::claude_common::validate_shared_write(&rendered, extra).map_err(rejected)?;
                crate::claude_common::apply(&rendered, extra).map_err(rejected)
            } else {
                Ok(rendered)
            }
        }
    }
}

fn render_codex(
    current: &str,
    changes: &ClientConfigurationChanges,
    mut entries: Vec<(String, OverlayEntry)>,
) -> Result<String, AdapterError> {
    let mut subagents = CodexSubagentSettings::automatic();
    for (&key, value) in &changes.subagents {
        subagents.set(key, value.clone());
        entries.push((key.path(), setting_entry(value)));
    }
    subagents.validate().map_err(|error| rejected(error.to_string()))?;
    if changes.subagents.contains_key(&CodexSubagentKey::MaxConcurrentThreadsPerSession) {
        entries.push((codex::RETIRED_MAX_THREADS_KEY.into(), OverlayEntry::RemoveIfPresent));
    }
    if !changes.subagents.is_empty() {
        entries.push(("agents".into(), OverlayEntry::RemoveTableIfEmpty));
    }
    if let Some(value) = &changes.permission_profile {
        codex::validate_permission_selection(value)?;
        entries.push((codex::DEFAULT_PERMISSIONS_KEY.into(), setting_entry(value)));
    }
    codex::render_entries(current, entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::ConfigValue;

    #[test]
    fn one_claude_edit_preserves_external_rules_and_other_preferences() {
        let current = r#"{"language":"en","permissions":{"deny":["Read(.env)"],"blockReadsOutsideWorkingDirectories":true},"statusLine":{"type":"command"}}"#;
        let patch = ClientConfigurationChanges {
            settings: BTreeMap::from([("language".into(), SettingValue::Explicit { value: ConfigValue::Str("zh-CN".into()) })]),
            ..Default::default()
        };
        let rendered = render_client_changes(AppKind::Claude, current, &patch).unwrap();
        let changes = super::super::full_diff(AppKind::Claude, &rendered, current).unwrap();
        assert_eq!(changes.iter().map(|change| change.key.as_str()).collect::<Vec<_>>(), ["language"]);
    }

    #[test]
    fn a_kept_file_value_leaves_exact_bytes_and_cannot_also_be_removed() {
        let current = "{\"language\":\"en\"}\n";
        let mut patch = ClientConfigurationChanges {
            keep_file_values: BTreeSet::from(["language".into()]), ..Default::default()
        };
        assert_eq!(render_client_changes(AppKind::Claude, current, &patch).unwrap(), current);
        patch.settings.insert("language".into(), SettingValue::Automatic);
        assert!(render_client_changes(AppKind::Claude, current, &patch).is_err());
        patch.keep_file_values.clear();
        assert_eq!(render_client_changes(AppKind::Claude, current, &patch).unwrap(), "{}");
    }

    #[test]
    fn one_subagent_removal_preserves_the_other_agent_keys_and_legacy_client_values() {
        let current = "allow_login_shell = false\n[agents]\nenabled = false\ninterrupt_message = true\n";
        let patch = ClientConfigurationChanges {
            subagents: BTreeMap::from([(CodexSubagentKey::InterruptMessage, SettingValue::Automatic)]),
            ..Default::default()
        };
        let rendered = render_client_changes(AppKind::Codex, current, &patch).unwrap();
        let changes = super::super::full_diff(AppKind::Codex, &rendered, current).unwrap();
        assert_eq!(changes.iter().map(|change| change.key.as_str()).collect::<Vec<_>>(), ["agents.interrupt_message"]);
    }

    #[test]
    fn codex_permission_selection_requires_a_candidate_without_legacy_sandbox() {
        let current = "sandbox_mode = 'workspace-write'\n[tui]\nanimations = true\n";
        let mut patch = ClientConfigurationChanges {
            permission_profile: Some(SettingValue::Explicit { value: ConfigValue::Str(":workspace".into()) }),
            ..Default::default()
        };
        assert!(render_client_changes(AppKind::Codex, current, &patch).is_err());
        patch.settings.insert("sandbox_mode".into(), SettingValue::Automatic);
        let rendered = render_client_changes(AppKind::Codex, current, &patch).unwrap();
        assert!(rendered.contains("default_permissions"));
        assert!(rendered.contains("animations = true"));
        assert!(!rendered.contains("sandbox_mode"));
    }
}
