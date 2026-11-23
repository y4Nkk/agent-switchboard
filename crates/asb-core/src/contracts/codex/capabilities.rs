use super::{CodexChatReasoning, CodexReasoningLevel};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The operations which may be exposed on the fixed local Codex endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexOperation {
    Responses,
    Compact,
    Models,
    ChatCompletions,
    AlphaSearch,
    ImageGeneration,
    ImageEdit,
}

impl CodexOperation {
    pub fn from_path(path: &str) -> Option<Self> {
        [Self::Responses, Self::Compact, Self::Models, Self::ChatCompletions,
            Self::AlphaSearch, Self::ImageGeneration, Self::ImageEdit]
            .into_iter().find(|operation| operation.upstream_path() == path)
    }

    pub const fn is_responses(self) -> bool {
        matches!(self, Self::Responses)
    }

    pub const fn is_compact(self) -> bool {
        matches!(self, Self::Compact)
    }

    pub const fn is_model_list(self) -> bool {
        matches!(self, Self::Models)
    }

    pub const fn upstream_path(self) -> &'static str {
        match self {
            Self::Responses => "/responses",
            Self::Compact => "/responses/compact",
            Self::Models => "/models",
            Self::ChatCompletions => "/chat/completions",
            Self::AlphaSearch => "/alpha/search",
            Self::ImageGeneration => "/images/generations",
            Self::ImageEdit => "/images/edits",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexAutomaticOperation {
    Models,
    ChatCompletions,
    AlphaSearch,
    ImageGeneration,
    ImageEdit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexProbedOperation {
    AlphaSearch,
    ImageGeneration,
    ImageEdit,
}

impl From<CodexProbedOperation> for CodexOperation {
    fn from(value: CodexProbedOperation) -> Self {
        match value {
            CodexProbedOperation::AlphaSearch => Self::AlphaSearch,
            CodexProbedOperation::ImageGeneration => Self::ImageGeneration,
            CodexProbedOperation::ImageEdit => Self::ImageEdit,
        }
    }
}

/// Effective provider capabilities. Automatic operations record which values
/// the editor can update when the connection or its discovery results change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexCapabilities {
    pub responses: bool,
    pub compact: bool,
    pub models: bool,
    pub chat_completions: bool,
    pub alpha_search: bool,
    pub image_generation: bool,
    pub image_edit: bool,
    pub automatic_operations: BTreeSet<CodexAutomaticOperation>,
    pub function_tools: bool,
    pub custom_tools: bool,
    pub tool_search: bool,
    pub reasoning: bool,
    pub chat_reasoning: CodexChatReasoning,
}

/// The full reasoning ladder a generated catalog row declares when neither
/// the source nor the user narrows it. The editor mirrors this order in
/// `REASONING_LEVELS` (src/components/codex-provider-editor/draft.ts).
pub const CODEX_REASONING_LADDER: [CodexReasoningLevel; 8] = [
    CodexReasoningLevel::None,
    CodexReasoningLevel::Minimal,
    CodexReasoningLevel::Low,
    CodexReasoningLevel::Medium,
    CodexReasoningLevel::High,
    CodexReasoningLevel::Xhigh,
    CodexReasoningLevel::Max,
    CodexReasoningLevel::Ultra,
];

impl CodexCapabilities {
    pub const fn supports_operation(&self, operation: CodexOperation) -> bool {
        match operation {
            CodexOperation::Responses => self.responses,
            CodexOperation::Compact => self.compact,
            CodexOperation::Models => self.models,
            CodexOperation::ChatCompletions => self.chat_completions,
            CodexOperation::AlphaSearch => self.alpha_search,
            CodexOperation::ImageGeneration => self.image_generation,
            CodexOperation::ImageEdit => self.image_edit,
        }
    }
}

/// Capabilities used by generated providers and one-click imports.
pub const DEFAULT_CODEX_CAPABILITIES: CodexCapabilities = CodexCapabilities {
    responses: true,
    compact: true,
    models: true,
    chat_completions: false,
    alpha_search: false,
    image_generation: false,
    image_edit: false,
    automatic_operations: BTreeSet::new(),
    function_tools: true,
    custom_tools: true,
    tool_search: true,
    reasoning: true,
    chat_reasoning: CodexChatReasoning::Unsupported,
};
