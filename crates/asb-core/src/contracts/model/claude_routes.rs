use super::ClaudeModelSettings;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeModelRole {
    Haiku,
    Sonnet,
    Opus,
    Fable,
    Subagent,
}

impl ClaudeModelRole {
    pub const ALL: [Self; 5] = [
        Self::Haiku,
        Self::Sonnet,
        Self::Opus,
        Self::Fable,
        Self::Subagent,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Haiku => "haiku",
            Self::Sonnet => "sonnet",
            Self::Opus => "opus",
            Self::Fable => "fable",
            Self::Subagent => "subagent",
        }
    }
    pub fn wire(self) -> &'static str {
        match self {
            Self::Haiku => "asb-claude-haiku",
            Self::Sonnet => "asb-claude-sonnet",
            Self::Opus => "asb-claude-opus",
            Self::Fable => "asb-claude-fable",
            Self::Subagent => "asb-claude-subagent",
        }
    }
    pub fn from_wire(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.wire() == value)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub enum ClaudeModelRoute {
    Primary {
        model: String,
        #[serde(rename = "oneM")]
        one_m: bool,
    },
    Provider {
        #[serde(rename = "profileId")]
        profile_id: String,
        model: String,
        #[serde(rename = "oneM")]
        one_m: bool,
    },
}
impl ClaudeModelRoute {
    pub fn model(&self) -> &str {
        match self {
            Self::Primary { model, .. } | Self::Provider { model, .. } => model,
        }
    }
    pub fn one_m(&self) -> bool {
        match self {
            Self::Primary { one_m, .. } | Self::Provider { one_m, .. } => *one_m,
        }
    }
    pub fn profile_id(&self) -> Option<&str> {
        match self {
            Self::Provider { profile_id, .. } => Some(profile_id),
            _ => None,
        }
    }
    pub fn primary(model: Option<String>, one_m: bool) -> Option<Self> {
        model.map(|model| Self::Primary { model, one_m })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaudeModelRoutes {
    pub haiku: Option<ClaudeModelRoute>,
    pub sonnet: Option<ClaudeModelRoute>,
    pub opus: Option<ClaudeModelRoute>,
    pub fable: Option<ClaudeModelRoute>,
    pub subagent: Option<ClaudeModelRoute>,
}
impl ClaudeModelSettings {
    pub fn route(&self, role: ClaudeModelRole) -> Option<&ClaudeModelRoute> {
        match role {
            ClaudeModelRole::Haiku => self.routes.haiku.as_ref(),
            ClaudeModelRole::Sonnet => self.routes.sonnet.as_ref(),
            ClaudeModelRole::Opus => self.routes.opus.as_ref(),
            ClaudeModelRole::Fable => self.routes.fable.as_ref(),
            ClaudeModelRole::Subagent => self.routes.subagent.as_ref(),
        }
    }
    /// Fable inherits Opus only when it has no explicit override.
    pub fn effective_route(&self, role: ClaudeModelRole) -> Option<&ClaudeModelRoute> {
        self.route(role).or_else(|| match role {
            ClaudeModelRole::Fable => self.route(ClaudeModelRole::Opus),
            _ => None,
        })
    }
    pub fn model(&self, role: ClaudeModelRole) -> Option<&str> {
        self.route(role).map(ClaudeModelRoute::model)
    }
    pub fn one_m(&self, role: ClaudeModelRole) -> bool {
        self.route(role).is_some_and(ClaudeModelRoute::one_m)
    }
    pub fn has_provider_routes(&self) -> bool {
        ClaudeModelRole::ALL
            .into_iter()
            .any(|role| self.route(role).is_some_and(|r| r.profile_id().is_some()))
    }
}

/// Offline conversion only. Current readers never accept the retired fields.
pub fn upgrade_claude_model_options(value: &mut serde_json::Value) -> Result<bool, String> {
    use serde_json::{json, Value};
    let Some(object) = value.as_object_mut() else {
        return Ok(false);
    };
    if object.get("kind").and_then(Value::as_str) != Some("claude") {
        return Ok(false);
    }
    let old_keys = [
        "haikuModel",
        "haikuOneM",
        "sonnetModel",
        "sonnetOneM",
        "opusModel",
        "opusOneM",
        "fableModel",
        "fableOneM",
        "subagentModel",
        "subagentOneM",
    ];
    if object.contains_key("routes") {
        if old_keys.iter().any(|key| object.contains_key(*key)) {
            return Err("Claude model options mix current routes with retired tier fields".into());
        }
        return Ok(false);
    }
    let mut routes = serde_json::Map::new();
    for role in ClaudeModelRole::ALL {
        let model = object
            .get(&format!("{}Model", role.name()))
            .cloned()
            .unwrap_or(Value::Null);
        let one_m = object
            .get(&format!("{}OneM", role.name()))
            .cloned()
            .unwrap_or(json!(false));
        if !one_m.is_boolean() {
            return Err(format!("{}OneM must be boolean", role.name()));
        }
        let route = match model {
            Value::Null if one_m == json!(false) => Value::Null,
            Value::String(model) if !model.trim().is_empty() => {
                json!({"mode":"primary","model":model,"oneM":one_m})
            }
            _ => return Err(format!("invalid {} model or 1M flag", role.name())),
        };
        routes.insert(role.name().into(), route);
    }
    for key in old_keys {
        object.remove(key);
    }
    object.insert("routes".into(), Value::Object(routes));
    Ok(true)
}

#[cfg(test)]
mod tests;
