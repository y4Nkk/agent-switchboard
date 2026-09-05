//! Display-only projection: provider credentials never enter the tray snapshot.

use crate::commands::{config_status_report, ConfigFileStatus};
use crate::local_state::{AppSettings, LocalState};
use asb_core::contracts::{AppKind, ProviderProfile, UsageSummary};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayProvider {
    id: String,
    app: AppKind,
    name: String,
    model: Option<String>,
    active: bool,
    usage: Option<UsageSummary>,
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
    usage: Option<UsageSummary>,
) -> TrayProvider {
    let status = statuses.iter().find(|status| status.app == profile.app);
    let active =
        status.is_some_and(|status| status.active_profile_id.as_ref() == Some(&profile.id));
    let model = if active {
        status
            .and_then(|status| status.route.as_ref())
            .and_then(|route| route.model.clone())
    } else {
        profile.model.clone()
    };
    TrayProvider {
        id: profile.id.clone(),
        app: profile.app,
        name: profile.name.clone(),
        model,
        active,
        usage,
    }
}

pub fn read(state: &LocalState, switching: bool) -> TraySnapshot {
    let mut errors = Vec::new();
    let settings = state
        .get_app_settings()
        .map_err(|error| errors.push(error))
        .ok();
    let statuses = config_status_report(state)
        .map_err(|error| errors.push(error.message))
        .unwrap_or_default();
    errors.extend(
        statuses
            .iter()
            .filter_map(|status| status.read_error.clone()),
    );
    let records = state
        .configuration()
        .list_providers()
        .map_err(|error| errors.push(error.to_string()))
        .unwrap_or_default();
    let providers = records
        .iter()
        .map(|record| {
            project(
                &record.profile,
                &statuses,
                crate::usage_cache::get(state, &record.profile),
            )
        })
        .collect();
    TraySnapshot {
        providers,
        settings,
        switching,
        error: (!errors.is_empty()).then(|| asb_core::adapter::scrub_message(errors.join("；"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::{ProviderDraft, RouteMode};

    #[test]
    fn projection_contains_only_display_fields() {
        let profile = ProviderProfile::from_draft(
            "p".into(),
            ProviderDraft {
                app: AppKind::Codex,
                route_mode: RouteMode::Custom,
                name: "供应商".into(),
                model: Some("model".into()),
                base_url: Some("https://example.invalid".into()),
                api_key: "private-test-key".into(),
                model_options: None,
                notes: None,
                website_url: None,
                usage_query: None,
            },
        );
        let value = serde_json::to_value(project(&profile, &[], None)).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 6);
        assert_eq!(value["active"], false);
        assert_eq!(value["model"], "model");
        assert!(!value.to_string().contains("private-test-key"));
        assert!(!value.to_string().contains("example.invalid"));
        let status = ConfigFileStatus {
            app: AppKind::Codex,
            path: String::new(),
            exists: true,
            syntax_ok: true,
            route: Some(asb_core::adapter::route_state(
                AppKind::Codex,
                "model = \"live-model\"\n",
            )),
            read_error: None,
            match_status: asb_core::contracts::MatchStatus::ExternallyModified {
                at: "test-time".into(),
            },
            active_profile_id: Some(profile.id.clone()),
            last_switch: None,
        };
        let value = serde_json::to_value(project(&profile, &[status], None)).unwrap();
        assert_eq!(value["active"], true);
        assert_eq!(value["model"], "live-model");
    }
}
