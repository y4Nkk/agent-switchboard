use serde::{Deserialize, Serialize};

use crate::extensions::contracts::{
    DesiredState, ExtensionKind, ExtensionTarget, McpDefinition, SkillDefinition,
};

/// The discriminated payload of an [`ExtensionDefinition`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ExtensionPayload {
    Skill(SkillDefinition),
    Mcp(McpDefinition),
}

/// Optional MCP library presentation data, never part of a client config.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<McpRegistrySource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpRegistrySource {
    pub name: String,
    pub version: String,
    pub endpoint: String,
}

/// One library definition, persisted as `extensions/definitions/<id>.json`.
/// The outer envelope cannot combine `deny_unknown_fields` with the
/// flattened payload union; strictness lives at the payload variants, and
/// an unknown `kind` still fails the union.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionDefinition {
    pub schema_version: u8,
    pub id: String,
    /// Skill library name or MCP native server key; never the resource id.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_metadata: Option<McpMetadata>,
    /// Monotonic per-definition revision for optimistic concurrency.
    pub revision: u64,
    /// Revision of the current MCP name and connection, present only when
    /// later edits affected library metadata alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_config_revision: Option<u64>,
    /// RFC 3339 UTC timestamps.
    pub created_at: String,
    pub updated_at: String,
    #[serde(flatten)]
    pub payload: ExtensionPayload,
}

impl ExtensionDefinition {
    pub fn mcp_config_revision(&self) -> u64 {
        self.mcp_config_revision.unwrap_or(self.revision)
    }

    pub fn kind(&self) -> ExtensionKind {
        match self.payload {
            ExtensionPayload::Skill(_) => ExtensionKind::Skill,
            ExtensionPayload::Mcp(_) => ExtensionKind::Mcp,
        }
    }
}

/// One deployment intent persisted as `extensions/bindings/<id>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionBinding {
    pub schema_version: u8,
    pub id: String,
    pub resource_id: String,
    pub target: ExtensionTarget,
    /// Native MCP server key; MCP bindings only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_key: Option<String>,
    /// Deployed directory name under the client's skill root; skill only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deploy_name: Option<String>,
    pub desired: DesiredState,
    /// Pin the binding to one content version; absent means the binding
    /// follows the definition's current digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked_digest: Option<String>,
    /// Definition revision covered by the last content deployment. Skips,
    /// visibility-only changes, and deployment of pinned older bytes keep it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_applied_revision: Option<u64>,
    pub updated_at: String,
}
