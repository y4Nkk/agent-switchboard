use super::discovery::{codex_import_source, discovery_report};
use super::error::{observe, operation_error, state, store_error, CommandError};
use super::switching;
use crate::local_state::LocalState;
use crate::runtime_log::RuntimeLogAction;
use asb_core::contracts::{AppKind, CodexProviderDraft, CodexProviderRecord, ProviderRecord};
use asb_core::discovery::CodexImportAction;
use std::collections::BTreeMap;
use tauri::Manager;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexImportResult {
    pub id: String,
    pub name: String,
    pub official: bool,
}

#[tauri::command]
pub async fn list_profiles(app: tauri::AppHandle) -> Result<Vec<ProviderRecord>, CommandError> {
    let state = state(&app)?;
    let gate = app.state::<super::ConfigWriteGate>().inner().clone();
    gate.run(move || state.configuration().list_providers().map_err(store_error)).await
}

/// Lists only current-format third-party Codex profiles. This dedicated
/// boundary prevents the renderer from treating a generic provider draft as
/// a Codex route definition.
#[tauri::command]
pub async fn list_codex_profiles(
    app: tauri::AppHandle,
) -> Result<Vec<CodexProviderRecord>, CommandError> {
    let state = state(&app)?;
    let gate = app.state::<super::ConfigWriteGate>().inner().clone();
    gate.run(move || {
        state
            .configuration()
            .list_codex_providers()
            .map_err(store_error)
    })
    .await
}

#[tauri::command]
pub async fn create_codex_profile(
    app: tauri::AppHandle,
    profile_id: String,
    draft: CodexProviderDraft,
) -> Result<CodexProviderRecord, CommandError> {
    let refresh_app = app.clone();
    let result = observe(RuntimeLogAction::ProfileCreated, async move {
        let state = state(&app)?;
        let gate = app.state::<super::ConfigWriteGate>().inner().clone();
        let gateway = app.state::<crate::gateway::GatewayController>().inner().clone();
        gate.run(move || {
            switching::ensure_profile_save_recovered(&app)?;
            let _targets = gateway.inner.profile_save_gate.write()
                .map_err(|_| CommandError::new("codex-profile-create-failed", "供应商保存锁不可用"))?;
            state
                .configuration()
                .create_codex_provider(profile_id, draft)
                .map_err(|error| operation_error("codex-profile-create-failed", error))
        })
        .await
    })
    .await;
    if result.is_ok() {
        crate::tray::refresh(&refresh_app);
    }
    result
}

/// A native Responses profile may be active without an in-memory gateway route.
/// The generated catalog reference still identifies its owning provider.
fn ensure_codex_profile_deletable(state: &LocalState, id: &str) -> Result<(), CommandError> {
    let (policy, _) = crate::gateway::codex::policy::load(state.root())
        .map_err(|error| CommandError::new("codex-policy-invalid", error))?;
    if policy.provider_ids.iter().any(|entry| entry == id) {
        return Err(CommandError::keyed(
            "codex-provider-in-failover-queue",
            "errors.sw.codexInFailoverQueue",
            "请先从 Codex 故障转移队列移除该供应商，再删除档案",
        ));
    }
    let referenced_by_subagent_route = state
        .configuration()
        .list_codex_providers()
        .map_err(store_error)?
        .into_iter()
        .any(|record| {
            record.profile.id != id
                && record
                    .profile
                    .subagent_route
                    .is_some_and(|route| route.profile_id == id)
        });
    if referenced_by_subagent_route {
        return Err(CommandError::keyed(
            "codex-provider-referenced-by-subagent-route",
            "errors.sw.codexReferencedBySubagentRoute",
            "该 Codex 供应商正被其他档案的子代理路由引用；请先移除引用后再删除档案",
        ));
    }
    let target = state
        .target(AppKind::Codex)
        .map_err(|error| CommandError::new("config-path-unavailable", error))?;
    let text = match std::fs::read_to_string(target) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => {
            return Err(CommandError::keyed(
                "codex-profile-delete-check-failed",
                "errors.sw.codexDeleteCheckUnreadable",
                "无法读取 Codex 配置以确认档案未被引用",
            ))
        }
    };
    let document = text.parse::<toml_edit::DocumentMut>().map_err(|_| {
        CommandError::keyed(
            "codex-profile-delete-check-failed",
            "errors.sw.codexDeleteCheckUnparseable",
            "Codex 配置格式无效，无法确认档案是否仍被引用",
        )
    })?;
    if document
        .get("model_catalog_json")
        .and_then(toml_edit::Item::as_str)
        .is_some_and(|pointer| codex_catalog_references_profile(pointer, id))
    {
        return Err(CommandError::keyed(
            "codex-profile-active",
            "errors.sw.codexStillReferenced",
            "该 Codex 供应商仍被当前客户端配置引用；请先切换后再删除",
        ));
    }
    Ok(())
}

fn codex_catalog_references_profile(pointer: &str, id: &str) -> bool {
    std::path::Path::new(pointer)
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| {
            name.starts_with(&format!("agent-switchboard-codex-{id}-")) && name.ends_with(".json")
        })
}

fn remove_claude_provider_from_failover_policy(
    state: &LocalState,
    provider_id: &str,
) -> Result<(), String> {
    if !crate::gateway::failover::path(state.root()).exists() {
        return Ok(());
    }
    let mut policy = crate::gateway::failover::load(state.root())?;
    policy.provider_ids.retain(|id| id != provider_id);
    crate::gateway::failover::save(state.root(), &policy)
}
#[tauri::command]
pub async fn delete_codex_profile(
    app: tauri::AppHandle,
    profile_id: String,
    expected_file_hash: String,
) -> Result<(), CommandError> {
    let refresh_app = app.clone();
    let cache_profile_id = profile_id.clone();
    let result = observe(RuntimeLogAction::ProfileDeleted, async move {
        let state = state(&app)?;
        let gateway = app
            .state::<crate::gateway::GatewayController>()
            .inner()
            .clone();
        let gate = app.state::<super::ConfigWriteGate>().inner().clone();
        gate.run(move || {
            switching::ensure_profile_save_recovered(&app)?;
            if gateway.uses_profile(&profile_id) {
                return Err(CommandError::keyed(
                    "gateway-profile-active",
                    "errors.sw.codexGatewayActiveDelete",
                    "该 Codex 供应商正在被本机协议网关使用；请先切换到官方登录后再删除",
                ));
            }
            ensure_codex_profile_deletable(&state, &profile_id)?;
            state
                .configuration()
                .delete_codex_provider(&profile_id, &expected_file_hash)
                .map_err(|error| operation_error("codex-profile-delete-failed", error))
        })
        .await
    })
    .await;
    if result.is_ok() {
        if let Ok(state) = LocalState::from_app(&refresh_app) {
            if let Err(error) = crate::usage_cache::invalidate(&state, &cache_profile_id) {
                log::warn!("无法清除已删除 Codex 供应商的托盘用量缓存: {error}");
            }

        }
        crate::tray::refresh(&refresh_app);
    }
    result
}

#[cfg(test)]
mod subagent_reference_tests {
    use super::ensure_codex_profile_deletable;

    fn draft_with_route(
        route: Option<asb_core::contracts::CodexSubagentRoute>,
    ) -> asb_core::contracts::CodexProviderDraft {
        let mut draft = crate::codex_common::test_draft();
        draft.subagent_route = route;
        draft
    }

    #[test]
    fn a_profile_referenced_by_a_subagent_route_is_not_deletable() {
        let _paths = crate::test_client_paths::redirect_client_paths();
        let directory = tempfile::tempdir().unwrap();
        let state = crate::local_state::LocalState::from_root(directory.path().join("state"));
        let target = state
            .configuration()
            .create_codex_provider(uuid::Uuid::new_v4().to_string(), draft_with_route(None))
            .unwrap();
        state
            .configuration()
            .create_codex_provider(uuid::Uuid::new_v4().to_string(), draft_with_route(Some(
                asb_core::contracts::CodexSubagentRoute {
                    profile_id: target.profile.id.clone(),
                    model: "relay-model".to_string(),
                },
            )))
            .unwrap();

        let error = ensure_codex_profile_deletable(&state, &target.profile.id).unwrap_err();
        assert_eq!(error.code, "codex-provider-referenced-by-subagent-route");

        let referencing = state
            .configuration()
            .list_codex_providers()
            .unwrap()
            .into_iter()
            .find(|record| record.profile.id != target.profile.id)
            .unwrap();
        assert!(ensure_codex_profile_deletable(&state, &referencing.profile.id).is_ok());
    }
}

#[tauri::command]
pub async fn reorder_codex_profiles(
    app: tauri::AppHandle,
    ordered_ids: Vec<String>,
    expected_file_hashes: BTreeMap<String, String>,
) -> Result<(), CommandError> {
    let refresh_app = app.clone();
    let result = observe(RuntimeLogAction::ProfilesReordered, async move {
        let state = state(&app)?;
        let gate = app.state::<super::ConfigWriteGate>().inner().clone();
        gate.run(move || {
            state
                .configuration()
                .reorder_codex_profiles(&ordered_ids, &expected_file_hashes)
                .map_err(|error| operation_error("codex-profile-reorder-failed", error))
        })
        .await
    })
    .await;
    if result.is_ok() {
        crate::tray::refresh(&refresh_app);
    }
    result
}

#[tauri::command]
pub async fn delete_profile(
    app: tauri::AppHandle,
    profile_id: String,
    expected_file_hash: String,
) -> Result<(), CommandError> {
    let refresh_app = app.clone();
    let cache_profile_id = profile_id.clone();
    let result = observe(RuntimeLogAction::ProfileDeleted, async move {
        let state = state(&app)?;
        let gateway = app
            .state::<crate::gateway::GatewayController>()
            .inner()
            .clone();
        let gate = app.state::<super::ConfigWriteGate>().inner().clone();
        gate.run(move || {
            switching::ensure_profile_save_recovered(&app)?;
            if gateway.uses_profile(&profile_id) {
                return Err(CommandError::keyed(
                    "gateway-profile-active",
                    "errors.sw.gatewayActiveDelete",
                    "该供应商正在被本机协议网关使用；请先切换到直连或官方登录后再删除",
                ));
            }
            let is_claude = state
                .configuration()
                .find_provider(&profile_id)
                .map(|profile| profile.app == AppKind::Claude)
                .unwrap_or(false);
            if !is_claude {
                crate::codex_auth::require_unbound(state.root(), &profile_id)
                    .map_err(|error| CommandError::new("codex-profile-account-bound", error))?;
            }
            let deleted = state
                .configuration()
                .delete_provider(&profile_id, &expected_file_hash)
                .map_err(|error| operation_error("profile-delete-failed", error));
            if deleted.is_ok() && is_claude {
                if let Err(error) =
                    remove_claude_provider_from_failover_policy(&state, &profile_id)
                {
                    log::warn!(
                        "无法从 Claude 故障转移队列清理已删除供应商 {}: {}",
                        profile_id,
                        error
                    );
                }
                if let Err(error) = gateway.refresh_claude_candidates(&state) {
                    log::warn!("无法刷新 Claude 故障转移候选路由: {error}");
                }
            }
            deleted
        })
        .await
    })
    .await;
    if result.is_ok() {
        if let Ok(state) = LocalState::from_app(&refresh_app) {
            if let Err(error) = crate::usage_cache::invalidate(&state, &cache_profile_id) {
                log::warn!("无法清除已删除供应商的托盘用量缓存: {error}");
            }

        }
        crate::codex_official_quota::invalidate(&cache_profile_id);
        crate::tray::refresh(&refresh_app);
    }
    result
}

#[tauri::command]
pub async fn reorder_claude_profiles(
    app: tauri::AppHandle,
    ordered_ids: Vec<String>,
    expected_file_hashes: BTreeMap<String, String>,
) -> Result<(), CommandError> {
    let refresh_app = app.clone();
    let result = observe(RuntimeLogAction::ProfilesReordered, async move {
        let state = state(&app)?;
        let gate = app.state::<super::ConfigWriteGate>().inner().clone();
        gate.run(move || {
            switching::ensure_profile_save_recovered(&app)?;
            state
                .configuration()
                .reorder_claude_providers(&ordered_ids, &expected_file_hashes)
                .map_err(|error| operation_error("claude-profile-reorder-failed", error))
        })
        .await
    })
    .await;
    if result.is_ok() {
        crate::tray::refresh(&refresh_app);
    }
    result
}

#[tauri::command]
pub async fn import_discovered_claude_profile(
    app: tauri::AppHandle,
) -> Result<ProviderRecord, CommandError> {
    let refresh_app = app.clone();
    let result = observe(RuntimeLogAction::ProfileImported, async move {
        let state = state(&app)?;
        let gateway = app
            .state::<crate::gateway::GatewayController>()
            .inner()
            .clone();
        let gate = app.state::<super::ConfigWriteGate>().inner().clone();
        gate.run(move || {
            switching::ensure_profile_save_recovered(&app)?;
            if gateway.has_active_route_for(AppKind::Claude) {
                return Err(CommandError::keyed(
                    "gateway-route-active",
                    "errors.sw.gatewayActiveImport",
                    "该客户端正在使用本机协议网关；请先切换到直连或官方登录后再导入供应商",
                ));
            }
            let proposal = discovery_report()?
                .claude_import_proposals
                .into_iter()
                .next()
                .ok_or_else(|| {
                    CommandError::keyed(
                        "import-unavailable",
                        "errors.sw.noImportableProvider",
                        "当前配置没有可导入的供应商",
                    )
                })?;
            state
                .configuration()
                .import_provider(proposal.draft)
                .map_err(|error| operation_error("profile-import-failed", error))
        })
        .await
    })
    .await;
    if result.is_ok() {
        // The imported configuration no longer matches the scanned snapshot.
        if let Ok(state) = LocalState::from_app(&refresh_app) {
            if let Err(error) = state.clear_discovery_cache() {
                log::warn!("无法清除导入后的发现扫描缓存: {error}");
            }
        }
        crate::tray::refresh(&refresh_app);
    }
    result
}

/// Imports the current local Codex route. OAuth tokens stay in auth.json and
/// third-party API keys are persisted only in the specialized Codex store.
#[tauri::command]
pub async fn import_discovered_codex_profile(
    app: tauri::AppHandle,
) -> Result<CodexImportResult, CommandError> {
    let refresh_app = app.clone();
    let result = observe(RuntimeLogAction::ProfileImported, async move {
        let state = state(&app)?;
        let gateway = app
            .state::<crate::gateway::GatewayController>()
            .inner()
            .clone();
        let gate = app.state::<super::ConfigWriteGate>().inner().clone();
        gate.run(move || {
            switching::ensure_profile_save_recovered(&app)?;
            if gateway.has_active_route_for(AppKind::Codex) {
                return Err(CommandError::keyed(
                    "gateway-route-active",
                    "errors.sw.codexGatewayActiveImport",
                    "Codex 当前正在使用本机协议网关；请先切换到官方登录或直连后再导入供应商",
                ));
            }
            match codex_import_source()?.action {
                CodexImportAction::Official => {
                    let (record, _) = state
                        .configuration()
                        .ensure_codex_official_record()
                        .map_err(|error| operation_error("profile-import-failed", error))?;
                    Ok(CodexImportResult {
                        id: record.profile.id,
                        name: record.profile.name,
                        official: true,
                    })
                }
                CodexImportAction::ThirdParty(draft) => {
                    let record = state
                        .configuration()
                        .create_codex_provider(uuid::Uuid::new_v4().to_string(), draft)
                        .map_err(|error| operation_error("codex-profile-import-failed", error))?;
                    Ok(CodexImportResult {
                        id: record.profile.id,
                        name: record.profile.name,
                        official: false,
                    })
                }
            }
        })
        .await
    })
    .await;
    if result.is_ok() {
        if let Ok(state) = LocalState::from_app(&refresh_app) {
            if let Err(error) = state.clear_discovery_cache() {
                log::warn!("无法清除导入后的发现扫描缓存: {error}");
            }
        }
        crate::tray::refresh(&refresh_app);
    }
    result
}
