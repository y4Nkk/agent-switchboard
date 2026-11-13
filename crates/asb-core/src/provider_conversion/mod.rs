//! Read-only conversion into independent, target-owned editing seeds.

mod codex_seed;
mod connection;
mod convert;
#[cfg(test)]
mod tests;

pub use codex_seed::{CodexConversionDraft, ConversionCatalogEntry};
pub use convert::{from_codex, from_provider};
use crate::contracts::{AppKind, LocalizedMessage, ProviderDraft, UsageQuery};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderConversionRequest {
    pub source_app: AppKind,
    pub source_profile_id: String,
    pub expected_source_file_hash: String,
    pub target_app: AppKind,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionSource {
    pub app: AppKind,
    pub id: String,
    pub name: String,
    pub file_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConversionStatus { Ready, NeedsInput, Blocked }

#[derive(Clone, PartialEq, Serialize)]
#[serde(tag = "app", rename_all = "lowercase")]
pub enum ProviderConversionSeed {
    Claude { draft: ProviderDraft },
    Codex { draft: CodexConversionDraft },
}

#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConversion {
    pub source: ConversionSource,
    pub target_app: AppKind,
    pub status: ConversionStatus,
    pub blocked_reason: Option<LocalizedMessage>,
    pub notices: Vec<LocalizedMessage>,
    pub seed: Option<ProviderConversionSeed>,
    pub usage_query: Option<UsageQuery>,
}

pub(super) fn message(key: &str, text: &str) -> LocalizedMessage {
    LocalizedMessage::new(format!("providers.conversion.{key}"), serde_json::Value::Null, text)
}

pub(super) fn blocked(source: ConversionSource, target: AppKind, key: &str, text: &str) -> ProviderConversion {
    ProviderConversion {
        source, target_app: target, status: ConversionStatus::Blocked,
        blocked_reason: Some(message(key, text)), notices: Vec::new(), seed: None, usage_query: None,
    }
}

pub(super) fn manual_query(query: &Option<UsageQuery>) -> Option<UsageQuery> {
    query.clone().map(|mut query| {
        match &mut query {
            UsageQuery::Declarative { refresh_interval_minutes, .. }
            | UsageQuery::Script { refresh_interval_minutes, .. } => *refresh_interval_minutes = 0,
        }
        query
    })
}
