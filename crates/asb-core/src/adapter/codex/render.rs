use crate::adapter::{AdapterError, OverlayEntry};
use crate::contracts::{SettingsValues, SwitchPlan};

use crate::adapter::codex::common_fragment::{merge_fragment, remove_applied_fragment, validate_fragment};
use crate::adapter::codex::document::{parse, remove_empty_table_path, remove_path, set_path};
use crate::adapter::codex::overlay::{client_settings_overlay, overlay};

pub(crate) fn render(current: &str, plan: &SwitchPlan) -> Result<String, AdapterError> {
    super::validate_projection(plan)?;
    let rendered = render_entries(current, overlay(plan))?;
    if !plan.writes_client_settings() {
        return Ok(rendered);
    }
    let candidate = match plan.codex_common_fragment() {
        None => Ok(rendered),
        Some(fragment) => {
            // 存储层可能被手工改动，投影边界处再做一次 fail-closed 校验。
            validate_fragment(&fragment.text)?;
            if fragment.enabled {
                merge_fragment(&rendered, &fragment.text)
            } else {
                remove_applied_fragment(&rendered, &fragment.text)
            }
        }
    }?;
    super::permissions::validate_candidate(&parse(&candidate)?)?;
    Ok(candidate)
}

pub(crate) fn render_gateway_base_url(
    current: &str,
    base_url: &str,
) -> Result<String, AdapterError> {
    if !super::is_gateway_base_url(base_url) {
        return Err(AdapterError {
            message: "Codex 网关地址必须是本机带路由凭证的入口".into(),
            line: None,
        });
    }
    render_entries(
        current,
        vec![(
            crate::ownership::CODEX_PROVIDER_BASE_URL_KEY.to_string(),
            OverlayEntry::Set(crate::contracts::ConfigValue::Str(base_url.to_string())),
        )],
    )
}

pub(crate) fn render_client_settings_into_file(
    current: &str,
    client_settings: &SettingsValues,
) -> Result<String, AdapterError> {
    render_entries(current, client_settings_overlay(client_settings))
}

#[cfg(test)]
mod subagent_route_render_tests {
    use super::render;
    use crate::contracts::{
        AppKind, CodexCatalogEntry, CodexEndpoint, CodexProviderDraft, CodexRouteMode,
        CodexSubagentRoute, CodexUpstream, ResponsesRequestMode, SwitchPlan,
        DEFAULT_CODEX_CAPABILITIES,
    };

    fn plan(subagent_route: Option<CodexSubagentRoute>) -> SwitchPlan {
        let draft = CodexProviderDraft {
            name: "Relay".into(),
            endpoint: CodexEndpoint("https://relay.example/v1".into()),
            api_key: "fixture-key".into(),
            authentication: None,
            connection: Default::default(),
            upstream: CodexUpstream::Responses,
            request_mode: ResponsesRequestMode::Standard,
            default_model: "relay-model".into(),
            catalog: vec![CodexCatalogEntry::default_entry("relay-model")],
            model_routes: Vec::new(),
            subagent_route,
            capabilities: DEFAULT_CODEX_CAPABILITIES,
            parameters: crate::ownership::default_provider_parameters(AppKind::Codex),
            notes: None,
            website_url: None,
            usage_query: None,
        };
        let file = draft.into_file("01234567-89ab-cdef-0123-456789abcdef".to_string(), 1);
        assert_eq!(
            file.profile.route_mode,
            if file.profile.subagent_route.is_some() {
                CodexRouteMode::Gateway
            } else {
                CodexRouteMode::Direct
            }
        );
        let profile = file.client_projection().into_profile(AppKind::Codex);
        SwitchPlan::direct(
            profile,
            crate::ownership::default_client_settings(AppKind::Codex),
        )
    }

    #[test]
    fn a_subagent_route_renders_its_wire_id() {
        let rendered = render(
            "model_provider = 'openai'\n",
            &plan(Some(CodexSubagentRoute {
                profile_id: "01234567-89ab-cdef-0123-456789abcdef".to_string(),
                model: "relay-model".to_string(),
            })),
        )
        .expect("render");
        assert!(rendered
            .contains("default_subagent_model = \"asb:01234567-89ab-cdef-0123-456789abcdef/relay-model\""));
    }

    #[test]
    fn a_missing_route_removes_the_managed_key() {
        let rendered = render(
            "[agents]\ndefault_subagent_model = \"asb:stale/route-model\"\n",
            &plan(None),
        )
        .expect("render");
        assert!(!rendered.contains("default_subagent_model"));
    }
}


pub(crate) fn render_entries(
    current: &str,
    entries: Vec<(String, OverlayEntry)>,
) -> Result<String, AdapterError> {
    let mut doc = parse(current)?;
    let writes_permissions = entries.iter().any(|(key, _)| key == "sandbox_mode" || key.starts_with("sandbox_workspace_write."));
    for (key, entry) in entries {
        match entry {
            OverlayEntry::Set(value) => set_path(&mut doc, &key, value)?,
            OverlayEntry::Leave => {}
            OverlayEntry::RemoveIfPresent => {
                remove_path(&mut doc, &key)?;
            }
            OverlayEntry::RemoveTableIfEmpty => {
                remove_empty_table_path(&mut doc, &key)?;
            }
        }
    }
    if writes_permissions {
        remove_empty_table_path(&mut doc, "sandbox_workspace_write")?;
        super::permissions::validate_candidate(&doc)?;
    }
    Ok(doc.to_string())
}
