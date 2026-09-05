//! Read-only configuration status: per-client file health, route facts,
//! match classification against the profile store, and lock observation.

use super::error::{blocking, observe, state, store_error, CommandError};
use crate::runtime_log::RuntimeLogAction;
use asb_core::contracts::{
    AppKind, ConfigWriteRecord, MatchStatus, ProviderProfile, RouteState, SwitchPlan,
    WriteOperation,
};
use asb_core::{adapter, LockStatus};
use asb_switch::io::{FsIo, SwitchIo};
use asb_switch::lockfile::{self, RecoveryEntry};
use asb_switch::sha256_hex;
use serde::Serialize;
use std::io::ErrorKind;
use std::path::Path;
use tauri::{AppHandle, Manager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFileStatus {
    pub app: AppKind,
    pub path: String,
    pub exists: bool,
    pub syntax_ok: bool,
    pub route: Option<RouteState>,
    pub read_error: Option<String>,
    pub match_status: MatchStatus,
    pub active_profile_id: Option<String>,
    pub last_switch: Option<ConfigWriteRecord>,
}

/// Stable, non-secret facts about the running Agent Switchboard process.
/// This is deliberately separate from client configuration status: it owns
/// only application metadata and never reads Codex or Claude Code files.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOverview {
    pub app_version: String,
    pub build_mode: RuntimeBuildMode,
    pub platform: String,
    pub architecture: String,
    pub transport: RuntimeTransport,
    pub app_data_path: String,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeBuildMode {
    Debug,
    Release,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RuntimeTransport {
    DesktopProtocol,
    WebDevelopment {
        host: String,
        port: u16,
        #[serde(rename = "healthStatus")]
        health_status: u16,
    },
}

fn runtime_transport() -> RuntimeTransport {
    #[cfg(debug_assertions)]
    {
        let web_development = std::env::var_os("ASB_WEB_DEVELOPMENT");
        if crate::web_development_enabled(web_development.as_deref()) {
            return RuntimeTransport::WebDevelopment {
                host: crate::dev_api::DEV_API_HOST.to_string(),
                port: crate::dev_api::DEV_API_PORT,
                health_status: crate::dev_api::DEV_API_HEALTH_STATUS,
            };
        }
    }

    RuntimeTransport::DesktopProtocol
}

fn runtime_overview_for(
    app_version: String,
    app_data_dir: &Path,
    transport: RuntimeTransport,
) -> RuntimeOverview {
    RuntimeOverview {
        app_version,
        build_mode: if cfg!(debug_assertions) {
            RuntimeBuildMode::Debug
        } else {
            RuntimeBuildMode::Release
        },
        platform: std::env::consts::OS.to_string(),
        architecture: std::env::consts::ARCH.to_string(),
        transport,
        app_data_path: app_data_dir.to_string_lossy().into_owned(),
    }
}

fn classify_match_status(
    last: Option<&ConfigWriteRecord>,
    current_hash: &str,
    matching_profile: Option<&ProviderProfile>,
) -> MatchStatus {
    if let Some(record) = last {
        if record.content_hash == current_hash
            && record.profile_id.is_none()
            && record.operation == WriteOperation::Restore
        {
            return MatchStatus::RestoredBackup {
                at: record.at.clone(),
            };
        }
    }
    if let Some(profile) = matching_profile {
        return MatchStatus::MatchesProfile {
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
        };
    }

    match last {
        Some(record) if record.content_hash == current_hash => MatchStatus::ProfileChanged {
            profile_name: record
                .profile_name
                .clone()
                .unwrap_or_else(|| "通用设置".to_string()),
        },
        Some(record) => MatchStatus::ExternallyModified {
            at: record.at.clone(),
        },
        None => MatchStatus::Unmanaged,
    }
}

/// Decides whether the live content still matches a current profile, a
/// restored backup, or the app's last record. Requires the file to be readable
/// and syntactically valid.
fn match_status_for(
    state: &crate::local_state::LocalState,
    kind: AppKind,
    text: &str,
) -> Result<MatchStatus, CommandError> {
    let configuration = state.configuration();
    let hash = sha256_hex(text);
    let last = configuration
        .latest_config_write(kind)
        .map_err(store_error)?;

    let common = configuration
        .get_common_settings(kind)
        .map_err(store_error)?
        .settings;
    let matching_profile = configuration
        .list_providers()
        .map_err(store_error)?
        .into_iter()
        .filter(|record| record.profile.app == kind)
        .find(|record| {
            let plan = SwitchPlan {
                profile: record.profile.clone(),
                common: common.clone(),
            };
            let unchanged = asb_core::validate_plan(&plan.profile, &plan.common).is_ok()
                && matches!(
                    adapter::preview(text, &plan, ""),
                    Ok(preview) if preview.changes.is_empty()
                );
            unchanged
        })
        .map(|record| record.profile);

    Ok(classify_match_status(
        last.as_ref(),
        &hash,
        matching_profile.as_ref(),
    ))
}

fn active_profile_id(
    profiles: &[ProviderProfile],
    kind: AppKind,
    text: &str,
    codex_auth: Option<&str>,
    last: Option<&ConfigWriteRecord>,
) -> Result<Option<String>, CommandError> {
    let mut candidates = Vec::new();
    for profile in profiles.iter().filter(|profile| profile.app == kind) {
        if adapter::matches_provider_identity(text, codex_auth, profile).map_err(|error| {
            CommandError::new("provider-identity-unavailable", error.to_string())
        })? {
            candidates.push(profile.id.clone());
        }
    }
    if candidates.len() == 1 {
        return Ok(candidates.pop());
    }
    // Identical connection profiles cannot be distinguished by model or list order.
    Ok(last
        .and_then(|record| record.profile_id.as_ref())
        .filter(|id| candidates.contains(id))
        .cloned())
}

/// The logic owner behind the `config_status` command. Also consumed by the
/// tray, which rebuilds its menu labels outside the command pipeline.
pub(crate) fn config_status_report(
    state: &crate::local_state::LocalState,
) -> Result<Vec<ConfigFileStatus>, CommandError> {
    let io = FsIo;
    let profiles = state
        .configuration()
        .list_providers()
        .map_err(store_error)?
        .into_iter()
        .map(|record| record.profile)
        .collect::<Vec<_>>();
    let auth_path = crate::local_state::LocalState::codex_auth_path()
        .map_err(|error| CommandError::new("config-path-unavailable", error))?;
    let codex_auth = match io.read_file(&auth_path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(_) => {
            return Err(CommandError::new(
                "provider-identity-unavailable",
                "无法读取 Codex 登录缓存",
            ))
        }
    };
    [AppKind::Codex, AppKind::Claude]
        .into_iter()
        .map(|kind| {
            let target = state
                .target(kind)
                .map_err(|error| CommandError::new("config-path-unavailable", error))?;
            let last_switch = state
                .configuration()
                .latest_config_write(kind)
                .map_err(store_error)?;
            let status = match io.read_file(&target) {
                Ok(text) => match adapter::validate_syntax(kind, &text) {
                    Ok(()) => ConfigFileStatus {
                        app: kind,
                        path: target.to_string_lossy().to_string(),
                        exists: true,
                        syntax_ok: true,
                        route: Some(adapter::route_state(kind, &text)),
                        read_error: None,
                        match_status: match_status_for(state, kind, &text)?,
                        active_profile_id: active_profile_id(
                            &profiles,
                            kind,
                            &text,
                            codex_auth.as_deref(),
                            last_switch.as_ref(),
                        )?,
                        last_switch,
                    },
                    Err(_) => ConfigFileStatus {
                        app: kind,
                        path: target.to_string_lossy().to_string(),
                        exists: true,
                        syntax_ok: false,
                        route: None,
                        read_error: None,
                        match_status: MatchStatus::Unknown,
                        active_profile_id: None,
                        last_switch,
                    },
                },
                Err(error) if error.kind() == ErrorKind::NotFound => ConfigFileStatus {
                    app: kind,
                    path: target.to_string_lossy().to_string(),
                    exists: false,
                    syntax_ok: false,
                    route: None,
                    read_error: None,
                    match_status: MatchStatus::Unknown,
                    active_profile_id: None,
                    last_switch,
                },
                Err(_) => ConfigFileStatus {
                    app: kind,
                    path: target.to_string_lossy().to_string(),
                    exists: true,
                    syntax_ok: false,
                    route: None,
                    read_error: Some("无法读取配置文件".to_string()),
                    match_status: MatchStatus::Unknown,
                    active_profile_id: None,
                    last_switch,
                },
            };
            Ok(status)
        })
        .collect()
}

#[tauri::command]
pub async fn config_status(app: AppHandle) -> Result<Vec<ConfigFileStatus>, CommandError> {
    let state = state(&app)?;
    blocking(move || config_status_report(&state)).await
}

#[tauri::command]
pub async fn runtime_overview(app: AppHandle) -> Result<RuntimeOverview, CommandError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|_| CommandError::new("runtime-path-unavailable", "无法定位应用数据目录"))?;
    Ok(runtime_overview_for(
        app.package_info().version.to_string(),
        &app_data_dir,
        runtime_transport(),
    ))
}

#[tauri::command]
pub async fn lock_status(app: AppHandle, target: AppKind) -> Result<LockStatus, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        let target = state
            .target(target)
            .map_err(|error| CommandError::new("config-path-unavailable", error))?;
        Ok(lockfile::probe_lock(&FsIo, &target))
    })
    .await
}

#[tauri::command]
pub async fn recover_stale_lock(
    app: AppHandle,
    target: AppKind,
) -> Result<RecoveryEntry, CommandError> {
    observe(RuntimeLogAction::StaleLockRecovered, async move {
        let state = state(&app)?;
        blocking(move || {
            let target = state
                .target(target)
                .map_err(|error| CommandError::new("config-path-unavailable", error))?;
            lockfile::recover_stale(&FsIo, &target)
                .map_err(|_| CommandError::new("lock-not-stale", "当前锁不是可恢复的遗留状态"))
        })
        .await
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> ProviderProfile {
        ProviderProfile {
            id: "p1".to_string(),
            app: AppKind::Codex,
            route_mode: asb_core::RouteMode::Custom,
            name: "当前档案".to_string(),
            model: Some("gpt-5.4".to_string()),
            base_url: Some("https://gateway.example/v1".to_string()),
            api_key: "test-api-key".into(),
            model_options: None,
            notes: None,
            website_url: None,
            usage_query: None,
        }
    }

    fn log(
        content_hash: &str,
        profile_id: Option<&str>,
        profile_name: Option<&str>,
    ) -> ConfigWriteRecord {
        ConfigWriteRecord {
            app: AppKind::Codex,
            profile_id: profile_id.map(str::to_string),
            profile_name: profile_name.map(str::to_string),
            content_hash: content_hash.to_string(),
            backup_id: "b1".to_string(),
            at: "2026-08-26T08:00:00Z".to_string(),
            operation: WriteOperation::Projection,
        }
    }

    #[test]
    fn match_status_never_activates_a_restored_backup_or_an_edited_profile() {
        let current = profile();
        let mut restore_record = log("same", None, None);
        restore_record.operation = WriteOperation::Restore;
        let restored = classify_match_status(Some(&restore_record), "same", Some(&current));
        assert!(matches!(restored, MatchStatus::RestoredBackup { .. }));

        let stale_profile =
            classify_match_status(Some(&log("same", Some("p1"), Some("旧档案"))), "same", None);
        assert_eq!(
            stale_profile,
            MatchStatus::ProfileChanged {
                profile_name: "旧档案".to_string(),
            }
        );

        let matching = classify_match_status(
            Some(&log("old", Some("p1"), Some("旧档案"))),
            "current",
            Some(&current),
        );
        assert_eq!(
            matching,
            MatchStatus::MatchesProfile {
                profile_id: "p1".to_string(),
                profile_name: "当前档案".to_string(),
            }
        );
    }

    #[test]
    fn active_identity_is_independent_of_config_match_and_restores() {
        for app in [AppKind::Codex, AppKind::Claude] {
            let mut official = profile();
            official.app = app;
            official.route_mode = asb_core::RouteMode::Official;
            official.base_url = None;
            official.api_key.clear();
            let text = match app {
                AppKind::Codex => {
                    "model = \"different-model\"\nmodel_reasoning_effort = \"high\"\n"
                }
                AppKind::Claude => r#"{"model":"different-model","effortLevel":"high"}"#,
            };
            let mut restored = log("same", None, None);
            restored.operation = WriteOperation::Restore;
            assert_eq!(
                active_profile_id(&[official], app, text, None, Some(&restored)).unwrap(),
                Some("p1".into())
            );
        }
    }

    #[test]
    fn identical_connections_require_a_unique_match_or_matching_history() {
        let first = profile();
        let mut second = first.clone();
        second.id = "p2".into();
        second.model = Some("another-model".into());
        let profiles = [first, second];
        let text = "openai_base_url = \"https://gateway.example/v1\"\n";
        let auth = Some(r#"{"auth_mode":"apikey","OPENAI_API_KEY":"test-api-key"}"#);
        assert_eq!(
            active_profile_id(&profiles, AppKind::Codex, text, auth, None).unwrap(),
            None
        );
        assert_eq!(
            active_profile_id(
                &profiles,
                AppKind::Codex,
                text,
                auth,
                Some(&log("old", Some("p2"), None))
            )
            .unwrap(),
            Some("p2".into())
        );
        assert_eq!(
            active_profile_id(
                &profiles,
                AppKind::Codex,
                text,
                auth,
                Some(&log("old", Some("removed"), None))
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn runtime_overview_contains_only_application_runtime_facts() {
        let overview = runtime_overview_for(
            "0.1.5".to_string(),
            Path::new("C:/data/Agent Switchboard"),
            RuntimeTransport::WebDevelopment {
                host: "127.0.0.1".to_string(),
                port: 1422,
                health_status: 204,
            },
        );

        assert_eq!(overview.app_version, "0.1.5");
        assert_eq!(overview.app_data_path, "C:/data/Agent Switchboard");
        assert!(matches!(
            overview.build_mode,
            RuntimeBuildMode::Debug | RuntimeBuildMode::Release
        ));
        assert!(!overview.platform.is_empty());
        assert!(!overview.architecture.is_empty());
        assert_eq!(
            overview.transport,
            RuntimeTransport::WebDevelopment {
                host: "127.0.0.1".to_string(),
                port: 1422,
                health_status: 204,
            }
        );
        assert_eq!(
            serde_json::to_value(&overview.transport).expect("runtime transport serializes"),
            serde_json::json!({
                "kind": "webDevelopment",
                "host": "127.0.0.1",
                "port": 1422,
                "healthStatus": 204,
            })
        );
    }
}
