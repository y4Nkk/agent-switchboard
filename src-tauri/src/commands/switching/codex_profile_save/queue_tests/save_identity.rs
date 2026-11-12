use super::*;

#[test]
fn saving_unreferenced_inactive_codex_profile_preserves_active_identity() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, primary) = providers(&state, &gateway);
    crate::gateway::codex::policy::save(state.root(), &Default::default()).unwrap();
    let before = std::fs::read(state.target(AppKind::Codex).unwrap()).unwrap();
    let prepared = prepare(&state, &gateway, &target);
    assert_eq!(prepared.kind, ProfileSaveKind::SaveOnly);
    assert!(prepared.preview.is_none());
    commit(&state, &gateway, &prepared, false).unwrap();
    assert_eq!(std::fs::read(state.target(AppKind::Codex).unwrap()).unwrap(), before);
    assert_eq!(gateway.active_profile_id_for(AppKind::Codex), Some(primary.profile.id));
}

#[test]
fn cancelled_codex_save_preparation_is_not_consumable() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, _) = providers(&state, &gateway);
    let preparations = CodexProfileSavePreparations::default();
    let id = preparations.issue(prepare(&state, &gateway, &target)).unwrap();
    preparations.cancel(&id).unwrap();
    assert!(preparations.take(&id).is_err());
}
