use crate::{
    adapter::AdapterError,
    claude_common::Extra,
    contracts::{ConfigValue, ModelOptions, SwitchPlan},
};

fn error(message: String) -> AdapterError {
    AdapterError {
        message,
        line: None,
    }
}

pub(super) fn projected(plan: &SwitchPlan) -> Result<(Extra, Extra), AdapterError> {
    let shared = plan.client_settings.claude_extra.clone();
    crate::claude_common::validate(&shared).map_err(error)?;
    let profile = projected_profile(plan)?;
    crate::claude_common::validate_scopes(&shared, &profile).map_err(error)?;
    Ok((shared, profile))
}

pub(super) fn projected_profile(plan: &SwitchPlan) -> Result<Extra, AdapterError> {
    project(&plan.profile.claude_fragment, plan)
}

fn project(extra: &Extra, plan: &SwitchPlan) -> Result<Extra, AdapterError> {
    crate::claude_common::validate(extra).map_err(error)?;
    let mut result = extra.clone();
    if !plan.is_gateway() {
        return Ok(result);
    }
    let Some(preferences) = extra
        .get("modelSettings")
        .and_then(serde_json::Value::as_object)
    else {
        return Ok(result);
    };
    let settings = match &plan.profile.model_options {
        Some(ModelOptions::Claude(settings)) => Some(settings),
        _ => None,
    };
    let roles = [
        "model",
        "env.ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "env.ANTHROPIC_DEFAULT_SONNET_MODEL",
        "env.ANTHROPIC_DEFAULT_OPUS_MODEL",
        "env.ANTHROPIC_DEFAULT_FABLE_MODEL",
        "env.CLAUDE_CODE_SUBAGENT_MODEL",
    ];
    let mut mapped = serde_json::Map::new();
    for (model, value) in preferences {
        let mut targets = Vec::new();
        for key in roles {
            if let Some(Some(ConfigValue::Str(client_model))) =
                super::gateway::model_value(plan, key, settings)
            {
                let (client_model, _) =
                    crate::claude_model::parse_model(&client_model, key, true).map_err(error)?;
                let source = super::gateway::gateway_model_source(
                    &client_model,
                    plan.profile.model.as_deref(),
                    settings,
                )
                .map_err(error)?;
                let fixed_target = crate::contracts::ClaudeModelRole::from_wire(&client_model)
                    .and_then(|role| settings.and_then(|s| s.effective_route(role)))
                    .is_some_and(|route| route.profile_id().is_some());
                if !fixed_target && source == Some(model.as_str()) {
                    targets.push(client_model);
                }
            }
        }
        if targets.is_empty() {
            targets.push(model.clone());
        }
        for target in targets {
            if mapped
                .get(&target)
                .is_some_and(|existing| existing != value)
            {
                return Err(error(format!(
                    "模型偏好映射到 {target} 时发生冲突，请只保留一份配置"
                )));
            }
            mapped.insert(target, value.clone());
        }
    }
    result.insert("modelSettings".into(), serde_json::Value::Object(mapped));
    Ok(result)
}
