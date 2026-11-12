use asb_core::contracts::{ClaudeModelSettings, ClaudeModelRole};
use serde_json::Value;

const ONE_M_SUFFIX: &str = "[1m]";

/// Applies Claude Code's provider-owned model tiers before the request enters
/// protocol conversion. The client keeps its configured model spelling, while
/// upstream receives only the selected provider model without the local 1M
/// capability marker.
pub(crate) fn apply_claude_model_mapping(
    body: &[u8],
    primary_model: Option<&str>,
    settings: Option<&ClaudeModelSettings>,
) -> Result<Vec<u8>, super::TransformError> {
    let mut value: Value = serde_json::from_slice(body)
        .map_err(|_| super::TransformError("请求体不是有效 JSON".to_string()))?;
    let Some(original) = value.get("model").and_then(Value::as_str) else {
        return Ok(body.to_vec());
    };
    let normalized = strip_one_m_suffix(original);
    let mapped = map_model(normalized, primary_model, settings)?;
    let outgoing = strip_one_m_suffix(mapped).to_string();
    if outgoing == original {
        return Ok(body.to_vec());
    }
    value["model"] = Value::String(outgoing);
    serde_json::to_vec(&value)
        .map_err(|_| super::TransformError("无法编码 Claude 模型路由请求".to_string()))
}

fn map_model<'a>(
    original: &'a str,
    primary_model: Option<&'a str>,
    settings: Option<&'a ClaudeModelSettings>,
) -> Result<&'a str, super::TransformError> {
    if let Some(model) = asb_core::adapter::claude::gateway_model_source(original, primary_model, settings)
        .map_err(super::TransformError)? {
        return Ok(model);
    }
    let local_model = |role| settings.and_then(|value| value.route(role))
        .filter(|route| route.profile_id().is_none()).map(|route| route.model());
    if let Some(subagent) = local_model(ClaudeModelRole::Subagent) {
        if strip_one_m_suffix(subagent) == original { return Ok(original); }
    }
    let name = original.to_ascii_lowercase();
    let role = [ClaudeModelRole::Fable, ClaudeModelRole::Haiku, ClaudeModelRole::Opus, ClaudeModelRole::Sonnet]
        .into_iter().find(|role| name.contains(role.name()));
    let mapped = role.and_then(|role| local_model(role))
        .or_else(|| (role == Some(ClaudeModelRole::Fable)).then(|| local_model(ClaudeModelRole::Opus)).flatten());
    Ok(mapped.or(primary_model).unwrap_or(original))
}

fn strip_one_m_suffix(model: &str) -> &str {
    let trimmed = model.trim_end();
    if trimmed.len() >= ONE_M_SUFFIX.len()
        && trimmed.as_bytes()[trimmed.len() - ONE_M_SUFFIX.len()..]
            .eq_ignore_ascii_case(ONE_M_SUFFIX.as_bytes())
    {
        return trimmed[..trimmed.len() - ONE_M_SUFFIX.len()].trim_end();
    }
    model
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ordinary_model_names_never_map_a_foreign_providers_model_onto_the_primary_connection() {
        use asb_core::contracts::{ClaudeModelRoute, ClaudeModelRoutes};
        let fixed = |model: &str| Some(ClaudeModelRoute::Provider {
            profile_id:"11111111-1111-4111-8111-111111111111".into(),model:model.into(),one_m:false,
        });
        let settings = ClaudeModelSettings { routes:ClaudeModelRoutes {
            sonnet:fixed("shared-name"),opus:fixed("foreign-opus"),subagent:fixed("foreign-child"),
            haiku:Some(ClaudeModelRoute::Primary {model:"local-haiku".into(),one_m:false}),
            ..Default::default()
        },..Default::default() };
        for requested in ["claude-sonnet-5","claude-fable-5","claude-opus-5","shared-name","foreign-opus","foreign-child"] {
            assert_eq!(map_model(requested,Some("source-main"),Some(&settings)).unwrap(),"source-main", "{requested}");
        }
        assert_eq!(map_model("claude-haiku-5",Some("source-main"),Some(&settings)).unwrap(),"local-haiku");
        // Explicit internal roles remain reserved for the destination resolver.
        assert_eq!(map_model("asb-claude-sonnet",Some("source-main"),Some(&settings)).unwrap(),"shared-name");
    }

    fn settings() -> ClaudeModelSettings {
        use asb_core::contracts::{ClaudeModelRoute, ClaudeModelRoutes};
        let local = |model: &str| Some(ClaudeModelRoute::Primary { model: model.into(), one_m: false });
        ClaudeModelSettings {
            routes: ClaudeModelRoutes { haiku: local("vendor-haiku"), sonnet: local("vendor-sonnet"),
                opus: local("vendor-opus"), subagent: local("vendor-agent"), ..Default::default() },
            ..Default::default()
        }
    }

    fn model(body: Value) -> String {
        let bytes = apply_claude_model_mapping(
            &body.to_string().into_bytes(),
            Some("default"),
            Some(&settings()),
        )
        .expect("model mapping");
        serde_json::from_slice::<Value>(&bytes).expect("mapped JSON")["model"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn maps_claude_tiers_and_fable_falls_back_to_opus() {
        assert_eq!(model(json!({"model":"claude-haiku-4"})), "vendor-haiku");
        assert_eq!(
            model(json!({"model":"claude-sonnet-4[1M]"})),
            "vendor-sonnet"
        );
        assert_eq!(model(json!({"model":"claude-opus-4"})), "vendor-opus");
        assert_eq!(model(json!({"model":"claude-fable-4"})), "vendor-opus");
    }

    #[test]
    fn keeps_configured_subagent_model_and_uses_primary_for_unknown_models() {
        assert_eq!(model(json!({"model":"vendor-agent[1m]"})), "vendor-agent");
        assert_eq!(model(json!({"model":"unknown-model"})), "default");
    }

    #[test]
    fn strips_one_m_without_a_mapping() {
        let body = json!({"model":"vendor-model[1m]", "stream":true});
        let bytes = apply_claude_model_mapping(&body.to_string().into_bytes(), None, None)
            .expect("strip marker");
        let mapped: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(mapped["model"], "vendor-model");
        assert_eq!(mapped["stream"], true);
    }

    #[test]
    fn internal_roles_require_configured_models_instead_of_hardcoded_defaults() {
        for model in ["asb-claude-primary", "asb-claude-haiku", "asb-claude-unknown"] {
            let body = json!({"model":model});
            assert!(apply_claude_model_mapping(&serde_json::to_vec(&body).unwrap(), None, None).is_err());
        }
        assert_eq!(model(json!({"model":"asb-claude-fable"})), "vendor-opus");
        assert_eq!(model(json!({"model":"asb-claude-subagent"})), "vendor-agent");
    }

    #[test]
    fn preserves_model_whitespace_when_no_one_m_suffix_is_present() {
        assert_eq!(strip_one_m_suffix("vendor-model "), "vendor-model ");
    }

    #[test]
    fn unicode_model_identifiers_never_slice_inside_a_codepoint() {
        assert_eq!(strip_one_m_suffix("中文模型"), "中文模型");
        assert_eq!(strip_one_m_suffix("中文模型[1M]"), "中文模型");
    }
}
