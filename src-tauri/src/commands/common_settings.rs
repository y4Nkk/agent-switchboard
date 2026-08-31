//! General-configuration overlay commands: the per-client toggle and choice
//! catalogs, the side-effect-free preview, and the transactional apply.

use super::error::{
    blocking, observe, record_audit_or_warn, require_write_confirmation, state, store_error,
    CommandError,
};
use crate::runtime_log::RuntimeLogAction;
use asb_core::contracts::{AppKind, CommonConfigPatch, SwitchLog, SwitchOp};
use asb_core::{adapter, ownership};
use asb_switch::io::{FsIo, SwitchIo};
use asb_switch::{execute_common, read_common_preview, CommonRequest, FilePreview, SwitchOutcome};
use serde::Serialize;
use tauri::AppHandle;

#[tauri::command]
pub async fn get_common(
    app: AppHandle,
    target: AppKind,
) -> Result<CommonConfigPatch, CommandError> {
    let state = state(&app)?;
    blocking(move || state.get_common(target).map_err(store_error)).await
}

#[tauri::command]
pub async fn set_common(
    app: AppHandle,
    target: AppKind,
    patch: CommonConfigPatch,
) -> Result<(), CommandError> {
    observe(RuntimeLogAction::CommonSettingsSaved, async move {
        let state = state(&app)?;
        blocking(move || {
            state
                .set_common(target, patch)
                .map_err(|error| CommandError::new("common-save-failed", error))
        })
        .await
    })
    .await
}

/// One settings-page checkbox: the official toggle plus whether the target
/// file currently carries its applied line.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleState {
    pub key: String,
    pub label: String,
    pub line: String,
    /// The value the checked line carries (e.g. `spinnerTipsEnabled = false`).
    pub applied: bool,
    /// Whether the target file currently carries the applied line.
    pub value: bool,
    pub group: String,
}

#[tauri::command]
pub async fn common_toggles(
    app: AppHandle,
    target: AppKind,
) -> Result<Vec<ToggleState>, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let path = state
            .target(target)
            .map_err(|error| CommandError::new("config-path-unavailable", error))?;
        let text = FsIo.read_file(&path).unwrap_or_default();
        Ok(ownership::common_toggles(target)
            .iter()
            .map(|spec| ToggleState {
                key: spec.key.to_string(),
                label: spec.label.to_string(),
                line: spec.line.to_string(),
                applied: spec.applied,
                value: adapter::toggle_is_active(target, &text, spec.key, spec.applied),
                group: spec.group.to_string(),
            })
            .collect())
    })
    .await
}

/// One selectable value of a multi-detent general setting.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceOptionState {
    pub value: String,
    pub label: String,
}

/// One multi-detent general setting: the official values plus the line the
/// target file currently carries (`None` = line absent, client default).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceState {
    pub key: String,
    pub label: String,
    pub group: String,
    /// "slider" or "segment"; how the settings page renders the choice.
    pub control: String,
    pub options: Vec<ChoiceOptionState>,
    /// The raw scalar at the key, even when it matches no catalog option.
    pub value: Option<String>,
}

/// The choice catalog for one client: section order plus the choices.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonChoicesState {
    /// Section order for the settings page, the single owner of grouping.
    pub groups: Vec<String>,
    pub choices: Vec<ChoiceState>,
}

#[tauri::command]
pub async fn common_choices(
    app: AppHandle,
    target: AppKind,
) -> Result<CommonChoicesState, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let path = state
            .target(target)
            .map_err(|error| CommandError::new("config-path-unavailable", error))?;
        let text = FsIo.read_file(&path).unwrap_or_default();
        let mut choices = Vec::new();
        for spec in ownership::common_choices(target) {
            let value = adapter::owned_scalar(target, &text, spec.key)
                .map_err(|error| CommandError::new("common-choice-unreadable", error.message))?;
            choices.push(ChoiceState {
                key: spec.key.to_string(),
                label: spec.label.to_string(),
                group: spec.group.to_string(),
                control: match spec.control {
                    ownership::ChoiceControl::Slider => "slider",
                    ownership::ChoiceControl::Segment => "segment",
                }
                .to_string(),
                options: spec
                    .options
                    .iter()
                    .map(|option| ChoiceOptionState {
                        value: option.value.to_string(),
                        label: option.label.to_string(),
                    })
                    .collect(),
                value,
            });
        }
        Ok(CommonChoicesState {
            groups: ownership::common_groups(target)
                .iter()
                .map(|group| group.to_string())
                .collect(),
            choices,
        })
    })
    .await
}

/// Side-effect-free general-config preview: the summary diff plus the
/// pretty-printed candidate file content, computed without locking or
/// writing.
#[tauri::command]
pub async fn preview_common(app: AppHandle, target: AppKind) -> Result<FilePreview, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let patch = state.get_common(target).map_err(store_error)?;
        let target_path = state
            .target(target)
            .map_err(|error| CommandError::new("config-path-unavailable", error))?;
        let backup_dir = state.backup_dir();
        let mut file = read_common_preview(
            &FsIo,
            &CommonRequest {
                target: &target_path,
                app: target,
                common: &patch,
                backup_dir: &backup_dir,
                expected_hash: "",
                expected_rendered_hash: "",
            },
        )
        .map_err(map_common_preview_error)?;
        file.preview.target = target_path.to_string_lossy().to_string();
        Ok(file)
    })
    .await
}

fn map_common_preview_error(error: asb_switch::SwitchError) -> CommandError {
    match error {
        asb_switch::SwitchError::PlanRejected { message, line } => CommandError::new(
            "common-patch-invalid",
            format!(
                "{message}{}",
                line.map(|line| format!("（第 {line} 行）"))
                    .unwrap_or_default()
            ),
        ),
        other => CommandError::from(other),
    }
}

/// Applies the general-configuration overlay on its own through the full
/// executor transaction (lock → backup → atomic replace → verify).
#[tauri::command]
pub async fn apply_common(
    app: AppHandle,
    target: AppKind,
    patch: CommonConfigPatch,
    confirm_write: bool,
) -> Result<SwitchOutcome, CommandError> {
    observe(RuntimeLogAction::CommonSettingsApplied, async move {
        require_write_confirmation(confirm_write, "写入通用配置")?;
        let state = state(&app)?;
        blocking(move || {
            patch
                .validate()
                .map_err(|error| CommandError::new("common-patch-invalid", error.to_string()))?;
            let target_path = state
                .target(target)
                .map_err(|error| CommandError::new("config-path-unavailable", error))?;
            let backup_dir = state.backup_dir();
            let preview = read_common_preview(
                &FsIo,
                &CommonRequest {
                    target: &target_path,
                    app: target,
                    common: &patch,
                    backup_dir: &backup_dir,
                    expected_hash: "",
                    expected_rendered_hash: "",
                },
            )
            .map_err(map_common_preview_error)?;
            let mut outcome = execute_common(
                &FsIo,
                &CommonRequest {
                    target: &target_path,
                    app: target,
                    common: &patch,
                    backup_dir: &backup_dir,
                    expected_hash: &preview.content_hash,
                    expected_rendered_hash: &preview.rendered_hash,
                },
            )
            .map_err(CommandError::from)?;
            outcome.preview.target = target_path.to_string_lossy().to_string();
            record_audit_or_warn(
                state.record_switch(SwitchLog {
                    app: target,
                    profile_id: None,
                    profile_name: None,
                    content_hash: outcome.final_hash.clone(),
                    backup_id: outcome.backup.id.clone(),
                    at: outcome.backup.created_at.clone(),
                    operation: SwitchOp::CommonSettings,
                }),
                &mut outcome.warnings,
            );
            crate::tray::refresh(&app);
            Ok(outcome)
        })
        .await
    })
    .await
}
