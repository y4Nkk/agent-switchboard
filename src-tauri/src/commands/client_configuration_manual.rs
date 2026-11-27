//! Confirmed manual edits for client configuration outside the typed UI fields.
//!
//! The renderer edits a redacted display copy. This command rehydrates unchanged
//! secret markers on the backend and uses the shared backup executor.

use super::{
    client_configuration_apply::{
        commit_rendered_client_configuration, ClientConfigurationApplyPreview, ClientFile,
    },
    error::{blocking, require_write_confirmation, state, store_error, CommandError},
    ConfigWriteGate,
};
use asb_core::adapter::PreviewDiff;
use asb_core::contracts::AppKind;
use asb_switch::{preview_rendered, rehydrate_display_content, sha256_hex};
use tauri::{AppHandle, Manager};

fn candidate(
    state: &crate::local_state::LocalState,
    target: AppKind,
    expected_source_hash: &str,
    display_content: &str,
) -> Result<(ClientFile, String), CommandError> {
    let file = ClientFile::read(state, target)?;
    if sha256_hex(&file.text) != expected_source_hash {
        return Err(CommandError::keyed("client-configuration-preview-stale", "errors.cfg.manualPreviewStale", "真实配置已变化，请重新读取后再编辑"));
    }
    let source = &file.text;
    if display_content == asb_switch::display_content(target, source) {
        return Err(CommandError::keyed("manual-configuration-no-change", "errors.cfg.manualNoChange", "手动配置没有变更"));
    }
    let manual = rehydrate_display_content(target, source, display_content).map_err(CommandError::from)?;
    let baseline = source;
    let manually_changed_owned = owned_changes(target, &manual, baseline)
        .map_err(|error| CommandError::new("manual-configuration-rejected", error.message))?;
    if !manually_changed_owned.is_empty() {
        let paths = manually_changed_owned.into_iter().collect::<Vec<_>>().join("、");
        return Err(CommandError::localized(
            "manual-configuration-rejected",
            "errors.cfg.manualOwnedFieldRejected",
            format!("手动配置只能修改界面未拥有的字段；请在对应界面修改：{paths}"),
            serde_json::json!({ "paths": paths }),
        ));
    }
    if target == AppKind::Codex {
        asb_core::adapter::codex::validate_permission_configuration(&manual)
            .map_err(|error| CommandError::new("manual-configuration-rejected", error.message))?;
    }
    Ok((file, manual))
}

fn owned_changes(target: AppKind, current: &str, previous: &str) -> Result<std::collections::BTreeSet<String>, asb_core::adapter::AdapterError> {
    use asb_core::{adapter, ownership};
    let mut changed: std::collections::BTreeSet<_> = adapter::owned_diff(target, current, previous)?
        .into_iter().map(|change| change.key).collect();
    let mut paths: Vec<String> = ownership::setting_specs(target).into_iter().map(|spec| spec.key.to_string()).collect();
    if target == AppKind::Codex {
        paths.push(adapter::codex::DEFAULT_PERMISSIONS_KEY.into());
        paths.extend(asb_core::CodexSubagentKey::ALL.into_iter().map(|key| key.path()));
    }
    for change in adapter::full_diff(target, current, previous)? {
        if paths.iter().any(|key| change.key == *key || change.key.strip_prefix(key.as_str())
            .is_some_and(|suffix| suffix.starts_with(['.', '[']))) {
            changed.insert(change.key);
        }
    }
    Ok(changed)
}

#[tauri::command]
pub async fn preview_manual_client_configuration(
    app: AppHandle,
    target: AppKind,
    expected_source_hash: String,
    display_content: String,
) -> Result<ClientConfigurationApplyPreview, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let stored = state.configuration().get_client_settings(target).map_err(store_error)?;
        let (source, rendered) = candidate(
            &state,
            target,
            &expected_source_hash,
            &display_content,
        )?;
        let file = preview_rendered(
            target,
            &source.path,
            &state.backup_dir(),
            &source.text,
            &rendered,
            PreviewDiff::Full,
        )
        .map_err(CommandError::from)?;
        Ok(ClientConfigurationApplyPreview {
            file,
            settings_hash: stored.settings_hash,
            target_existed: source.exists,
        })
    }).await
}

#[tauri::command]
pub async fn commit_manual_client_configuration(
    app: AppHandle,
    target: AppKind,
    expected_source_hash: String,
    expected_rendered_hash: String,
    expected_settings_hash: String,
    expected_target_existed: bool,
    display_content: String,
    confirm_write: bool,
) -> Result<(), CommandError> {
    require_write_confirmation(confirm_write, "应用手动客户端配置")?;
    let state = state(&app)?;
    let gate = app.try_state::<ConfigWriteGate>()
        .ok_or_else(|| CommandError::keyed("app-state-unavailable", "errors.cfg.writeGateNotInitialized", "写入闸门尚未初始化"))?
        .inner().clone();
    blocking(move || {
        let _guard = gate.lock().map_err(|error| CommandError::new("config-write-gate-unavailable", error))?;
        crate::commands::switching::ensure_profile_save_recovered(&app)?;
        let (source, rendered) = candidate(
            &state,
            target,
            &expected_source_hash,
            &display_content,
        )?;
        commit_rendered_client_configuration(
            &state,
            app.state::<crate::gateway::GatewayController>().inner(),
            target,
            &source.text,
            source.exists,
            state.configuration().get_client_settings(target).map_err(store_error)?.settings,
            &rendered,
            &expected_source_hash,
            &expected_rendered_hash,
            &expected_settings_hash,
            expected_target_existed,
            "manual-client-configuration",
        )
    }).await
}
