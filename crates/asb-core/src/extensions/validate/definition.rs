use crate::contracts::AppKind;
use crate::extensions::contracts::{
    ExtensionDefinition, ExtensionPayload, SkillDependency, SkillManifest,
    EXTENSIONS_SCHEMA_VERSION,
};

use crate::extensions::validate::binding::validate_id;
use crate::extensions::validate::error::reject;
use crate::extensions::validate::error::ExtensionValidationError;
use crate::extensions::validate::naming::{validate_mcp_definition, validate_skill_name};

/// Validates one library definition (any revision) end to end.
pub fn validate_definition(
    definition: &ExtensionDefinition,
) -> Result<(), ExtensionValidationError> {
    if definition.schema_version != EXTENSIONS_SCHEMA_VERSION {
        return reject(
            "schemaVersion",
            format!(
                "扩展定义版本 {} 不是当前支持的版本 {}",
                definition.schema_version, EXTENSIONS_SCHEMA_VERSION
            ),
        );
    }
    validate_id(&definition.id, "id")?;
    if definition.name.trim().is_empty() {
        return reject("name", "扩展名称不能为空");
    }
    if definition.revision == 0 {
        return reject("revision", "扩展版本号必须从 1 开始");
    }
    if definition.mcp_config_revision.is_some_and(|revision| {
        revision == 0 || revision >= definition.revision
            || !matches!(definition.payload, ExtensionPayload::Mcp(_))
    }) {
        return reject("mcpConfigRevision", "MCP 配置修订必须早于当前定义修订");
    }
    if definition.created_at.trim().is_empty() || definition.updated_at.trim().is_empty() {
        return reject("createdAt", "扩展时间戳不能为空");
    }
    if let Some(metadata) = &definition.mcp_metadata {
        if !matches!(definition.payload, ExtensionPayload::Mcp(_)) {
            return reject("mcpMetadata", "MCP 元信息只能用于 MCP 定义");
        }
        super::metadata::validate_mcp_metadata(metadata)?;
    }
    match &definition.payload {
        ExtensionPayload::Skill(skill) => {
            validate_skill_definition(&skill.manifest, skill.host_scoped, &skill.dependencies)
        }
        ExtensionPayload::Mcp(mcp) => validate_mcp_definition(mcp),
    }
}

/// Validates a definition draft that has no id/revision yet (library input).
pub fn validate_definition_draft(
    name: &str,
    payload: &ExtensionPayload,
) -> Result<(), ExtensionValidationError> {
    if name.trim().is_empty() {
        return reject("name", "扩展名称不能为空");
    }
    match payload {
        ExtensionPayload::Skill(skill) => {
            validate_skill_definition(&skill.manifest, skill.host_scoped, &skill.dependencies)
        }
        ExtensionPayload::Mcp(mcp) => validate_mcp_definition(mcp),
    }
}

fn validate_skill_definition(
    manifest: &SkillManifest,
    host_scoped: Option<AppKind>,
    dependencies: &[SkillDependency],
) -> Result<(), ExtensionValidationError> {
    validate_skill_name(&manifest.name)?;
    // Host-scoped imports follow the host's own rules and may omit fields the
    // portable specification requires; portable content may not.
    if host_scoped.is_none()
        && manifest
            .description
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        return reject(
            "description",
            "通用 Skill 必须提供 description；宿主专用内容请先标记来源客户端",
        );
    }
    for dependency in dependencies {
        if dependency.name.trim().is_empty() {
            return reject("dependencies", "依赖名称不能为空");
        }
        if let Some(resource_id) = &dependency.resource_id {
            validate_id(resource_id, "dependencies").map_err(|error| ExtensionValidationError {
                field: "dependencies",
                message: format!("依赖 {} 的关联定义无效：{}", dependency.name, error.message),
            })?;
        }
    }
    Ok(())
}
