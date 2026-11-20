//! Application half of a configuration transaction; the executor owns file recovery.
mod catalog;
mod journal;
mod intent;
mod application;
mod settings;
#[cfg(test)]
mod backfill_tests;

use application::{complete, rollback_application, restore_application_snapshot};
pub(super) use intent::{begin, begin_with_codex_backfill_and_auth, begin_restore};
pub(in crate::commands) use intent::begin_client_configuration;
pub(in crate::commands) use settings::commit_client_settings;

use super::codex_backfill::{AppliedCodexBackfill, PreparedCodexBackfill};
use crate::commands::error::CommandError;
use crate::{
    gateway::{GatewayActivationSnapshot, GatewayController},
    local_state::LocalState,
};
use asb_core::{AppKind, ConfigWriteRecord, WriteOperation};
use asb_switch::{FsIo, PendingConfigWrite, SwitchIo};
#[cfg(test)]
pub(super) use catalog::apply_catalog_artifact;
use catalog::{restore_catalog, validate_catalog_target, verify_catalog_after, verify_catalog_before};
pub(super) use catalog::{stage_catalog, CatalogArtifact};
use journal::{clear, error, load, path, read_target};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SwitchIntent {
    version: u8,
    app: AppKind,
    target: String,
    profile_id: Option<String>,
    profile_hash: Option<String>,
    before_hash: String,
    before_existed: bool,
    after_hash: String,
    after_existed: bool,
    previous_route: GatewayActivationSnapshot,
    previous_write: Option<ConfigWriteRecord>,
    catalog: Option<CatalogArtifact>,
    #[serde(default)]
    codex_backfill: Option<CodexBackfillIntent>,
    #[serde(default)]
    auth: Option<AuthIntent>,
    operation: WriteOperation,
    client_settings: Option<settings::ClientSettingsIntent>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::commands) struct AuthIntent {
    pub(in crate::commands) before_hash: String,
    pub(in crate::commands) before_existed: bool,
    pub(in crate::commands) after_hash: String,
    pub(in crate::commands) after_existed: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CodexBackfillIntent {
    profile_id: String,
    before_hash: String,
    after_hash: String,
    before: asb_core::contracts::CodexProviderFile,
}

fn auth_snapshot_matches(
    config_target: &Path,
    expected_hash: &str,
    expected_existed: bool,
) -> Result<bool, String> {
    let target = config_target.with_file_name("auth.json");
    let (content, existed) = match FsIo.read_file(&target) {
        Ok(content) => (content, true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (String::new(), false),
        Err(error) => return Err(format!("无法读取 Codex 认证文件：{error}")),
    };
    Ok(existed == expected_existed && asb_switch::sha256_hex(&content) == expected_hash)
}

pub(in crate::commands) fn finish<T>(
    state: &LocalState,
    gateway: &GatewayController,
    result: Result<T, asb_switch::SwitchError>,
) -> Result<T, CommandError> {
    match result {
        Ok(value) => {
            clear(state).map_err(error)?;
            Ok(value)
        }
        Err(failure) => {
            if let Some(intent) = load(state).map_err(error)? {
                if asb_switch::pending_config_write(&FsIo, &state.backup_dir(), intent.app)
                    .map_err(|e| error(e.to_string()))?
                    .is_none()
                {
                    if intent.codex_backfill.is_some() || intent.client_settings.is_some() {
                        restore_application_snapshot(state, gateway, &intent, None)
                            .map_err(error)?;
                    } else {
                        restore_catalog(&intent).map_err(error)?;
                    }
                    clear(state).map_err(error)?;
                    return Err(CommandError::from(failure));
                }
            }
            let (current, before) = match load(state).map_err(error)? {
                Some(intent) => {
                    let (text, exists) =
                        read_target(Path::new(&intent.target), intent.app).map_err(error)?;
                    (
                        asb_switch::sha256_hex(&text),
                        (intent.before_hash, exists == intent.before_existed),
                    )
                }
                None => return Err(CommandError::from(failure)),
            };
            if current != before.0 || !before.1 {
                return Err(CommandError::localized(
                    "config-recovery-required",
                    "errors.sw.configCompensationIncomplete",
                    format!("{failure}；配置补偿未完成，保留事务和备份"),
                    serde_json::json!({ "failure": failure.to_string() }),
                ));
            }
            // An active profile save restores its profile before restoring the route snapshot.
            if state
                .configuration()
                .pending_profile_save()
                .map_err(|e| error(e.to_string()))?
                .is_some()
            {
                return Err(CommandError::from(failure));
            }
            if let Some(intent) = load(state).map_err(error)? {
                if intent.client_settings.is_none() {
                    recover(state, gateway).map_err(error)?;
                    return Err(CommandError::from(failure));
                }
                compensate_client_failure(state, gateway, &intent).map_err(error)?;
            }
            Err(CommandError::from(failure))
        }
    }
}

fn compensate_client_failure(state: &LocalState, gateway: &GatewayController, intent: &SwitchIntent) -> Result<(), String> {
    let pending = asb_switch::pending_config_write(&FsIo, &state.backup_dir(), intent.app)
        .map_err(|error| error.to_string())?.ok_or("执行器事务已消失，保留应用恢复记录")?;
    validate_pending(intent, &pending)?;
    if intent.auth.is_some() {
        if !auth_matches(intent, false)? && !auth_matches(intent, true)? {
            return Err("认证文件已发生额外变化，保留事务和备份".into());
        }
        rollback_pending_transaction(state, gateway, intent, &pending)?;
    } else {
        rollback_application(state, gateway, intent, Some(&pending))?;
    }
    clear(state)
}

pub(super) fn recover(state: &LocalState, gateway: &GatewayController) -> Result<(), String> {
    let Some(intent) = load(state)? else {
        return reject_orphan(state);
    };
    let target = state.target(intent.app)?;
    if intent.version != 2
        || Path::new(&intent.target) != target
        || intent.previous_route.app != intent.app
    {
        return Err("配置事务目标或版本不匹配，保留恢复记录".into());
    }
    validate_catalog_target(&target, intent.catalog.as_ref())?;
    let pending = asb_switch::pending_config_write(&FsIo, &state.backup_dir(), intent.app)
        .map_err(|e| e.to_string())?;
    if let Some(pending) = &pending {
        validate_pending(&intent, pending)?;
    }
    let (text, existed) = read_target(&target, intent.app)?;
    let hash = asb_switch::sha256_hex(&text);
    let auth_before = auth_matches(&intent, false)?;
    let auth_after = auth_matches(&intent, true)?;
    let unchanged_application = state
        .configuration()
        .latest_config_write(intent.app)
        .map_err(|e| e.to_string())?
        == intent.previous_write;
    if let Some(backfill) = &intent.codex_backfill {
        if !is_hex_revision(&backfill.before_hash, 16) {
            return Err("Codex 回填的原档案版本无效，保留恢复记录".into());
        }
        if !is_hex_revision(&backfill.after_hash, 16) {
            if !is_hex_revision(&backfill.after_hash, 64)
                || pending.is_some()
                || !unchanged_application
                || hash != intent.before_hash
                || !existed
                || existed != intent.before_existed
                || !auth_before
            {
                return Err("旧 Codex 回填已不符合安全回滚条件，保留恢复记录".into());
            }
            return recover_interrupted_backfill(state, gateway, &intent, backfill);
        }
    }
    if pending.is_none()
        && unchanged_application
        && hash == intent.before_hash
        && existed == intent.before_existed
        && auth_before
    {
        if !resume_active_save(state, gateway, &intent, None, &text)? {
            rollback_application(state, gateway, &intent, None)?;
        }
    } else if hash == intent.after_hash && existed == intent.after_existed && auth_after {
        complete(state, gateway, &intent, pending.as_ref(), &text)?;
    } else if hash == intent.before_hash && existed == intent.before_existed && auth_before {
        if !resume_active_save(state, gateway, &intent, pending.as_ref(), &text)? {
            rollback_application(state, gateway, &intent, pending.as_ref())?;
        }
    } else if pending.is_some()
        && snapshot_is_expected(
            hash == intent.before_hash,
            hash == intent.after_hash,
            auth_before,
            auth_after,
        )
    {
        rollback_pending_transaction(state, gateway, &intent, pending.as_ref().unwrap())?;
    } else {
        let backup = pending
            .as_ref()
            .map(|p| p.backup.id.as_str())
            .unwrap_or("尚未创建备份");
        return Err(format!(
            "配置与事务前后快照均不符；保留恢复记录，请在备份列表显式恢复事务备份 {backup}"
        ));
    }
    clear(state)
}

fn is_hex_revision(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

// The old comparison failed before staging the catalog or entering the
// executor. Consume only that interruption, without replaying the switch.
fn recover_interrupted_backfill(
    state: &LocalState,
    gateway: &GatewayController,
    intent: &SwitchIntent,
    backfill: &CodexBackfillIntent,
) -> Result<(), String> {
    let Some(route) = &intent.previous_route.route else {
        return Err("旧 Codex 回填缺少原网关路由，保留恢复记录".into());
    };
    if intent.app != AppKind::Codex
        || intent.operation != WriteOperation::Projection
        || intent.client_settings.is_some()
        || !intent.profile_id.as_ref().is_some_and(|id| {
            id != &backfill.profile_id && uuid::Uuid::parse_str(id).is_ok()
        })
        || backfill.before.profile.id != backfill.profile_id
        || route.profile_id != backfill.profile_id
        || crate::gateway::codex_route_fingerprint(&backfill.before)? != route.revision
        || state.configuration().pending_profile_save().map_err(|e| e.to_string())?.is_some()
    {
        return Err("旧 Codex 回填与切换记录不匹配，保留恢复记录".into());
    }
    reject_orphan(state)?;
    verify_catalog_before(intent)?;
    // On restart the gateway may already have discarded the old route,
    // whose stored profile no longer matched the route revision.
    let current = gateway.snapshot_activation(AppKind::Codex)?;
    if current.route.as_ref().is_some_and(|active| active != route) {
        return Err("Codex 网关路由已发生额外变化，保留恢复记录".into());
    }
    state.configuration().restore_interrupted_codex_backfill(
        &backfill.before, &backfill.before_hash, &backfill.after_hash,
    ).map_err(|e| e.to_string())?;
    gateway.restore_activation_snapshot(state, &intent.previous_route)?;
    clear(state)
}

fn rollback_pending_transaction(
    state: &LocalState,
    gateway: &GatewayController,
    intent: &SwitchIntent,
    pending: &PendingConfigWrite,
) -> Result<(), String> {
    asb_switch::rollback_pending_config(&FsIo, &state.backup_dir(), pending,
    |backup| settings::capture_current_backup(state, backup), || {
        restore_application_snapshot(state, gateway, intent, Some(pending))
    })
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn snapshot_is_expected(
    config_before: bool,
    config_after: bool,
    auth_before: bool,
    auth_after: bool,
) -> bool {
    (config_before || config_after)
        && (auth_before || auth_after)
        && !(config_before && auth_before)
        && !(config_after && auth_after)
}

fn resume_active_save(
    state: &LocalState,
    gateway: &GatewayController,
    intent: &SwitchIntent,
    pending: Option<&PendingConfigWrite>,
    _current: &str,
) -> Result<bool, String> {
    let Some(save) = state
        .configuration()
        .pending_profile_save()
        .map_err(|e| e.to_string())?
    else {
        return Ok(false);
    };
    if save.app != intent.app || Some(&save.projection_profile_id) != intent.profile_id.as_ref() {
        return Err("供应商保存与配置事务不匹配".into());
    }
    let revision = profile_revision(state, save.app, &save.profile_id)?;
    if revision == save.previous_file_hash {
        return Ok(false);
    }
    super::profile_rollback::validate_saved_revision(state, &save.profile_id, &revision)?;
    let projection_revision = profile_revision(state, save.app, &save.projection_profile_id)?;
    if Some(&projection_revision) != intent.profile_hash.as_ref() {
        return Err("待恢复供应商版本已发生额外变化".into());
    }
    let projected =
        super::plan::build_plan(state, gateway, &save.projection_profile_id).map_err(|e| e.message)?;
    let preview = super::plan::preview_projection(state, &projected).map_err(|e| e.message)?;
    if preview.rendered_hash != intent.after_hash {
        return Err("保存后的目标配置已变化，不能重新解释已确认事务".into());
    }
    finish_pending(
        state,
        pending,
        &intent.before_hash,
        intent.before_existed,
        || Ok(()),
    )?;
    Ok(true)
}

pub(super) fn profile_revision(
    state: &LocalState,
    app: AppKind,
    id: &str,
) -> Result<String, String> {
    profile_name_and_revision(state, app, id).map(|(_, revision)| revision)
}

fn profile_name_and_revision(
    state: &LocalState,
    app: AppKind,
    id: &str,
) -> Result<(String, String), String> {
    if app == AppKind::Codex {
        if let Some(record) = state
            .configuration()
            .list_codex_providers()
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|record| record.profile.id == id)
        {
            return Ok((record.profile.name, record.file_hash));
        }
    }
    let record = state
        .configuration()
        .find_provider_record(id)
        .map_err(|error| error.to_string())?;
    if record.profile.app != app {
        return Err("供应商不属于当前配置事务的客户端".into());
    }
    Ok((record.profile.name, record.file_hash))
}

fn finish_pending(
    state: &LocalState,
    pending: Option<&PendingConfigWrite>,
    hash: &str,
    existed: bool,
    commit: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match pending {
        Some(pending) => asb_switch::finish_config_recovery(
            &FsIo,
            &state.backup_dir(),
            pending,
            hash,
            existed,
            commit,
        )
        .map_err(|e| e.to_string()),
        None => commit(),
    }
}

fn validate_pending(intent: &SwitchIntent, pending: &PendingConfigWrite) -> Result<(), String> {
    if intent.codex_backfill.as_ref().is_some_and(|backfill| {
        !is_hex_revision(&backfill.before_hash, 16) || !is_hex_revision(&backfill.after_hash, 16)
    }) {
        return Err("Codex 回填记录与执行器事务不匹配，保留恢复记录".into());
    }
    if pending.backup.target_path != intent.target
        || pending.backup.content_hash != intent.before_hash
        || pending.backup.target_existed != intent.before_existed
        || pending.after_hash != intent.after_hash
        || pending.after_existed != intent.after_existed
        || pending.profile_id != intent.profile_id
    {
        return Err("应用事务与执行器事务不匹配，保留恢复记录".into());
    }
    match (&intent.auth, &pending.auth) {
        (None, None) => {}
        (Some(expected), Some(actual))
            if actual.backup.target_path
                == Path::new(&intent.target)
                    .with_file_name("auth.json")
                    .to_string_lossy()
                && actual.backup.content_hash == expected.before_hash
                && actual.backup.target_existed == expected.before_existed
                && actual.after_hash == expected.after_hash
                && actual.after_existed == expected.after_existed => {}
        _ => return Err("应用认证事务与执行器事务不匹配，保留恢复记录".into()),
    }
    Ok(())
}

fn auth_matches(intent: &SwitchIntent, after: bool) -> Result<bool, String> {
    let Some(auth) = intent.auth.as_ref() else {
        return Ok(true);
    };
    let expected_hash = if after {
        &auth.after_hash
    } else {
        &auth.before_hash
    };
    let expected_existed = if after {
        auth.after_existed
    } else {
        auth.before_existed
    };
    auth_snapshot_matches(Path::new(&intent.target), expected_hash, expected_existed)
}

pub(super) fn restore_pending_backup(
    state: &LocalState,
    gateway: &GatewayController,
    backup_id: &str,
) -> Result<Option<asb_switch::RestoreOutcome>, CommandError> {
    let Some(intent) = load(state).map_err(error)? else {
        return Ok(None);
    };
    let Some(pending) = asb_switch::pending_config_write(&FsIo, &state.backup_dir(), intent.app)
        .map_err(|e| error(e.to_string()))?
    else {
        return Ok(None);
    };
    if pending.backup.id != backup_id {
        return Ok(None);
    }
    validate_pending(&intent, &pending).map_err(error)?;
    if Path::new(&intent.target) != state.target(intent.app).map_err(error)? {
        return Err(CommandError::keyed(
            "config-recovery-required",
            "errors.sw.transactionTargetClientMismatch",
            "事务目标不属于当前客户端",
        ));
    }
    if let Some(save) = state
        .configuration()
        .pending_profile_save()
        .map_err(|e| error(e.to_string()))?
    {
        if save.app != intent.app || intent.profile_id.as_deref() != Some(save.projection_profile_id.as_str()) {
            return Err(CommandError::keyed(
                "config-recovery-required",
                "errors.sw.saveTransactionMismatch",
                "供应商保存与配置事务不匹配",
            ));
        }
        let revision = profile_revision(state, save.app, &save.profile_id).map_err(error)?;
        if revision != save.previous_file_hash {
            super::profile_rollback::validate_saved_revision(state, &save.profile_id, &revision)
                .map_err(error)?;
        }
        super::profile_rollback::restore(
            state,
            &save.profile_id,
            &revision,
        )
        .map_err(error)?;
    }
    let outcome = asb_switch::rollback_pending_config(&FsIo, &state.backup_dir(), &pending,
    |backup| settings::capture_current_backup(state, backup), || {
        restore_application_snapshot(state, gateway, &intent, Some(&pending))?;
        state.configuration().clear_profile_save()
    })
    .map_err(|e| error(e.to_string()))?;
    clear(state).map_err(error)?;
    super::profile_rollback::clear(state).map_err(error)?;
    Ok(Some(outcome))
}

fn reject_orphan(state: &LocalState) -> Result<(), String> {
    for app in [AppKind::Codex, AppKind::Claude] {
        if let Some(pending) = asb_switch::pending_config_write(&FsIo, &state.backup_dir(), app)
            .map_err(|e| e.to_string())?
        {
            return Err(format!("执行器事务缺少应用意图 {}；请退出应用，保留事务 {}、备份 {} 与当前配置 {}，人工核对后恢复配套意图文件；不要直接删除事务或覆盖配置", path(state).display(), asb_switch::config_journal_path(&state.backup_dir(), app).display(), pending.backup.backup_path, pending.backup.target_path));
        }
    }
    Ok(())
}

pub(in crate::commands) fn recovery_status(state: &LocalState) -> Option<String> {
    let intent = match load(state) {
        Ok(intent) => intent,
        Err(error) => return Some(error),
    };
    if let Some(intent) = intent {
        let client = match intent.app { AppKind::Codex => "Codex", AppKind::Claude => "Claude" };
        return Some(match asb_switch::pending_config_write(&FsIo, &state.backup_dir(), intent.app) {
            Ok(Some(pending)) => format!(
                "{client} 配置事务未完成。请在设置的备份恢复中选择备份 {}；恢复前保留当前文件和事务记录。",
                pending.backup.id
            ),
            Ok(None) => format!("{client} 配置事务未完成。请保留当前文件和事务记录，重试原操作以完成恢复；若仍失败，请查看日志中的具体原因。"),
            Err(error) => error.to_string(),
        });
    }
    reject_orphan(state).err()
}
