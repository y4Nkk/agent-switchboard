//! Backend-owned refresh cadence for both clients, independent of UI lifetime.

use crate::config_store::{providers::load_provider_files, StoreOperationError};
use crate::commands::{error::{operation_error, store_error, CommandError}, ConfigWriteGate};
use crate::local_state::LocalState;
use crate::{tray, usage_cache};
use asb_core::contracts::{AppKind, ProviderProfile, RouteMode};
use std::thread;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

const TICK: Duration = Duration::from_secs(60);

#[cfg(test)]
mod tests;

type QueryLocks = HashMap<(PathBuf, String), Weak<Mutex<()>>>;
static QUERY_LOCKS: OnceLock<Mutex<QueryLocks>> = OnceLock::new();

fn query_lock(state: &LocalState, id: &str) -> Result<Arc<Mutex<()>>, String> {
    let mut locks = QUERY_LOCKS.get_or_init(|| Mutex::new(HashMap::new())).lock()
        .map_err(|_| "用量查询锁不可用")?;
    locks.retain(|_, lock| lock.strong_count() > 0);
    let key = (state.root().to_path_buf(), id.to_string());
    if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
        return Ok(lock);
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(key, Arc::downgrade(&lock));
    Ok(lock)
}

pub(crate) fn spawn(app: AppHandle) {
    let spawned = thread::Builder::new()
        .name("usage-scheduler".into())
        .spawn(move || loop {
            let tick_started = Instant::now();
            run_tick(&app);
            if let Some(remaining) = TICK.checked_sub(tick_started.elapsed()) {
                thread::sleep(remaining);
            }
        });
    if let Err(error) = spawned {
        log::warn!("无法启动用量定时查询线程: {error}");
    }
}

fn run_tick(app: &AppHandle) {
    let state = match LocalState::from_app(app) {
        Ok(state) => state,
        Err(error) => {
            log::warn!("定时用量查询跳过一轮：{error}");
            return;
        }
    };
    let gate = app.state::<ConfigWriteGate>();
    let profiles = match profiles(&state, &gate) {
        Ok(profiles) => profiles,
        Err(error) => { log::warn!("定时用量查询无法读取配置：{error}"); return; }
    };
    thread::scope(|scope| {
        for profile in profiles {
            let state = &state;
            let gate = &*gate;
            scope.spawn(move || {
                let result = if profile.app == AppKind::Codex
                    && profile.route_mode == RouteMode::Official
                {
                    crate::commands::quota_refresh::refresh(state, &profile.id, false, gate)
                        .map_err(|error| error.message)
                } else {
                    let interval = profile.usage_query.as_ref()
                        .map(|query| query.refresh_interval_minutes()).unwrap_or(0);
                    if interval == 0 || !usage_cache::due(state, &profile, interval, chrono::Utc::now()) {
                        return;
                    }
                    execute_once(state, profile.app, &profile.id, false, gate).map_err(|error| error.message)
                };
                match result {
                    Ok(true) => tray::refresh(app),
                    Ok(false) => {},
                    Err(error) => {
                        log::warn!("供应商 {} 的定时用量查询失败：{}", profile.name,
                            asb_core::adapter::scrub_message(error));
                        tray::refresh(app);
                    }
                }
            });
        }
    });
}

/// Each store is independent: one client's invalid files cannot stop the other.
fn profiles(state: &LocalState, gate: &ConfigWriteGate) -> Result<Vec<ProviderProfile>, String> {
    let _guard = gate.lock()?;
    let store = state.configuration();
    let mut profiles = Vec::new();
    for app in [AppKind::Claude, AppKind::Codex] {
        match load_provider_files(&store, app) {
            Ok(files) => profiles.extend(files.into_iter().map(|file| file.into_profile(app))),
            Err(error) => { let error = store_error(error); log::warn!("{app:?} 定时用量查询无法读取供应商：{error}"); },
        }
    }
    match store.list_codex_providers() {
        Ok(records) => {
            for record in records {
                match store.find_codex_provider_file(&record.profile.id) {
                    Ok(file) => profiles.push(file.client_projection().into_profile(AppKind::Codex)),
                    Err(error) => { let error = store_error(error); log::warn!("定时用量查询无法读取 Codex 供应商：{error}"); },
                }
            }
        }
        Err(error) => { let error = store_error(error); log::warn!("定时用量查询无法读取 Codex 供应商列表：{error}"); },
    }
    Ok(profiles)
}

/// The cache orders completions by attempt start, including failed attempts.
/// Query responses never become a second display source in the renderer.
pub(crate) fn execute_once(state: &LocalState, app: AppKind, id: &str, force: bool,
    gate: &ConfigWriteGate) -> Result<bool, CommandError> {
    let failed = |message| CommandError::new("usage-query-failed", message);
    let lock = query_lock(state, id).map_err(failed)?;
    let _guard = lock.lock().map_err(|_| failed("用量查询锁不可用".to_string()))?;
    let profile = &{
        let _guard = gate.lock().map_err(failed)?;
        usage_profile(state, app, id).map_err(|error| operation_error("profile-not-found", error))?
    };
    let query = profile.usage_query.as_ref()
        .ok_or_else(|| failed("该供应商尚未配置用量查询".to_string()))?;
    let interval = query.refresh_interval_minutes();
    if !force && (interval == 0 || !usage_cache::due(state, profile, interval, chrono::Utc::now())) {
        return Ok(false);
    }
    let upstream_protocol = profile.upstream_protocol
        .ok_or_else(|| failed("供应商缺少上游 API 格式".to_string()))?;
    let attempted_at = chrono::Utc::now();
    let result = crate::usage_query::run_usage_query_with_connection(
        query, &profile.api_key, profile.base_url.as_deref(), upstream_protocol,
        profile.authentication, &profile.connection,
    );
    let _guard = gate.lock().map_err(failed)?;
    let current = usage_profile(state, profile.app, &profile.id)
        .map_err(|error| operation_error("profile-not-found", error))?;
    if current != *profile {
        return Err(failed("供应商在查询期间已修改，本次结果已丢弃，请重新刷新".into()));
    }
    match result {
        Ok(summary) => {
            if usage_cache::record_success(state, profile, summary.clone(), attempted_at).map_err(failed)? {
                if let Err(error) = crate::usage_history::record_provider(state, profile, &summary) {
                    log::warn!("用量查询成功，但无法保存趋势历史: {error}");
                }
            }
            Ok(true)
        }
        Err(error) => {
            if let Err(cache_error) = usage_cache::record_failure(state, profile, attempted_at, &error) {
                log::warn!("用量查询失败，且无法记录尝试时间: {cache_error}");
            }
            Err(failed(error))
        }
    }
}

/// Explicit client scope avoids reading the other client's store during a query.
pub(crate) fn usage_profile(
    state: &LocalState, app: AppKind, profile_id: &str,
) -> Result<ProviderProfile, StoreOperationError> {
    let store = state.configuration();
    if app == AppKind::Codex {
        return store.find_codex_provider_file(profile_id)
            .map(|file| file.client_projection().into_profile(app))
            .map_err(StoreOperationError::from);
    }
    load_provider_files(&store, app)?.into_iter()
        .find(|file| file.id == profile_id)
        .map(|file| file.into_profile(app))
        .ok_or_else(|| "供应商不存在".into())
}
