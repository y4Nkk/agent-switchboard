use super::*;
use asb_core::extensions::contracts::{CodexServerOptions, ExtensionPayload, ProjectRegistration, EXTENSIONS_SCHEMA_VERSION};
use std::collections::BTreeMap;

#[test]
fn project_context_and_codex_cwd_have_distinct_client_semantics() {
    let temp = tempfile::tempdir().unwrap();
    let store = ExtensionStore::from_root(temp.path().join("library"));
    let root = temp.path().join("project");
    std::fs::create_dir_all(root.join("server")).unwrap();
    store.create_project(&ProjectRegistration {
        schema_version: EXTENSIONS_SCHEMA_VERSION, id: "project".into(), root: root.to_string_lossy().into(),
        display_name: "fixture".into(), registered_at: "2026-10-13T00:00:00Z".into(),
    }).unwrap();
    let mcp = McpDefinition::Stdio {
        command: "node".into(), args: Vec::new(), env: BTreeMap::new(),
        codex_options: Some(CodexServerOptions { cwd: Some("server".into()), ..Default::default() }),
    };
    let definition = ExtensionDefinition {
        mcp_config_revision: None,
        schema_version: EXTENSIONS_SCHEMA_VERSION, id: "fixture".into(), name: "fixture".into(), revision: 3,
        mcp_metadata: None, created_at: "2026-10-13T00:00:00Z".into(), updated_at: "2026-10-13T00:00:00Z".into(),
        payload: ExtensionPayload::Mcp(mcp.clone()),
    };
    let target = |client| ExtensionTarget::ProjectShared { client, project_id: "project".into() };
    let codex = request_for(&store, &definition, &mcp, target(AppKind::Codex), 3).unwrap();
    let claude = request_for(&store, &definition, &mcp, target(AppKind::Claude), 3).unwrap();
    assert_eq!(codex.working_dir, root.join("server"));
    assert_eq!(claude.working_dir, root);
    assert!(request_for(&store, &definition, &mcp, target(AppKind::Codex), 2).is_err());
    assert!(request_for(&store, &definition, &mcp,
        ExtensionTarget::ProjectPrivate { client: AppKind::Codex, project_id: "project".into() }, 3).is_err());
}
