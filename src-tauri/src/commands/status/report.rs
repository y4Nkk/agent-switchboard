use super::ConfigFileStatus;
use crate::commands::error::{store_error, CommandError};
use asb_core::adapter;
use asb_core::contracts::{AppKind, ConfigWriteRecord, ProviderProfile, SwitchPlan};
use asb_switch::io::{FsIo, SwitchIo};
use std::io::ErrorKind;

pub(super) fn active_profile_id(
    profiles: &[ProviderProfile],
    gateway: Option<&crate::gateway::GatewayController>,
    kind: AppKind,
    text: &str,
    last: Option<&ConfigWriteRecord>,
) -> Result<Option<String>, CommandError> {
    if let Some(profile_id) = gateway
        .map(|gateway| gateway.active_profile_id(kind, text))
        .transpose()
        .map_err(|error| CommandError::new("provider-identity-unavailable", error))?
        .flatten()
    {
        return Ok(Some(profile_id));
    }
    let mut candidates = Vec::new();
    for profile in profiles
        .iter()
        .filter(|profile| profile.app == kind && !profile.requires_gateway())
    {
        let plan = SwitchPlan::direct(
            profile.clone(),
            asb_core::ownership::default_client_settings(kind),
        );
        if adapter::matches_provider_identity(text, &plan).map_err(|error| {
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
    gateway: &crate::gateway::GatewayController,
) -> Result<Vec<ConfigFileStatus>, CommandError> {
    let profiles = state
        .configuration()
        .list_providers()
        .map_err(store_error)?
        .into_iter()
        .map(|record| record.profile)
        .collect::<Vec<_>>();
    let mut applicable = Vec::new();
    for profile in profiles {
        if profile.app != AppKind::Codex
            || profile.route_mode != asb_core::RouteMode::Official
            || super::codex::account_matches(state, &profile.id)?
        {
            applicable.push(profile);
        }
    }
    let profiles = applicable;
    let recovery_issue = crate::commands::switching::transaction::recovery_status(state);
    [AppKind::Codex, AppKind::Claude]
        .into_iter()
        .map(|kind| observe_client_file(state, gateway, &profiles, kind, recovery_issue.as_deref()))
        .collect()
}

fn observe_client_file(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    profiles: &[ProviderProfile],
    kind: AppKind,
    recovery_issue: Option<&str>,
) -> Result<ConfigFileStatus, CommandError> {
    let target = state
        .target(kind)
        .map_err(|error| CommandError::new("config-path-unavailable", error))?;
    let mut status = ConfigFileStatus {
        app: kind,
        path: target.to_string_lossy().to_string(),
        exists: false,
        syntax_ok: false,
        route: None,
        gateway_port: None,
        read_error: None,
        recovery_issue: recovery_issue.map(str::to_owned),
        client_settings: None,
        client_settings_error: None,
        codex_permission_profile: None,
        active_profile_id: None,
        last_switch: state
            .configuration()
            .latest_config_write(kind)
            .map_err(store_error)?,
    };
    let text = match FsIo.read_file(&target) {
        Ok(text) => {
            status.exists = true;
            text
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            status.client_settings = Some(asb_core::ownership::default_client_settings(kind));
            return Ok(status);
        }
        Err(_) => {
            status.exists = true;
            status.read_error = Some("无法读取配置文件".to_string());
            status.client_settings_error = Some("无法读取配置文件".to_string());
            return Ok(status);
        }
    };
    if adapter::validate_syntax(kind, &text).is_err() {
        status.client_settings_error = Some("配置文件格式无效".to_string());
        return Ok(status);
    }
    status.syntax_ok = true;
    if kind == AppKind::Codex {
        status.codex_permission_profile = adapter::codex::permission_profile_status(&text).ok();
    }
    match adapter::extract_client_settings(kind, &text) {
        Ok(settings) => status.client_settings = Some(settings),
        Err(error) => status.client_settings_error = Some(error.to_string()),
    }
    populate_route_status(state, gateway, profiles, &text, &mut status)?;
    Ok(status)
}

fn populate_route_status(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    profiles: &[ProviderProfile],
    text: &str,
    status: &mut ConfigFileStatus,
) -> Result<(), CommandError> {
    let kind = status.app;
    let mut route = adapter::route_state(kind, text);
    status.gateway_port = crate::gateway::client_gateway_port(kind, text);
    // Route URLs cross the redact boundary under their ownership key: hosts
    // stay readable, token-shaped values still mask.
    route.base_url = route.base_url.map(|url| match kind {
        AppKind::Codex => asb_core::redact::redact(
            asb_core::ownership::CODEX_PROVIDER_BASE_URL_KEY,
            &url,
        ),
        AppKind::Claude => asb_core::redact::redact("env.ANTHROPIC_BASE_URL", &url),
    });
    status.route = Some(route);
    status.active_profile_id = if kind == AppKind::Codex {
        let routed = gateway
            .active_profile_id(AppKind::Codex, text)
            .map_err(|e| CommandError::new("codex-state-unavailable", e))?;
        let identity = match routed {
            Some(id) => Some(id),
            None => super::codex::active_direct(state, text)?,
        };
        match identity {
            Some(id) => Some(id),
            // A third-party route never reaches here; official routing falls
            // back to the stored official-login record so the row can show
            // its live state like Claude official profiles do.
            None => active_profile_id(
                profiles,
                Some(gateway),
                kind,
                text,
                status.last_switch.as_ref(),
            )?,
        }
    } else {
        active_profile_id(
            profiles,
            Some(gateway),
            kind,
            text,
            status.last_switch.as_ref(),
        )?
    };
    Ok(())
}
