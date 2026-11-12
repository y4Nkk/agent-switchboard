use super::{parse_ccswitch_model, parse_optional_model};
use crate::contracts::{
    ClaudeModelDisplayNames, ClaudeModelRoute, ClaudeModelRoutes, ClaudeModelSettings, ModelOptions,
};
use serde_json::{Map, Value};

#[derive(Clone, Copy)]
pub(crate) enum ModelSource {
    Client,
    CcSwitch,
}

impl ModelSource {
    fn decode(
        self,
        value: Option<&str>,
        name: &str,
        one_m: bool,
    ) -> Result<(Option<String>, bool), String> {
        match self {
            Self::Client => parse_optional_model(value, name, one_m),
            Self::CcSwitch => parse_ccswitch_model(value, name, one_m),
        }
    }
}

pub(crate) fn import_models(
    config: &Value,
    source: ModelSource,
) -> Result<(Option<String>, Option<ModelOptions>), String> {
    let env = match config.get("env") {
        None | Some(Value::Null) => None,
        Some(Value::Object(env)) => Some(env),
        _ => return Err("Claude env must be an object".into()),
    };
    let primary = text(
        env.and_then(|env| env.get("ANTHROPIC_MODEL")),
        "ANTHROPIC_MODEL",
    )?
    .or(text(config.get("model"), "model")?);
    let (model, primary_one_m) = source.decode(primary, "primary model", true)?;
    let mut settings = read_roles(env, source)?;
    settings.primary_one_m = primary_one_m;
    settings.display_names = read_names(env)?;
    settings.available_models = read_available(config.get("availableModels"), source)?;
    let options =
        (settings != ClaudeModelSettings::default()).then_some(ModelOptions::Claude(settings));
    Ok((model, options))
}

fn read_roles(
    env: Option<&Map<String, Value>>,
    source: ModelSource,
) -> Result<ClaudeModelSettings, String> {
    let field = |key: &str| text(env.and_then(|env| env.get(key)), key);
    let haiku = field("ANTHROPIC_DEFAULT_HAIKU_MODEL")?;
    let (haiku_model, haiku_one_m) = source.decode(haiku, "Haiku", true)?;
    let (sonnet_model, sonnet_one_m) =
        source.decode(field("ANTHROPIC_DEFAULT_SONNET_MODEL")?, "Sonnet", true)?;
    let (opus_model, opus_one_m) =
        source.decode(field("ANTHROPIC_DEFAULT_OPUS_MODEL")?, "Opus", true)?;
    let (fable_model, fable_one_m) =
        source.decode(field("ANTHROPIC_DEFAULT_FABLE_MODEL")?, "Fable", true)?;
    let (subagent_model, subagent_one_m) =
        source.decode(field("CLAUDE_CODE_SUBAGENT_MODEL")?, "subagent model", true)?;
    Ok(ClaudeModelSettings {
        routes: ClaudeModelRoutes {
            haiku: ClaudeModelRoute::primary(haiku_model, haiku_one_m),
            sonnet: ClaudeModelRoute::primary(sonnet_model, sonnet_one_m),
            opus: ClaudeModelRoute::primary(opus_model, opus_one_m),
            fable: ClaudeModelRoute::primary(fable_model, fable_one_m),
            subagent: ClaudeModelRoute::primary(subagent_model, subagent_one_m),
        },
        ..ClaudeModelSettings::default()
    })
}

fn read_names(env: Option<&Map<String, Value>>) -> Result<Option<ClaudeModelDisplayNames>, String> {
    let name =
        |key| text(env.and_then(|env| env.get(key)), key).map(|name| name.map(str::to_string));
    let names = ClaudeModelDisplayNames {
        haiku: name("ANTHROPIC_DEFAULT_HAIKU_MODEL_NAME")?,
        sonnet: name("ANTHROPIC_DEFAULT_SONNET_MODEL_NAME")?,
        opus: name("ANTHROPIC_DEFAULT_OPUS_MODEL_NAME")?,
        fable: name("ANTHROPIC_DEFAULT_FABLE_MODEL_NAME")?,
    };
    Ok((names != ClaudeModelDisplayNames::default()).then_some(names))
}

fn read_available(
    value: Option<&Value>,
    source: ModelSource,
) -> Result<Option<Vec<String>>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                let model = text(Some(value), "availableModels")?
                    .ok_or("availableModels contains an empty model")?;
                let (model, _) = source.decode(Some(model), "availableModels", false)?;
                model.ok_or_else(|| "availableModels contains an empty model".into())
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        _ => Err("availableModels must be an array".into()),
    }
}

fn text<'a>(value: Option<&'a Value>, field: &str) -> Result<Option<&'a str>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok((!value.trim().is_empty()).then_some(value.as_str())),
        _ => Err(format!("{field} must be a string")),
    }
}
