use serde::{Deserialize, Serialize};

/// The declared rendering of a Codex Responses reasoning request when a
/// provider exposes only Chat Completions. This is deliberately profile data:
/// provider names and endpoint hosts are never used to infer a dialect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CodexChatReasoning {
    Unsupported,
    Configured {
        thinking_parameter: CodexChatThinkingParameter,
        separate_output: bool,
        effort_parameter: CodexChatEffortParameter,
        effort_mode: CodexChatEffortMode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexChatThinkingParameter {
    None,
    Thinking,
    EnableThinking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexChatEffortParameter {
    None,
    ReasoningEffort,
    ReasoningObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexChatEffortMode {
    Passthrough,
    LowHigh,
    DeepSeek,
    OpenRouter,
    /// Clamp effort against the requested model's declared catalog ladder.
    Catalog,
}

/// One Codex-visible reasoning level declared for a specific catalog model.
/// The provider file owns these levels; the generated model catalog merely
/// projects them for the local Codex client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CodexReasoningLevel {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
    Ultra,
}

impl CodexReasoningLevel {
    pub const fn description(self) -> &'static str {
        match self {
            Self::None => "Disable Thinking",
            Self::Minimal => "Minimal Thinking",
            Self::Low => "Low Thinking",
            Self::Medium => "Medium Thinking",
            Self::High => "High Thinking",
            Self::Xhigh => "Extra High Thinking",
            Self::Max => "Max Thinking",
            Self::Ultra => "Ultra Thinking",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn configuration() -> serde_json::Value {
        json!({
            "kind":"configured", "thinkingParameter":"thinking", "separateOutput":true,
            "effortParameter":"reasoningEffort", "effortMode":"deepSeek",
        })
    }

    #[test]
    fn current_configuration_round_trips_with_independent_output() {
        let value = configuration();
        let parsed: CodexChatReasoning = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);
    }

    #[test]
    fn old_thinking_enum_and_missing_output_field_are_rejected() {
        let mut old_enum = configuration();
        old_enum["thinkingParameter"] = json!("reasoningSplit");
        assert!(serde_json::from_value::<CodexChatReasoning>(old_enum).is_err());
        let mut missing_field = configuration();
        missing_field.as_object_mut().unwrap().remove("separateOutput");
        assert!(serde_json::from_value::<CodexChatReasoning>(missing_field).is_err());
    }
}
