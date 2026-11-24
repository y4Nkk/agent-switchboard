use super::*;
use asb_core::contracts::AppKind;
use asb_core::extensions::contracts::{
    DesiredState, ExtensionDefinition, ExtensionPayload, ExtensionTarget, McpDefinition,
    ProjectRegistration, EXTENSIONS_SCHEMA_VERSION,
};
use asb_core::extensions::plan::PlanStep;
use crate::extensions::{discovery::DiscoveredPaths, store::ExtensionStore};

fn definition(id: &str) -> ExtensionDefinition {
    ExtensionDefinition {
        mcp_config_revision: None,
        schema_version: EXTENSIONS_SCHEMA_VERSION, id: id.into(), name: id.into(), revision: 1,
        mcp_metadata: None, created_at: "2026-10-13T00:00:00Z".into(), updated_at: "2026-10-13T00:00:00Z".into(),
        payload: ExtensionPayload::Mcp(McpDefinition::Stdio {
            command: "node".into(), args: Vec::new(), env: BTreeMap::new(), codex_options: None,
        }),
    }
}

fn install(id: &str, target: ExtensionTarget) -> PlanRequestOperation {
    PlanRequestOperation { operation: PlanOperation::Install, definition_id: Some(id.into()),
        binding_id: None, targets: vec![target], shared_settings: None }
}

#[test]
fn grouped_multi_scope_installs_keep_one_final_write_per_document() {
    let temp = tempfile::tempdir().unwrap();
    let store = ExtensionStore::from_root(temp.path().join("library"));
    for id in ["server-a", "server-b"] { store.create_definition(&definition(id)).unwrap(); }
    let project = temp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    store.create_project(&ProjectRegistration {
        schema_version: EXTENSIONS_SCHEMA_VERSION, id: "project".into(), root: project.to_string_lossy().into(),
        display_name: "fixture".into(), registered_at: "2026-10-13T00:00:00Z".into(),
    }).unwrap();
    let paths = DiscoveredPaths { home: temp.path().join("home"), codex_home: None, claude_dir: None };
    let planner = Planner { store: &store, paths: &paths, secrets: &|_| None };
    let app = ExtensionTarget::App { client: AppKind::Codex };
    let project = ExtensionTarget::ProjectShared { client: AppKind::Codex, project_id: "project".into() };
    let request = PlanRequest { operations: vec![
        install("server-a", app.clone()), install("server-b", app),
        install("server-a", project.clone()), install("server-b", project),
    ] };
    let plan = planner.build(&request).unwrap();
    assert_eq!(plan.operations.len(), 2);
    assert!(plan.operations.iter().all(|operation| operation.targets.len() == 2));
    let writes = plan.planned_targets().flat_map(|target| &target.steps).collect::<Vec<_>>();
    assert_eq!(writes.len(), 2);
    for step in writes {
        let PlanStep::DocumentWrite { rendered, .. } = step else { panic!("unexpected step") };
        assert!(rendered.contains("server-a") && rendered.contains("server-b"));
    }
    assert!(!paths.home.exists());
}

#[test]
fn distinct_bindings_of_one_definition_group_but_repeated_binding_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("library");
    std::fs::create_dir_all(root.join("bindings")).unwrap();
    let store = ExtensionStore::from_root(root.clone());
    for (id, client) in [("codex", AppKind::Codex), ("claude", AppKind::Claude)] {
        let binding = ExtensionBinding {
            schema_version: EXTENSIONS_SCHEMA_VERSION, id: id.into(), resource_id: "server".into(),
            target: ExtensionTarget::App { client }, native_key: Some("server".into()), deploy_name: None,
            desired: DesiredState::Enabled, locked_digest: None, last_applied_revision: Some(1),
            updated_at: "2026-10-13T00:00:00Z".into(),
        };
        std::fs::write(root.join("bindings").join(format!("{id}.json")), serde_json::to_vec(&binding).unwrap()).unwrap();
    }
    let paths = DiscoveredPaths { home: temp.path().join("home"), codex_home: None, claude_dir: None };
    let planner = Planner { store: &store, paths: &paths, secrets: &|_| None };
    let removal = |id: &str| PlanRequestOperation {
        operation: PlanOperation::Remove, definition_id: None, binding_id: Some(id.into()),
        targets: Vec::new(), shared_settings: None,
    };
    let request = PlanRequest { operations: vec![removal("codex"), removal("claude")] };
    let groups = planner.group_requests(&request).unwrap();
    assert_eq!((groups.len(), groups[0].len()), (1, 2));
    let repeated = PlanRequest { operations: vec![removal("codex"), removal("codex")] };
    assert!(planner.group_requests(&repeated).is_err());
}
