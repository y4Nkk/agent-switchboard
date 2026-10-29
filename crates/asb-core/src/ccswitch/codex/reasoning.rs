//! Explicit CC metadata is translated at import, never guessed at request time.
use crate::contracts::{
    CodexChatEffortMode as Mode, CodexChatEffortParameter as Effort, CodexChatReasoning,
    CodexChatThinkingParameter as Thinking,
};
use serde_json::Value;

pub(super) fn parse(value: Option<&Value>) -> Result<CodexChatReasoning, String> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(CodexChatReasoning::Unsupported);
    };
    let object = value
        .as_object()
        .ok_or("meta.codexChatReasoning 必须是对象")?;
    let enabled = |name: &str| -> Result<bool, String> {
        object
            .get(name)
            .map(|value| {
                value
                    .as_bool()
                    .ok_or_else(|| format!("meta.codexChatReasoning.{name} 必须是布尔值"))
            })
            .transpose()
            .map(|v| v.unwrap_or(false))
    };
    let text = |name: &str| -> Result<&str, String> {
        object
            .get(name)
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| format!("meta.codexChatReasoning.{name} 必须是字符串"))
            })
            .transpose()
            .map(|v| v.unwrap_or("none"))
    };
    let (thinking_parameter, separate_output) = if !enabled("supportsThinking")? {
        (Thinking::None, false)
    } else {
        match text("thinkingParam")? {
            "none" => (Thinking::None, false),
            "thinking" => (Thinking::Thinking, false),
            "enable_thinking" => (Thinking::EnableThinking, false),
            "reasoning_split" => (Thinking::None, true),
            _ => return Err("meta.codexChatReasoning.thinkingParam 不受支持".into()),
        }
    };
    let effort_parameter = if !enabled("supportsEffort")? {
        Effort::None
    } else {
        match text("effortParam")? {
            "none" => Effort::None,
            "reasoning_effort" => Effort::ReasoningEffort,
            "reasoning.effort" => Effort::ReasoningObject,
            _ => return Err("meta.codexChatReasoning.effortParam 不受支持".into()),
        }
    };
    let effort_mode = match text("effortValueMode")? {
        "none" | "passthrough" => Mode::Passthrough,
        "low_high" => Mode::LowHigh,
        "deepseek" => Mode::DeepSeek,
        "openrouter" => Mode::OpenRouter,
        "zen" => Mode::Catalog,
        _ => return Err("meta.codexChatReasoning.effortValueMode 不受支持".into()),
    };
    Ok(CodexChatReasoning::Configured {
        thinking_parameter,
        separate_output,
        effort_parameter,
        effort_mode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn external_split_metadata_imports_as_output_without_a_thinking_toggle() {
        let source = json!({"supportsThinking":true,"thinkingParam":"reasoning_split"});
        let parsed = parse(Some(&source)).unwrap();
        assert_eq!(parsed, CodexChatReasoning::Configured {
            thinking_parameter: Thinking::None,
            separate_output: true,
            effort_parameter: Effort::None,
            effort_mode: Mode::Passthrough,
        });
    }
}
