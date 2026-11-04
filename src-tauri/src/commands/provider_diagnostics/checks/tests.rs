use super::*;
use asb_core::{contracts::*, ownership};
use crate::gateway::{GatewayFailureKind, GatewayFailureReport, GatewayMetricsSnapshot, ProviderHealthState, RouteObservation};

fn profile() -> ProviderProfile {
    ProviderProfile::from_draft("isolated-provider".into(),
        asb_core::codex_official_draft(ownership::default_provider_parameters(AppKind::Codex)))
}

fn status(profile: &ProviderProfile, configuration: &str) -> ConfigFileStatus {
    let mut route = asb_core::route_state(profile.app, configuration);
    route.base_url = Some(asb_core::REDACTED.into());
    ConfigFileStatus {
        app: profile.app, path: "isolated-config".into(), exists: true, syntax_ok: true,
        route: Some(route), gateway_port: crate::gateway::client_gateway_port(profile.app, configuration),
        read_error: None, recovery_issue: None, client_settings: None, client_settings_error: None, codex_permission_profile: None,
        active_profile_id: Some(profile.id.clone()), last_switch: None,
    }
}

fn observation(profile: &ProviderProfile, state: GatewayStatusKind) -> GatewayObservation {
    GatewayObservation {
        configured_port: 47821, listening_port: None, base_url: None, status: state,
        failure: None, repair_reason: None, blocked_recovery: None,
        routes: vec![RouteObservation { app: profile.app, profile_id: profile.id.clone(),
            upstream_protocol: UpstreamProtocol::Responses, health: ProviderHealthState::Closed,
            consecutive_failures: 0, candidate_count: 1 }],
        metrics: GatewayMetricsSnapshot { started_at_ms: 0, total_requests: 0, failed_requests: 0, samples: vec![] },
    }
}

fn gateway_configuration() -> String {
    format!("model_provider = 'openai'\nopenai_base_url = 'http://127.0.0.1:47821/codex/asb_codex_{}/v1'\n", "a".repeat(64))
}

#[test]
fn environment_reports_names_and_process_scope_only() {
    let result = environment_for(vec!["OPENAI_API_KEY", "HTTPS_PROXY"]);
    assert_eq!(result.status, "warning");
    assert_eq!(result.message.params.unwrap()["names"], "OPENAI_API_KEY, HTTPS_PROXY");
    assert_eq!(environment_for(vec![]).status, "unknown");
}

#[test]
fn inactive_provider_has_no_route_failure_when_gateway_is_unavailable() {
    let profile = profile();
    let status = status(&profile, &gateway_configuration());
    let result = route(&profile, &status, false, observation(&profile, GatewayStatusKind::PortConflict));
    assert_eq!(result.status, "unknown");
    assert_eq!(result.message.key, "providerDiagnostics.backend.inactiveRoute");
}

#[test]
fn official_takeover_is_diagnosed_from_raw_routing_facts_after_display_redaction() {
    let profile = profile();
    let status = status(&profile, &gateway_configuration());
    assert_eq!(status.gateway_port, Some(47821));
    let mut observed = observation(&profile, GatewayStatusKind::Running);
    observed.listening_port = Some(47821);
    let result = route(&profile, &status, true, observed);
    assert_eq!(result.status, "pass");
    assert_eq!(result.message.key, "providerDiagnostics.backend.gatewayReady");
}

#[test]
fn stopped_conflicted_and_blocked_gateways_keep_structured_facts_without_a_write_plan() {
    let profile = profile();
    let status = status(&profile, &gateway_configuration());
    for state in [GatewayStatusKind::BindRejected, GatewayStatusKind::PortConflict, GatewayStatusKind::RecoveryBlocked] {
        let mut observed = observation(&profile, state);
        let detail = if state == GatewayStatusKind::RecoveryBlocked {
            observed.blocked_recovery = Some(crate::gateway::port_change::BlockedPortChange {
                from_port: 47820, to_port: 47821, apps: vec![AppKind::Codex], reason: "isolated recovery pending".into(),
            });
            "isolated recovery pending"
        } else {
            observed.failure = Some(GatewayFailureReport { port: 47821, kind: GatewayFailureKind::PortInUse,
                os_code: Some(10048), message: "isolated listener failure".into(), process: None });
            "isolated listener failure"
        };
        let result = route(&profile, &status, true, observed);
        assert_eq!(result.status, "error");
        assert_eq!(result.message.key, "providerDiagnostics.backend.gatewayFacts");
        assert_eq!(result.suggestion.unwrap().key, "providerDiagnostics.backend.openGateway");
        let facts = result.message.params.unwrap();
        assert_eq!(facts["configuredPort"], 47821);
        assert_eq!(facts["clientPort"], "47821");
        assert_eq!(facts["listeningPort"], "—");
        assert_eq!(facts["stateKey"], gateway_state_key(state));
        assert_eq!(facts["detail"], detail);
    }
}

#[test]
fn a_live_listener_on_a_different_client_port_or_missing_route_is_not_ready() {
    let profile = profile();
    let mut status = status(&profile, &gateway_configuration());
    status.gateway_port = Some(47822);
    let mut observed = observation(&profile, GatewayStatusKind::Running);
    observed.listening_port = Some(47821);
    assert_eq!(route(&profile, &status, true, observed).status, "error");
    status.gateway_port = Some(47821);
    let mut observed = observation(&profile, GatewayStatusKind::Running);
    observed.listening_port = Some(47821);
    observed.routes.clear();
    assert_eq!(route(&profile, &status, true, observed).status, "error");
}

#[test]
fn projection_failure_keeps_its_structured_reason() {
    let profile = profile();
    let status = status(&profile, "model = 'fixture-model'\n");
    let error = CommandError::localized(
        "isolated-projection-failure", "providerDiagnostics.backend.projectionDetail",
        "isolated projection detail", serde_json::json!({ "detail": "isolated projection detail" }),
    );
    let result = configuration(&status, true, Err(&error));
    assert_eq!(result.status, "error");
    assert_eq!(result.message.key, "providerDiagnostics.backend.projectionDetail");
    assert_eq!(result.message.params.unwrap()["detail"], "isolated projection detail");
}

#[test]
fn observed_configuration_errors_remain_diagnostic_without_a_projection() {
    let profile = profile();
    let error = CommandError::new("isolated-projection-failure", "projection unavailable");
    for diagnostic in ["read", "syntax", "settings", "recovery"] {
        let mut status = status(&profile, "model = 'fixture-model'\n");
        match diagnostic {
            "read" => status.read_error = Some("isolated read detail".into()),
            "syntax" => status.syntax_ok = false,
            "settings" => status.client_settings_error = Some("isolated setting detail".into()),
            "recovery" => status.recovery_issue = Some("isolated recovery detail".into()),
            _ => unreachable!(),
        }
        let result = configuration(&status, true, Err(&error));
        assert_eq!(result.status, "error");
        assert_eq!(result.category, "configuration");
        if diagnostic != "syntax" {
            assert_eq!(result.message.key, "providerDiagnostics.backend.configurationDetail");
            assert!(result.message.params.unwrap()["detail"].as_str().unwrap().contains("isolated"));
        }
    }
}
