use super::*;
use asb_core::{contracts::AppKind, extensions::contracts::{ExtensionPayload, ExtensionTarget, McpCheckOutcome, McpDefinition}};

fn definition() -> ExtensionDefinition {
    ExtensionDefinition {
        mcp_config_revision: None,
        schema_version: EXTENSIONS_SCHEMA_VERSION, id: "fixture".into(), name: "fixture".into(), revision: 1,
        mcp_metadata: None, created_at: "2026-10-13T00:00:00Z".into(), updated_at: "2026-10-13T00:00:00Z".into(),
        payload: ExtensionPayload::Mcp(McpDefinition::Http {
            url: "https://example.test/mcp".into(), headers: std::collections::BTreeMap::new(), bearer: None,
        }),
    }
}

fn result(project: usize, revision: u64) -> McpCheckResult {
    McpCheckResult {
        definition_id: "fixture".into(), definition_revision: revision,
        target: ExtensionTarget::ProjectShared { client: AppKind::Codex, project_id: format!("project-{project}") },
        checked_at: "2026-10-13T00:00:00Z".into(), protocol_version: Some("2025-11-25".into()),
        outcome: McpCheckOutcome::Passed { tools: 0, resources: 0, prompts: 0 }, duration_ms: 1, truncated: false,
    }
}

#[test]
fn keeps_a_result_for_every_target_and_rejects_late_results_for_old_or_deleted_definitions() {
    let temp = tempfile::tempdir().unwrap();
    let store = ExtensionStore::from_root(temp.path().join("library"));
    let mut definition = definition();
    store.create_definition(&definition).unwrap();
    for project in 0..12 { assert!(store.save_check_if_current(&result(project, 1)).unwrap()); }
    assert_eq!(store.list_checks().unwrap().len(), 12);
    definition.revision = 2;
    store.update_definition(&definition, 1).unwrap();
    assert!(store.save_check_if_current(&result(0, 2)).unwrap());
    assert!(!store.save_check_if_current(&result(0, 1)).unwrap());
    assert_eq!(store.list_checks().unwrap().into_iter()
        .find(|check| check.target == result(0, 2).target).unwrap().definition_revision, 2);
    store.delete_definition_if_unbound(&definition.id).unwrap();
    assert!(!store.save_check_if_current(&result(0, 2)).unwrap());
}
