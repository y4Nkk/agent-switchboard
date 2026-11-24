use super::*;
use asb_core::contracts::AppKind;
use asb_core::extensions::contracts::{
    DesiredState, ExtensionTarget, McpDefinition, McpMetadata, McpRegistrySource, SkillDefinition,
    SkillManifest, EXTENSIONS_SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};

use crate::extensions::store::ExtensionStore;

#[test]
fn unpinning_an_older_digest_is_pending_even_if_a_previous_skip_recorded_the_latest_revision() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("deployed");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("SKILL.md"), "---\nname: fixture\ndescription: fixture\n---\nold\n").unwrap();
    let old_digest = content_digest(&walk_skill_dir(&target).unwrap());
    let mut binding = ExtensionBinding {
        schema_version: EXTENSIONS_SCHEMA_VERSION, id: "binding".into(), resource_id: "fixture".into(),
        target: ExtensionTarget::App { client: AppKind::Codex }, native_key: None, deploy_name: Some("fixture".into()),
        desired: DesiredState::Enabled, locked_digest: Some(old_digest.clone()), last_applied_revision: Some(2),
        updated_at: "2026-10-13T00:00:00Z".into(),
    };
    let baseline = ManagedBaselineFile::new(&binding.id, vec![ManagedBaseline::Directory {
        target_dir: target.to_string_lossy().into(), original_existed: false, original_files: None,
        original_digest: None, last_files: Vec::new(), last_digest: old_digest,
        backup_reference: None,
    }]);
    let definition = ExtensionDefinition {
        mcp_config_revision: None,
        schema_version: EXTENSIONS_SCHEMA_VERSION, id: "fixture".into(), name: "fixture".into(), revision: 2,
        mcp_metadata: None, created_at: binding.updated_at.clone(), updated_at: binding.updated_at.clone(),
        payload: ExtensionPayload::Skill(SkillDefinition {
            content_digest: "a".repeat(64), manifest: SkillManifest {
                name: "fixture".into(), description: Some("fixture".into()), license: None,
                allowed_tools: None, unparsed_keys: Vec::new(),
            }, source: None, host_scoped: None, compatibility: Vec::new(), dependencies: Vec::new(),
        }),
    };
    assert_eq!(binding_file_state(&binding, &definition, Some(&baseline), &[]), FileState::InSync);
    binding.locked_digest = None;
    assert_eq!(binding_file_state(&binding, &definition, Some(&baseline), &[]), FileState::PendingApply);
    std::fs::write(target.join("SKILL.md"), "external edit").unwrap();
    assert_eq!(binding_file_state(&binding, &definition, Some(&baseline), &[]), FileState::ExternalChange);
}

#[test]
fn mcp_registry_association_preserves_deployed_and_pending_connections() {
    let temp = tempfile::tempdir().unwrap();
    let store = ExtensionStore::from_root(temp.path().join("library"));
    let target = temp.path().join("config.toml");
    let endpoint = "https://example.test/mcp";
    let document = format!("[mcp_servers.fixture]\nurl = \"{endpoint}\"\n");
    std::fs::write(&target, &document).unwrap();
    let timestamp = "2026-11-23T00:00:00Z".to_string();
    let mut binding = ExtensionBinding {
        schema_version: EXTENSIONS_SCHEMA_VERSION, id: "binding".into(), resource_id: "fixture".into(),
        target: ExtensionTarget::App { client: AppKind::Codex }, native_key: Some("fixture".into()),
        deploy_name: None, desired: DesiredState::Enabled, locked_digest: None,
        last_applied_revision: Some(1), updated_at: timestamp.clone(),
    };
    let baseline = ManagedBaselineFile::new(&binding.id, vec![ManagedBaseline::DocumentEntry {
        target_path: target.to_string_lossy().into(), entry_pointer: "mcp_servers.fixture".into(),
        original_value: None, target_existed_before: true, backup_reference: None,
        last_written_value: asb_core::extensions::mcp::managed_entry_text(&binding, &document, &[]).unwrap(),
        last_document_hash: format!("{:x}", Sha256::digest(document.as_bytes())),
    }]);
    let registry_metadata = |version: &str, endpoint: &str| McpMetadata {
        registry: Some(McpRegistrySource {
            name: "com.example/service".into(), version: version.into(), endpoint: endpoint.into(),
        }),
        ..McpMetadata::default()
    };
    let mut definition = ExtensionDefinition {
        schema_version: EXTENSIONS_SCHEMA_VERSION, id: "fixture".into(), name: "fixture".into(),
        revision: 1, mcp_config_revision: None, mcp_metadata: Some(registry_metadata("1.0", endpoint)),
        created_at: timestamp.clone(), updated_at: timestamp,
        payload: ExtensionPayload::Mcp(McpDefinition::Http {
            url: endpoint.into(), headers: Default::default(), bearer: None,
        }),
    };
    store.create_definition(&definition).unwrap();
    let associate = |definition: &mut ExtensionDefinition, version: &str, endpoint: &str| {
        definition.mcp_metadata = Some(registry_metadata(version, endpoint));
        definition.revision += 1;
        store.update_definition(definition, definition.revision - 1).unwrap();
        *definition = store.get_definition(&definition.id).unwrap().unwrap();
    };
    associate(&mut definition, "2.0", endpoint);
    associate(&mut definition, "3.0", endpoint);
    assert_eq!(definition.mcp_config_revision, Some(1));
    assert_eq!(binding_file_state(&binding, &definition, Some(&baseline), &[]), FileState::InSync);
    binding.last_applied_revision = Some(2);
    assert_eq!(binding_file_state(&binding, &definition, Some(&baseline), &[]), FileState::InSync);

    let next_endpoint = "https://example.test/next";
    let ExtensionPayload::Mcp(McpDefinition::Http { url, .. }) = &mut definition.payload else { unreachable!() };
    *url = next_endpoint.into();
    definition.revision = 4;
    store.update_definition(&definition, 3).unwrap();
    definition = store.get_definition(&definition.id).unwrap().unwrap();
    assert_eq!(definition.mcp_config_revision, None);
    associate(&mut definition, "4.0", next_endpoint);
    assert_eq!(definition.mcp_config_revision, Some(4));
    assert_eq!(binding_file_state(&binding, &definition, Some(&baseline), &[]), FileState::PendingApply);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), document);
    std::fs::write(&target, "[mcp_servers.fixture]\nurl = \"https://example.test/external\"\n").unwrap();
    assert_eq!(binding_file_state(&binding, &definition, Some(&baseline), &[]), FileState::ExternalChange);
}
