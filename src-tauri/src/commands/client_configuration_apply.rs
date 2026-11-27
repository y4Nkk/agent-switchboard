//! Hash-bound client configuration application and scoped reset transactions.
use super::{
    error::{blocking, require_write_confirmation, state, store_error, CommandError},
    ConfigWriteGate,
};
use asb_core::{
    adapter::{ClientConfigurationChanges, PreviewDiff, RemovableClientField},
    contracts::{AppKind, ConfigWriteRecord, SettingValue, SettingsValues, WriteOperation},
    ownership,
};
use asb_switch::{execute_rendered, preview_rendered, sha256_hex, FsIo, RenderedWriteRequest};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, io::ErrorKind, path::PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientConfigurationApplyPreview {
    pub file: asb_switch::FilePreview,
    pub settings_hash: String,
    pub target_existed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientConfigurationResetPreview {
    #[serde(flatten)]
    pub candidate: ClientConfigurationApplyPreview,
    pub removable_fields: Vec<RemovableClientField>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientConfigurationChangesPreview {
    #[serde(flatten)]
    pub candidate: ClientConfigurationApplyPreview,
    pub settings_result_hash: String,
}

/// The non-draft client-configuration mutations. The backend owns their
/// exact scopes so a UI action cannot accidentally persist unrelated edits.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClientConfigurationResetKind {
    NativeDefaults,
    ClearExtraConfiguration,
}

impl ClientConfigurationResetKind {
    fn settings(
        self,
        target: AppKind,
        saved: SettingsValues,
    ) -> Result<SettingsValues, CommandError> {
        match self {
            Self::NativeDefaults => {
                let mut settings = ownership::default_client_settings(target);
                if target == AppKind::Claude {
                    settings.claude_extra = saved.claude_extra;
                }
                Ok(settings)
            }
            Self::ClearExtraConfiguration => {
                if target != AppKind::Claude {
                    return Err(CommandError::keyed(
                        "client-configuration-rejected",
                        "errors.cfg.clearExtraClaudeOnly",
                        "仅 Claude 支持清空 ASB 管理的额外通用配置",
                    ));
                }
                let mut settings = saved;
                settings.claude_extra.clear();
                Ok(settings)
            }
        }
    }

    fn write_label(self) -> &'static str {
        match self {
            Self::NativeDefaults => "恢复客户端原生默认值",
            Self::ClearExtraConfiguration => "清空 ASB 管理的额外通用配置",
        }
    }

    fn backup_reason(self, selected: &BTreeSet<String>) -> &'static str {
        match self {
            Self::NativeDefaults if !selected.is_empty() => "client-configuration-native-defaults-unmanaged",
            Self::NativeDefaults => "client-configuration-native-defaults",
            Self::ClearExtraConfiguration => "client-configuration-clear-extra-configuration",
        }
    }

    fn diff_scope(self) -> PreviewDiff {
        match self {
            Self::NativeDefaults => PreviewDiff::Full,
            Self::ClearExtraConfiguration => PreviewDiff::Owned,
        }
    }
}

pub(super) struct ClientFile {
    pub path: PathBuf,
    pub text: String,
    pub exists: bool,
}

impl ClientFile {
    pub(super) fn read(state: &crate::local_state::LocalState, target: AppKind) -> Result<Self, CommandError> {
        let path = state.target(target).map_err(|error| CommandError::new("config-path-unavailable", error))?;
        let (text, exists) = match std::fs::read_to_string(&path) {
            Ok(text) => (text, true),
            Err(error) if error.kind() == ErrorKind::NotFound => (
                if target == AppKind::Claude { "{}".into() } else { String::new() }, false,
            ),
            Err(_) => return Err(CommandError::keyed("client-configuration-unreadable", "errors.cfg.clientConfigUnreadable", "无法读取真实客户端配置文件")),
        };
        Ok(Self { path, text, exists })
    }

    fn verify_source(&self, hash: &str, exists: bool) -> Result<(), CommandError> {
        if sha256_hex(&self.text) != hash || self.exists != exists {
            return Err(CommandError::keyed("client-configuration-preview-stale", "errors.cfg.previewStaleTargetChanged", "真实配置或候选已变化，请重新读取"));
        }
        Ok(())
    }
}

fn candidate(
    state: &crate::local_state::LocalState, target: AppKind,
    file: &ClientFile, mut settings: SettingsValues, changes: &ClientConfigurationChanges,
    directory: Option<&str>,
) -> Result<(SettingsValues, String), CommandError> {
    let rendered = asb_core::adapter::render_client_changes(target, &file.text, changes)
        .map_err(|error| CommandError::new("client-configuration-preview-failed", error.to_string()))?;
    super::client_configuration_sources::validate_client_changes(state, target, changes, directory)?;
    for key in &changes.keep_file_values {
        if !matches!(settings.settings.remove(key), Some(SettingValue::Explicit { .. })) {
            return Err(CommandError::new("client-configuration-rejected", "该字段没有已保存的 ASB 共用值"));
        }
    }
    for (key, value) in &changes.settings {
        if matches!(value, SettingValue::Automatic) { settings.settings.remove(key); }
        else { settings.settings.insert(key.clone(), value.clone()); }
    }
    if let Some(extra) = &changes.claude_extra { settings.claude_extra.clone_from(extra); }
    Ok((settings, rendered))
}

fn settings_result_hash(settings: &SettingsValues) -> Result<String, CommandError> {
    let text = serde_json::to_string(settings).map_err(|error| CommandError::new("client-configuration-rejected", error.to_string()))?;
    Ok(sha256_hex(&text))
}

pub(super) fn commit_rendered_client_configuration(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    target: AppKind,
    current: &str,
    existed: bool,
    settings: SettingsValues,
    rendered: &str,
    expected_hash: &str,
    expected_rendered_hash: &str,
    expected_settings_hash: &str,
    expected_target_existed: bool,
    reason: &'static str,
) -> Result<(), CommandError> {
    if sha256_hex(current) != expected_hash ||
        sha256_hex(rendered) != expected_rendered_hash ||
        existed != expected_target_existed {
        return Err(CommandError::keyed("client-configuration-preview-stale", "errors.cfg.previewStaleTargetChanged", "真实配置或候选已变化，请重新预览"));
    }
    let config = state.configuration();
    let before = config.get_client_settings(target).map_err(store_error)?;
    if before.settings_hash != expected_settings_hash {
        return Err(CommandError::keyed("client-configuration-preview-stale", "errors.cfg.previewStaleSettingsChanged", "通用配置意图已变化，请重新预览"));
    }
    if current == rendered && before.settings == settings {
        return Ok(());
    }
    let file_target = state.target(target).map_err(|error| CommandError::new("config-path-unavailable", error))?;
    let backup_dir = state.backup_dir();
    super::switching::transaction::begin_client_configuration(
        state, gateway, target, expected_rendered_hash, existed || current != rendered, Some((&before, &settings)),
    )?;
    let execution = execute_rendered(&FsIo, &RenderedWriteRequest {
        target: &file_target,
        app: target,
        backup_dir: &backup_dir,
        expected_hash,
        expected_target_existed,
        rendered,
        reason,
    }, |outcome| {
        super::switching::transaction::commit_client_settings(state, &outcome.backup)?;
        config.record_config_write(ConfigWriteRecord {
            app: target,
            profile_id: None,
            profile_name: None,
            content_hash: outcome.final_hash.clone(),
            backup_id: outcome.backup.id.clone(),
            at: outcome.backup.created_at.clone(),
            operation: WriteOperation::Projection,
        }).map_err(|error| error.to_string())
    });
    super::switching::transaction::finish(state, gateway, execution)?;
    Ok(())
}

fn render_reset_configuration(
    target: AppKind,
    current: &str,
    kind: ClientConfigurationResetKind,
    settings: &SettingsValues,
    codex_fragment: &str,
    selected: &BTreeSet<String>,
) -> Result<(String, Vec<RemovableClientField>), CommandError> {
    match kind {
        ClientConfigurationResetKind::NativeDefaults => {
            let (pruned, available) = asb_core::adapter::select_unmanaged_entries(target, current, codex_fragment, selected)
                .map_err(|error| {
                    CommandError::new("client-configuration-preview-failed", error.to_string())
                })?;
            asb_core::adapter::reset_client_settings(target, &pruned)
                .map(|rendered| (rendered, available))
                .map_err(|error| CommandError::new("client-configuration-preview-failed", error.to_string()))
        }
        ClientConfigurationResetKind::ClearExtraConfiguration => {
            if !selected.is_empty() {
                return Err(CommandError::new("client-configuration-rejected", "清空额外配置不能同时移除界面外字段"));
            }
            settings.validate_client_settings(target)
                .map_err(|error| CommandError::new("client-configuration-rejected", error.to_string()))?;
            asb_core::claude_common::apply(current, &settings.claude_extra)
                .map(|rendered| (rendered, Vec::new()))
                .map_err(|error| CommandError::new("client-configuration-preview-failed", error))
        }
    }
}

fn reset_candidate(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    target: AppKind,
    kind: ClientConfigurationResetKind,
    saved: SettingsValues,
    selected: &BTreeSet<String>,
) -> Result<(ClientFile, SettingsValues, String, Vec<RemovableClientField>), CommandError> {
    let file = ClientFile::read(state, target)?;
    super::switching::validate_client_configuration_backup(state, gateway, target, &file.text, file.exists)?;
    let settings = kind.settings(target, saved)?;
    let fragment = if target == AppKind::Codex && matches!(kind, ClientConfigurationResetKind::NativeDefaults) {
        crate::codex_common::view_fragment(state.root())
            .map_err(|error| CommandError::new("client-configuration-preview-failed", error))?.text
    } else { String::new() };
    let (rendered, available) = render_reset_configuration(target, &file.text, kind, &settings, &fragment, selected)?;
    Ok((file, settings, rendered, available))
}

fn preview_reset(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    target: AppKind,
    kind: ClientConfigurationResetKind,
    selected: &BTreeSet<String>,
) -> Result<ClientConfigurationResetPreview, CommandError> {
    let stored = state.configuration().get_client_settings(target).map_err(store_error)?;
    let (source, _, rendered, removable_fields) = reset_candidate(state, gateway, target, kind, stored.settings, selected)?;
    let file = preview_rendered(
        target,
        &source.path,
        &state.backup_dir(),
        &source.text,
        &rendered,
        kind.diff_scope(),
    )
    .map_err(CommandError::from)?;
    Ok(ClientConfigurationResetPreview {
        candidate: ClientConfigurationApplyPreview { file, settings_hash: stored.settings_hash, target_existed: source.exists },
        removable_fields,
    })
}

#[tauri::command]
pub async fn preview_client_configuration_apply(
    app: AppHandle,
    target: AppKind,
    expected_source_hash: String,
    expected_source_exists: bool,
    expected_settings_hash: String,
    changes: ClientConfigurationChanges,
    directory: Option<String>,
) -> Result<ClientConfigurationChangesPreview, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let stored = state.configuration().get_client_settings(target).map_err(store_error)?;
        if stored.settings_hash != expected_settings_hash {
            return Err(CommandError::keyed("client-configuration-preview-stale", "errors.cfg.previewStaleSettingsChanged", "通用配置意图已变化，请重新读取"));
        }
        let source = ClientFile::read(&state, target)?;
        source.verify_source(&expected_source_hash, expected_source_exists)?;
        let (settings, rendered) = candidate(&state, target, &source, stored.settings, &changes, directory.as_deref())?;
        let file = preview_rendered(
            target,
            &source.path,
            &state.backup_dir(),
            &source.text,
            &rendered,
            PreviewDiff::Owned,
        )
        .map_err(CommandError::from)?;
        Ok(ClientConfigurationChangesPreview {
            candidate: ClientConfigurationApplyPreview { file, settings_hash: stored.settings_hash, target_existed: source.exists },
            settings_result_hash: settings_result_hash(&settings)?,
        })
    }).await
}

#[tauri::command]
pub async fn preview_client_configuration_reset(
    app: AppHandle,
    target: AppKind,
    reset_kind: ClientConfigurationResetKind,
    selected: BTreeSet<String>,
    expected_source_hash: Option<String>,
) -> Result<ClientConfigurationResetPreview, CommandError> {
    let state = state(&app)?;
    let gateway = app.state::<crate::gateway::GatewayController>().inner().clone();
    blocking(move || {
        let preview = preview_reset(&state, &gateway, target, reset_kind, &selected)?;
        if expected_source_hash.as_ref().is_some_and(|hash| hash != &preview.candidate.file.content_hash) {
            return Err(CommandError::keyed("client-configuration-preview-stale", "errors.cfg.previewStaleTargetChanged", "真实配置已变化，请重新打开恢复预览"));
        }
        Ok(preview)
    }).await
}

#[tauri::command]
pub async fn commit_client_configuration_apply(
    app: AppHandle,
    target: AppKind,
    expected_hash: String,
    expected_rendered_hash: String,
    expected_settings_hash: String,
    expected_settings_result_hash: String,
    expected_target_existed: bool,
    changes: ClientConfigurationChanges,
    directory: Option<String>,
    confirm_write: bool,
) -> Result<(), CommandError> {
    require_write_confirmation(confirm_write, "应用客户端配置")?;
    let state = state(&app)?;
    let gate = app.try_state::<ConfigWriteGate>()
        .ok_or_else(|| CommandError::keyed("app-state-unavailable", "errors.cfg.writeGateNotInitialized", "写入闸门尚未初始化"))?
        .inner().clone();
    blocking(move || {
        let _guard = gate.lock().map_err(|error| CommandError::new("config-write-gate-unavailable", error))?;
        crate::commands::switching::ensure_profile_save_recovered(&app)?;
        let source = ClientFile::read(&state, target)?;
        source.verify_source(&expected_hash, expected_target_existed)?;
        let before = state.configuration().get_client_settings(target).map_err(store_error)?;
        if before.settings_hash != expected_settings_hash {
            return Err(CommandError::keyed("client-configuration-preview-stale", "errors.cfg.previewStaleSettingsChanged", "通用配置意图已变化，请重新读取"));
        }
        let (settings, rendered) = candidate(&state, target, &source, before.settings, &changes, directory.as_deref())?;
        if settings_result_hash(&settings)? != expected_settings_result_hash {
            return Err(CommandError::keyed("client-configuration-preview-stale", "errors.cfg.previewStaleSettingsChanged", "预览中的 ASB 设置已变化，请重新预览"));
        }
        commit_rendered_client_configuration(
            &state,
            app.state::<crate::gateway::GatewayController>().inner(),
            target,
            &source.text,
            source.exists,
            settings,
            &rendered,
            &expected_hash,
            &expected_rendered_hash,
            &expected_settings_hash,
            expected_target_existed,
            "client-configuration-apply",
        )
    }).await
}

#[tauri::command]
pub async fn commit_client_configuration_reset(
    app: AppHandle,
    target: AppKind,
    reset_kind: ClientConfigurationResetKind,
    selected: BTreeSet<String>,
    expected_hash: String,
    expected_rendered_hash: String,
    expected_settings_hash: String,
    expected_target_existed: bool,
    confirm_write: bool,
) -> Result<(), CommandError> {
    require_write_confirmation(confirm_write, reset_kind.write_label())?;
    let state = state(&app)?;
    let gate = app.try_state::<ConfigWriteGate>()
        .ok_or_else(|| CommandError::keyed("app-state-unavailable", "errors.cfg.writeGateNotInitialized", "写入闸门尚未初始化"))?
        .inner().clone();
    blocking(move || {
        let _guard = gate.lock().map_err(|error| CommandError::new("config-write-gate-unavailable", error))?;
        crate::commands::switching::ensure_profile_save_recovered(&app)?;
        let stored = state.configuration().get_client_settings(target).map_err(store_error)?;
        let (source, settings, rendered, _) = reset_candidate(
            &state,
            app.state::<crate::gateway::GatewayController>().inner(),
            target,
            reset_kind,
            stored.settings,
            &selected,
        )?;
        commit_rendered_client_configuration(
            &state,
            app.state::<crate::gateway::GatewayController>().inner(),
            target,
            &source.text,
            source.exists,
            settings,
            &rendered,
            &expected_hash,
            &expected_rendered_hash,
            &expected_settings_hash,
            expected_target_existed,
            reset_kind.backup_reason(&selected),
        )
    }).await
}

#[cfg(test)]
#[path = "client_configuration_apply_tests.rs"]
mod tests;
