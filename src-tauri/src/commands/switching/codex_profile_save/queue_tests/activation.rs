use super::*;

fn activation(state: &LocalState, gateway: &GatewayController, target: &CodexProviderRecord,
    route: Option<asb_core::contracts::CodexSubagentRoute>) -> PreparedCodexProfileSave {
    let mut draft = crate::codex_common::test_draft();
    draft.subagent_route = route;
    let (kind, preview, projection_profile, candidate_revision) = prepare_data(
        state, gateway, &target.profile.id, &draft, &target.file_hash, ProfileSaveIntent::Activate,
    ).unwrap();
    PreparedCodexProfileSave { intent: ProfileSaveIntent::Activate, created_at: Instant::now(),
        profile_id: target.profile.id.clone(), expected_file_hash: target.file_hash.clone(),
        draft, kind, preview, projection_profile, candidate_revision }
}

#[test]
fn unchanged_target_can_be_explicitly_activated_with_confirmation() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, _) = providers(&state, &gateway);
    let prepared = activation(&state, &gateway, &target, None);
    assert_eq!(prepared.kind, ProfileSaveKind::SaveAndApply);
    assert!(prepared.preview.is_some());
    assert!(commit(&state, &gateway, &prepared, false).is_err());
    commit(&state, &gateway, &prepared, true).unwrap();
    assert_eq!(gateway.active_profile_id_for(AppKind::Codex), Some(target.profile.id));
}

#[test]
fn stale_activation_does_not_change_the_selected_owner_or_provider() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, primary) = providers(&state, &gateway);
    let prepared = activation(&state, &gateway, &target, None);
    std::fs::write(state.target(AppKind::Codex).unwrap(), "model = \"externally-changed\"\n").unwrap();
    assert!(commit(&state, &gateway, &prepared, true).is_err());
    let stored = state.configuration().find_codex_provider_file(&target.profile.id).unwrap();
    assert_eq!(stored.profile.default_model, target.profile.default_model);
    assert_eq!(gateway.active_profile_id_for(AppKind::Codex), Some(primary.profile.id));
}

#[test]
fn activation_saves_the_route_and_projects_its_target_catalog_together() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, primary) = providers(&state, &gateway);
    let route = asb_core::contracts::CodexSubagentRoute {
        profile_id: primary.profile.id.clone(), model: primary.profile.default_model.clone(),
    };
    let wire = route.wire_id();
    let prepared = activation(&state, &gateway, &target, Some(route.clone()));
    let saved = commit(&state, &gateway, &prepared, true).unwrap();
    assert_eq!(saved.profile.subagent_route, Some(route));
    assert_eq!(gateway.active_profile_id_for(AppKind::Codex), Some(target.profile.id));
    let client = state.target(AppKind::Codex).unwrap();
    let content = std::fs::read_to_string(&client).unwrap();
    let config: toml_edit::DocumentMut = content.parse().unwrap();
    assert_eq!(config["agents"]["default_subagent_model"].as_str(), Some(wire.as_str()));
    let catalog_path = client.parent().unwrap().join(config["model_catalog_json"].as_str().unwrap());
    let catalog = std::fs::read(catalog_path).unwrap();
    let catalog: serde_json::Value = serde_json::from_slice(&catalog).unwrap();
    assert!(catalog["models"].as_array().unwrap().iter().any(|model| model["slug"] == wire));
    assert!(state.configuration().pending_profile_save().unwrap().is_none());
}

#[test]
fn cancelled_preparation_cannot_be_taken_and_preserves_the_active_route() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, primary) = providers(&state, &gateway);
    let path = state.target(AppKind::Codex).unwrap();
    let before = std::fs::read(&path).unwrap();
    let preparations = CodexProfileSavePreparations::default();
    let id = preparations.issue(activation(&state, &gateway, &target, None)).unwrap();
    preparations.cancel(&id).unwrap();
    preparations.cancel(&id).unwrap();
    assert!(preparations.take(&id).is_err());
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert_eq!(gateway.active_profile_id_for(AppKind::Codex), Some(primary.profile.id));
}
