//! Read-only configuration status: per-client file health, route facts,
//! match classification against the profile store, and lock observation.

use super::error::{blocking, observe, state, store_error, CommandError};
use crate::runtime_log::RuntimeLogAction;
use asb_core::contracts::{
    AppKind, CommonConfigPatch, MatchStatus, ProviderProfile, RouteState, SwitchLog, SwitchOp,
    SwitchPlan,
};
use asb_core::{adapter, LockStatus};
use asb_switch::io::{FsIo, SwitchIo};
use asb_switch::lockfile::{self, RecoveryEntry};
use asb_switch::sha256_hex;
use serde::Serialize;
use std::io::ErrorKind;
use tauri::AppHandle;

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
    pub last_switch: Option<SwitchLog>,
}

fn classify_match_status(
    last: Option<&SwitchLog>,
    current_hash: &str,
    matching_profile: Option<&ProviderProfile>,
) -> MatchStatus {
    if let Some(record) = last {
        if record.content_hash == current_hash
            && record.profile_id.is_none()
            && record.operation == SwitchOp::Switch
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
        Some(record) if record.content_hash == current_hash => {
            if record.operation == SwitchOp::CommonSettings {
                MatchStatus::MatchesSettings {
                    at: record.at.clone(),
                }
            } else {
                MatchStatus::ProfileChanged {
                    profile_name: record
                        .profile_name
                        .clone()
                        .unwrap_or_else(|| "已删除档案".to_string()),
                }
            }
        }
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
    let store = state.load_store().map_err(store_error)?;
    let hash = sha256_hex(text);
    let last = store
        .switch_log
        .iter()
        .rev()
        .find(|entry| entry.app == kind);

    let common = store
        .common
        .iter()
        .find(|patch| patch.app == kind)
        .cloned()
        .unwrap_or(CommonConfigPatch {
            app: kind,
            entries: vec![],
        });
    let matching_profile = store
        .profiles
        .iter()
        .filter(|p| p.app == kind)
        .find(|profile| {
            let profile = *profile;
            let plan = SwitchPlan {
                app: kind,
                profile: profile.clone(),
                common: common.clone(),
            };
            let unchanged = asb_core::validate_plan(&plan.profile, &plan.common).is_ok()
                && matches!(
                    adapter::preview(text, &plan, ""),
                    Ok(preview) if preview.changes.is_empty()
                );
            unchanged
        });

    Ok(classify_match_status(last, &hash, matching_profile))
}

/// The logic owner behind the `config_status` command. Also consumed by the
/// tray, which rebuilds its menu labels outside the command pipeline.
pub(crate) fn config_status_report(
    state: &crate::local_state::LocalState,
) -> Result<Vec<ConfigFileStatus>, CommandError> {
    let io = FsIo;
    [AppKind::Codex, AppKind::Claude]
        .into_iter()
        .map(|kind| {
            let target = state
                .target(kind)
                .map_err(|error| CommandError::new("config-path-unavailable", error))?;
            let last_switch = state.latest_switch(kind).map_err(store_error)?;
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

    fn log(content_hash: &str, profile_id: Option<&str>, profile_name: Option<&str>) -> SwitchLog {
        SwitchLog {
            app: AppKind::Codex,
            profile_id: profile_id.map(str::to_string),
            profile_name: profile_name.map(str::to_string),
            content_hash: content_hash.to_string(),
            backup_id: "b1".to_string(),
            at: "2026-08-26T08:00:00Z".to_string(),
            operation: SwitchOp::Switch,
        }
    }

    #[test]
    fn match_status_never_activates_a_restored_backup_or_an_edited_profile() {
        let current = profile();
        let restored =
            classify_match_status(Some(&log("same", None, None)), "same", Some(&current));
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
}
