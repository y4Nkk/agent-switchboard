//! Application-owned common-settings commands.
//!
//! These commands never read, lock, back up, or write a Codex or Claude Code
//! configuration file. They expose the typed ownership catalog, persist plain
//! parameter values in `configuration/common/{client}.json`, and can render a
//! read-only common-settings fragment. Applying those values remains the
//! selected supplier's switch transaction.

use super::error::{blocking, state, store_error, CommandError};
use asb_core::contracts::{
    AppKind, CommonSettings, CommonSettingsPreview, CommonSettingsSnapshot, ConfigValue,
};
use asb_core::ownership::{
    self, ChoiceControl, OfficialSettingDisposition, SettingControl, SettingOwner,
};
use serde::Serialize;
use tauri::AppHandle;

/// A catalog option available for one common choice control.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonChoiceOption {
    pub value: String,
    pub label: String,
}

/// The directory-defined default for one parameter. The renderer shows it as
/// the target of the group's "恢复默认值" action; it is a plain value, never
/// a remove instruction.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonDefaultValue {
    pub bool_value: Option<bool>,
    pub str_value: Option<String>,
}

/// One editor-safe projection of the ownership directory. The renderer never
/// receives a client path or arbitrary config key: every parameter comes from
/// the shared catalog and is therefore known to be a general setting.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonSettingSpec {
    pub key: String,
    pub label: String,
    pub group: String,
    /// `toggle`, `slider`, or `segment`.
    pub control: String,
    pub default: CommonDefaultValue,
    pub options: Vec<CommonChoiceOption>,
}

/// One official configuration family and its actual ownership boundary. This
/// gives the renderer an exhaustive directory without handing it arbitrary
/// file paths or a second write mechanism.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialSettingDirectoryEntry {
    pub title: String,
    pub paths: Vec<String>,
    /// `direct`, `separateModule`, or `preserveOnly`.
    pub disposition: String,
    pub detail: String,
}

/// Complete typed input required by the common settings page for one client.
/// `settings` and `settings_hash` are application state only, not a rendered
/// client configuration candidate.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonSettingsEditor {
    pub app: AppKind,
    pub settings: CommonSettings,
    pub settings_hash: String,
    pub groups: Vec<String>,
    pub specs: Vec<CommonSettingSpec>,
    pub directory: Vec<OfficialSettingDirectoryEntry>,
}

fn directory_catalog(target: AppKind) -> Vec<OfficialSettingDirectoryEntry> {
    ownership::official_setting_directory(target)
        .into_iter()
        .map(|entry| OfficialSettingDirectoryEntry {
            title: entry.title.to_string(),
            paths: std::iter::once(entry.path)
                .chain(entry.related_paths.iter().copied())
                .map(str::to_string)
                .collect(),
            disposition: match entry.disposition {
                OfficialSettingDisposition::Direct => "direct",
                OfficialSettingDisposition::SeparateModule => "separateModule",
                OfficialSettingDisposition::PreserveOnly => "preserveOnly",
            }
            .to_string(),
            detail: entry.detail.to_string(),
        })
        .collect()
}

fn default_value(default: Option<&ConfigValue>) -> CommonDefaultValue {
    match default {
        Some(ConfigValue::Bool(value)) => CommonDefaultValue {
            bool_value: Some(*value),
            str_value: None,
        },
        Some(ConfigValue::Str(value)) => CommonDefaultValue {
            bool_value: None,
            str_value: Some(value.clone()),
        },
        Some(other) => CommonDefaultValue {
            bool_value: None,
            str_value: Some(other.display()),
        },
        None => CommonDefaultValue {
            bool_value: None,
            str_value: None,
        },
    }
}

fn editor_catalog(target: AppKind) -> Vec<CommonSettingSpec> {
    ownership::setting_specs(target)
        .into_iter()
        .filter(|spec| spec.owner == SettingOwner::Common)
        .map(|spec| {
            let (control, options) = match spec.control {
                SettingControl::Toggle => ("toggle".to_string(), vec![]),
                SettingControl::Choice { presentation } => (
                    match presentation {
                        ChoiceControl::Slider => "slider",
                        ChoiceControl::Segment => "segment",
                    }
                    .to_string(),
                    spec.allowed_values
                        .iter()
                        .map(|option| CommonChoiceOption {
                            value: option.value.to_string(),
                            label: option.label.to_string(),
                        })
                        .collect(),
                ),
                SettingControl::None => unreachable!("common setting must have an editor control"),
            };
            CommonSettingSpec {
                key: spec.key.to_string(),
                label: spec
                    .label
                    .expect("common setting must have a visible label")
                    .to_string(),
                group: spec
                    .group
                    .expect("common setting must have an editor group")
                    .to_string(),
                control,
                default: default_value(spec.default.as_ref()),
                options,
            }
        })
        .collect()
}

fn editor_from_snapshot(target: AppKind, snapshot: CommonSettingsSnapshot) -> CommonSettingsEditor {
    CommonSettingsEditor {
        app: target,
        settings_hash: snapshot.settings_hash,
        settings: snapshot.settings,
        groups: ownership::common_groups(target)
            .iter()
            .map(|group| group.to_string())
            .collect(),
        specs: editor_catalog(target),
        directory: directory_catalog(target),
    }
}

/// Reads the application-owned parameter values and the catalog that can edit
/// them. This command deliberately does not access the real client
/// configuration.
#[tauri::command]
pub async fn get_common_settings_editor(
    app: AppHandle,
    target: AppKind,
) -> Result<CommonSettingsEditor, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        state
            .configuration()
            .get_common_settings(target)
            .map(|snapshot| editor_from_snapshot(target, snapshot))
            .map_err(store_error)
    })
    .await
}

/// Replaces one client's common settings after an optimistic revision check.
/// Saving here cannot modify either real client config file; the supplier
/// switch path is the only projection writer.
#[tauri::command]
pub async fn save_common_settings(
    app: AppHandle,
    target: AppKind,
    settings: CommonSettings,
    expected_settings_hash: String,
) -> Result<CommonSettingsSnapshot, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        state
            .configuration()
            .save_common_settings(target, settings, &expected_settings_hash)
            .map_err(|message| CommandError::new("common-settings-save-failed", message))
    })
    .await
}

/// Renders the editor's current draft as an application-owned common-settings
/// fragment. It is pure: no client file is read or written, and no provider
/// data can enter this preview.
#[tauri::command]
pub async fn preview_common_settings(
    target: AppKind,
    settings: CommonSettings,
) -> Result<CommonSettingsPreview, CommandError> {
    blocking(move || {
        let content =
            asb_core::adapter::render_common_settings(target, &settings).map_err(|error| {
                CommandError::new("common-settings-preview-failed", error.to_string())
            })?;
        Ok(CommonSettingsPreview {
            app: target,
            target: format!("{} 通用配置片段", target.config_label()),
            content,
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_settings_catalog_contains_only_common_owned_controls() {
        for app in [AppKind::Codex, AppKind::Claude] {
            let catalog = editor_catalog(app);
            assert!(!catalog.is_empty());
            for setting in &catalog {
                assert_eq!(
                    ownership::owner_for(app, &setting.key),
                    SettingOwner::Common
                );
                assert!(!setting.label.is_empty());
                assert!(!setting.group.is_empty());
                assert!(matches!(
                    setting.control.as_str(),
                    "toggle" | "slider" | "segment"
                ));
                if setting.control == "toggle" {
                    assert!(setting.options.is_empty());
                    assert!(setting.default.bool_value.is_some());
                } else {
                    assert!(!setting.options.is_empty());
                    assert!(setting.default.str_value.is_some());
                }
            }
        }
    }

    #[test]
    fn editor_has_no_client_file_projection_data() {
        let editor = editor_from_snapshot(
            AppKind::Codex,
            CommonSettingsSnapshot {
                settings: ownership::default_common_settings(AppKind::Codex),
                settings_hash: "revision".to_string(),
            },
        );
        let json = serde_json::to_value(editor).expect("editor serializes");
        assert!(json.get("target").is_none());
        assert!(json.get("content").is_none());
        assert!(json.get("preview").is_none());
        // The official directory may name a path family and its ownership
        // boundary, but it never carries a rendered client-file candidate or
        // patch instruction.
        let text = serde_json::to_string(&json).unwrap();
        for forbidden in ["Leave", "Remove", "backupPath", "renderedHash"] {
            assert!(!text.contains(forbidden));
        }
    }

    #[test]
    fn preview_renders_only_the_common_settings_fragment() {
        let settings = ownership::default_common_settings(AppKind::Codex);
        let preview =
            tauri::async_runtime::block_on(preview_common_settings(AppKind::Codex, settings))
                .expect("preview");
        assert_eq!(preview.app, AppKind::Codex);
        assert!(preview.target.contains("config.toml"));
        assert!(preview.content.contains("所有通用设置均为默认值"));
    }
}
