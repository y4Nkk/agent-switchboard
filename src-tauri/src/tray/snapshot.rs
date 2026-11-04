//! Display-only projection: provider credentials never enter the tray snapshot.

use crate::commands::{config_status_report, ConfigFileStatus};
use crate::gateway::GatewayController;
use crate::local_state::{AppSettings, LocalState};
use asb_core::contracts::{AppKind, CodexOfficialQuota, ProviderProfile, RouteMode, UsageSnapshot};
use serde::Serialize;

#[derive(Serialize)]
#[serde(tag = "kind", content = "reading", rename_all = "camelCase")]
enum TrayUsage {
    Script(UsageSnapshot),
    Official(CodexOfficialQuota),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayProvider {
    id: String,
    app: AppKind,
    name: String,
    active: bool,
    usage: Option<TrayUsage>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraySnapshot {
    providers: Vec<TrayProvider>,
    settings: Option<AppSettings>,
    error: Option<String>,
    switching: bool,
}

fn project(
    profile: &ProviderProfile,
    statuses: &[ConfigFileStatus],
    usage: Option<TrayUsage>,
) -> TrayProvider {
    let status = statuses.iter().find(|status| status.app == profile.app);
    let active =
        status.is_some_and(|status| status.active_profile_id.as_ref() == Some(&profile.id));
    TrayProvider {
        id: profile.id.clone(),
        app: profile.app,
        name: profile.name.clone(),
        active,
        usage,
    }
}

pub fn read(state: &LocalState, gateway: &GatewayController, switching: bool) -> TraySnapshot {
    let mut errors = Vec::new();
    let settings = state
        .get_app_settings()
        .map_err(|error| errors.push(error))
        .ok();
    let statuses = config_status_report(state, gateway)
        .map_err(|error| errors.push(error.message))
        .unwrap_or_default();
    errors.extend(
        statuses
            .iter()
            .filter_map(|status| status.read_error.clone()),
    );
    let mut providers = Vec::new();
    for app in [AppKind::Claude, AppKind::Codex] {
        let files = crate::config_store::providers::load_provider_files(&state.configuration(), app)
            .map_err(|error| errors.push(format!("{app:?}: {error}")))
            .unwrap_or_default();
        for file in files {
            let profile = file.into_profile(app);
            providers.push(project(&profile, &statuses, cached_usage(state, &profile, &mut errors)));
        }
    }
    let codex_records = state
        .configuration()
        .list_codex_providers()
        .map_err(|error| errors.push(error.to_string()))
        .unwrap_or_default();
    for record in codex_records {
        match state
            .configuration()
            .find_codex_provider_file(&record.profile.id)
        {
            Ok(file) => {
                let profile = file.client_projection().into_profile(AppKind::Codex);
                providers.push(project(
                    &profile,
                    &statuses,
                    cached_usage(state, &profile, &mut errors),
                ));
            }
            Err(error) => errors.push(error.to_string()),
        }
    }
    TraySnapshot {
        providers,
        settings,
        switching,
        error: (!errors.is_empty()).then(|| asb_core::adapter::scrub_message(errors.join("；"))),
    }
}

fn cached_usage(
    state: &LocalState,
    profile: &ProviderProfile,
    errors: &mut Vec<String>,
) -> Option<TrayUsage> {
    if profile.route_mode != RouteMode::Official {
        return crate::usage_cache::get(state, profile)
            .map_err(|error| errors.push(error)).ok().flatten().map(TrayUsage::Script);
    }
    if profile.app != AppKind::Codex {
        return None;
    }
    LocalState::codex_auth_path()
        .and_then(|path| crate::codex_auth::cached_profile_quota(state.root(), &profile.id, &path))
        .map_err(|error| errors.push(error))
        .ok()
        .flatten()
        .map(TrayUsage::Official)
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::{ProviderDraft, RouteMode, UpstreamProtocol};

    #[test]
    fn usage_serialization_has_one_explicit_source() {
        let script = UsageSnapshot { summary: Some(asb_core::contracts::UsageSummary {
            readings: vec![], at: "2026-09-21T00:00:00Z".into(),
        }), error: None };
        let official = CodexOfficialQuota {
            status: asb_core::contracts::CodexOfficialQuotaStatus::Available,
            windows: vec![asb_core::contracts::CodexOfficialQuotaWindow {
                label: "5 小时".into(), used_percent: 25.0, resets_at: None,
            }],
            at: Some("2026-09-21T00:00:00Z".into()),
            stale: false,
            last_reset: None,
            failure: None,
        };
        let script = serde_json::to_value(TrayUsage::Script(script)).unwrap();
        let official = serde_json::to_value(TrayUsage::Official(official)).unwrap();
        assert_eq!(script["kind"], "script");
        assert!(script["reading"]["summary"].get("readings").is_some());
        assert!(script["reading"].get("windows").is_none());
        assert_eq!(official["kind"], "official");
        assert_eq!(official["reading"]["windows"][0]["usedPercent"], 25.0);
        assert!(official["reading"].get("readings").is_none());
        assert_eq!(script.as_object().unwrap().len(), 2);
        assert_eq!(official.as_object().unwrap().len(), 2);
    }

    #[test]
    fn projection_contains_only_display_fields() {
        let profile = ProviderProfile::from_draft(
            "p".into(),
            ProviderDraft {
                authentication: None,
                parameters: asb_core::ownership::default_provider_parameters(AppKind::Claude),
                claude_fragment: Default::default(),
                app: AppKind::Claude,
                route_mode: RouteMode::Custom,
                name: "供应商".into(),
                model: Some("model".into()),
                base_url: Some("https://example.invalid".into()),
                connection: Default::default(),
                api_key: "private-test-key".into(),
                upstream_protocol: Some(UpstreamProtocol::AnthropicMessages),
                responses_options: None,
                max_output_tokens: None.into(),
                model_options: None,
                notes: None,
                website_url: None,
                usage_query: None,
                official_quota_refresh_interval_minutes: None,
                display: None,
            },
        );
        let value = serde_json::to_value(project(&profile, &[], None)).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 5);
        assert!(value["usage"].is_null());
        assert_eq!(value["active"], false);
        assert!(!value.to_string().contains("private-test-key"));
        assert!(!value.to_string().contains("example.invalid"));
        let status = ConfigFileStatus {
            gateway_port: None,
            app: AppKind::Claude,
            path: String::new(),
            exists: true,
            syntax_ok: true,
            route: None,
            read_error: None,
            recovery_issue: None,
            client_settings: None,
            client_settings_error: None,
            codex_permission_profile: None,
            active_profile_id: Some(profile.id.clone()),
            last_switch: None,
        };
        let value = serde_json::to_value(project(&profile, &[status], None)).unwrap();
        assert_eq!(value["active"], true);
    }
}
