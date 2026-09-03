//! Tauri commands driving the official login flows.
//!
//! The renderer receives only status, device codes, and URLs. Tokens exist
//! inside a single poll call between the vendor exchange and the native
//! credential write, and are never part of a response, log entry, or error.

use super::error::{blocking, CommandError};
use crate::local_state::LocalState;
use crate::official_login::{
    self, claude, codex, credentials, LoginSession, OfficialLoginPhase, OfficialLoginStart,
    OfficialLoginStatus,
};
use crate::runtime_log::RuntimeLogAction;
use asb_core::contracts::AppKind;
use std::time::Instant;

/// Starts one official login per client. A live login for the same client is
/// rejected; a leftover past its expiry window — whose listener already ended
/// by itself — is replaced.
#[tauri::command]
pub async fn official_login_start(target: AppKind) -> Result<OfficialLoginStart, CommandError> {
    blocking(move || {
        let mut guard = official_login::sessions().lock().expect("login sessions");
        if let Some(previous) = guard.remove(&target) {
            if !previous.expired(Instant::now()) {
                guard.insert(target, previous);
                return Err(CommandError::new(
                    "official-login-in-progress",
                    "已有进行中的官方登录，请先完成或取消",
                ));
            }
        }
        match target {
            AppKind::Codex => {
                let device = codex::request_device_code(&codex::CodexOAuthEndpoints::default())
                    .map_err(|error| CommandError::new("official-login-start-failed", error))?;
                let start = OfficialLoginStart {
                    user_code: Some(device.user_code.clone()),
                    verification_url: codex::DEVICE_VERIFICATION_URL.to_string(),
                };
                guard.insert(
                    target,
                    LoginSession::Codex {
                        device_auth_id: device.device_auth_id,
                        user_code: device.user_code,
                        started_at: Instant::now(),
                    },
                );
                Ok(start)
            }
            AppKind::Claude => {
                let listener = claude::begin(&claude::ClaudeOAuthEndpoints::default())
                    .map_err(|error| CommandError::new("official-login-start-failed", error))?;
                let start = OfficialLoginStart {
                    user_code: None,
                    verification_url: listener.authorize_url.clone(),
                };
                guard.insert(
                    target,
                    LoginSession::Claude {
                        listener,
                        started_at: Instant::now(),
                    },
                );
                Ok(start)
            }
        }
    })
    .await
}

/// Advances one login by a single step: a pending device-code poll, a browser
/// callback check, or — when the vendor approves — the token exchange and the
/// native credential write. Terminal results drop the session so a retry
/// always starts clean.
#[tauri::command]
pub async fn official_login_poll(target: AppKind) -> Result<OfficialLoginStatus, CommandError> {
    let status = blocking(move || {
        let mut guard = official_login::sessions().lock().expect("login sessions");
        let Some(session) = guard.get(&target) else {
            return Err(CommandError::new(
                "official-login-not-started",
                "尚未开始官方登录",
            ));
        };
        if session.expired(Instant::now()) {
            guard.remove(&target);
            return Ok(OfficialLoginStatus::failed(
                "登录已超时，请重新开始".to_string(),
            ));
        }
        match target {
            AppKind::Codex => poll_codex(&mut guard, target),
            AppKind::Claude => poll_claude(&mut guard, target),
        }
    })
    .await?;
    match status.phase {
        OfficialLoginPhase::Completed => {
            crate::runtime_log::record_success(RuntimeLogAction::OfficialLoginCompleted)
        }
        OfficialLoginPhase::Failed => crate::runtime_log::record_failure(
            RuntimeLogAction::OfficialLoginCompleted,
            "official-login-failed",
        ),
        OfficialLoginPhase::Pending => {}
    }
    Ok(status)
}

fn poll_codex(
    guard: &mut std::collections::BTreeMap<AppKind, LoginSession>,
    target: AppKind,
) -> Result<OfficialLoginStatus, CommandError> {
    let Some(LoginSession::Codex {
        device_auth_id,
        user_code,
        ..
    }) = guard.get(&target)
    else {
        return Err(CommandError::new(
            "official-login-not-started",
            "尚未开始官方登录",
        ));
    };
    let device_auth_id = device_auth_id.clone();
    let user_code = user_code.clone();

    match codex::poll_login(
        &codex::CodexOAuthEndpoints::default(),
        &device_auth_id,
        &user_code,
    ) {
        Ok(codex::CodexPollOutcome::Pending) => Ok(OfficialLoginStatus::pending(
            Some(user_code),
            codex::DEVICE_VERIFICATION_URL.to_string(),
        )),
        Ok(codex::CodexPollOutcome::Completed { tokens, account_id }) => {
            guard.remove(&target);
            let outcome = LocalState::codex_auth_path().and_then(|path| {
                credentials::write_codex_auth(&path, &tokens, account_id.as_deref())
            });
            Ok(match outcome {
                Ok(()) => OfficialLoginStatus::completed(),
                Err(message) => OfficialLoginStatus::failed(message),
            })
        }
        Err(message) => {
            guard.remove(&target);
            Ok(OfficialLoginStatus::failed(message))
        }
    }
}

fn poll_claude(
    guard: &mut std::collections::BTreeMap<AppKind, LoginSession>,
    target: AppKind,
) -> Result<OfficialLoginStatus, CommandError> {
    let Some(LoginSession::Claude { listener, .. }) = guard.get(&target) else {
        return Err(CommandError::new(
            "official-login-not-started",
            "尚未开始官方登录",
        ));
    };
    match claude::consume_callback(&listener.callback, &listener.state) {
        Ok(None) => Ok(OfficialLoginStatus::pending(
            None,
            listener.authorize_url.clone(),
        )),
        Ok(Some(code)) => {
            let outcome = claude::exchange_code(
                &claude::ClaudeOAuthEndpoints::default(),
                &code,
                &listener.verifier,
                &listener.redirect_uri,
            )
            .and_then(|tokens| {
                LocalState::claude_credentials_path()
                    .and_then(|path| credentials::write_claude_credentials(&path, &tokens))
            });
            // The worker ended its listener when it stored the callback, so
            // dropping the session is all the teardown needed.
            guard.remove(&target);
            Ok(match outcome {
                Ok(()) => OfficialLoginStatus::completed(),
                Err(message) => OfficialLoginStatus::failed(message),
            })
        }
        Err(message) => {
            guard.remove(&target);
            Ok(OfficialLoginStatus::failed(message))
        }
    }
}

/// Cancels one in-flight login. Idempotent: without a session this is a no-op.
#[tauri::command]
pub async fn official_login_cancel(target: AppKind) -> Result<(), CommandError> {
    blocking(move || {
        if let Some(session) = official_login::take_session(target) {
            session.stop_listener();
        }
        Ok(())
    })
    .await
}
