use super::*;
use asb_core::contracts::AppKind;
use asb_core::extensions::contracts::{
    DesiredState, ExtensionTarget, McpDefinition, SkillDefinition, SkillManifest,
    EXTENSIONS_SCHEMA_VERSION,
};
use std::collections::BTreeMap;

fn fixture() -> (ExtensionDefinition, ExtensionBinding) {
    let definition = ExtensionDefinition {
        mcp_config_revision: None,
        schema_version: EXTENSIONS_SCHEMA_VERSION, id: "fixture".into(), name: "fixture".into(),
        revision: 2, mcp_metadata: None, created_at: "2026-10-13T00:00:00Z".into(),
        updated_at: "2026-10-13T00:00:00Z".into(),
        payload: ExtensionPayload::Skill(SkillDefinition {
            content_digest: "a".repeat(64), manifest: SkillManifest {
                name: "fixture".into(), description: Some("fixture".into()), license: None,
                allowed_tools: None, unparsed_keys: Vec::new(),
            },
            source: None, host_scoped: None, compatibility: Vec::new(), dependencies: Vec::new(),
        }),
    };
    let binding = ExtensionBinding {
        schema_version: EXTENSIONS_SCHEMA_VERSION, id: "binding".into(), resource_id: "fixture".into(),
        target: ExtensionTarget::App { client: AppKind::Codex }, native_key: None,
        deploy_name: Some("fixture".into()), desired: DesiredState::Enabled,
        locked_digest: Some("b".repeat(64)), last_applied_revision: Some(1),
        updated_at: definition.updated_at.clone(),
    };
    (definition, binding)
}

#[test]
fn deploying_pinned_older_content_preserves_the_covered_revision() {
    let (definition, mut binding) = fixture();
    assert_eq!(deployed_binding(&definition, &binding).last_applied_revision, Some(1));
    binding.locked_digest = Some("a".repeat(64));
    assert_eq!(deployed_binding(&definition, &binding).last_applied_revision, Some(2));
    binding.locked_digest = None;
    assert_eq!(deployed_binding(&definition, &binding).last_applied_revision, Some(2));
}

#[test]
fn rendering_current_mcp_content_covers_the_revision_even_when_disabled() {
    let (mut definition, mut binding) = fixture();
    definition.payload = ExtensionPayload::Mcp(McpDefinition::Stdio {
        command: "node".into(), args: Vec::new(), env: BTreeMap::new(), codex_options: None,
    });
    binding.locked_digest = None;
    binding.desired = DesiredState::Disabled;
    assert_eq!(deployed_binding(&definition, &binding).last_applied_revision, Some(2));
}
