use super::*;

fn activation(state: &LocalState, gateway: &GatewayController, record: &ProviderRecord) -> PreparedProfileSave {
    let draft = draft(&record.profile.name);
    let (kind, preview, projection_profile, candidate_revision) = prepare_profile_save_data(
        state, gateway, Some(&record.profile.id), &draft, Some(&record.file_hash), ProfileSaveIntent::Activate,
    ).unwrap();
    PreparedProfileSave { intent: ProfileSaveIntent::Activate, created_at: Instant::now(),
        profile_id: Some(record.profile.id.clone()), expected_file_hash: Some(record.file_hash.clone()),
        draft, kind, preview, projection_profile, candidate_revision }
}

#[test]
fn unchanged_inactive_profile_still_previews_and_activates() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, _) = providers(&state, &gateway);
    let prepared = activation(&state, &gateway, &target);
    assert_eq!(prepared.kind, ProfileSaveKind::SaveAndApply);
    assert!(prepared.preview.is_some());
    assert!(commit_prepared_profile_save(&state, &gateway, &prepared, false).is_err());
    commit_prepared_profile_save(&state, &gateway, &prepared, true).unwrap();
    assert_eq!(gateway.active_profile_id_for(AppKind::Claude), Some(target.profile.id));
}

#[test]
fn stale_client_preview_rolls_back_provider_and_keeps_previous_owner() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, primary) = providers(&state, &gateway);
    let mut changed = draft(&target.profile.name);
    changed.model = Some("changed-model".into());
    let (kind, preview, projection_profile, candidate_revision) = prepare_profile_save_data(
        &state, &gateway, Some(&target.profile.id), &changed, Some(&target.file_hash), ProfileSaveIntent::Activate,
    ).unwrap();
    let prepared = PreparedProfileSave { intent: ProfileSaveIntent::Activate, created_at: Instant::now(),
        profile_id: Some(target.profile.id.clone()), expected_file_hash: Some(target.file_hash.clone()),
        draft: changed, kind, preview, projection_profile, candidate_revision };
    std::fs::write(state.target(AppKind::Claude).unwrap(), "{\"external\":true}").unwrap();
    assert!(commit_prepared_profile_save(&state, &gateway, &prepared, true).is_err());
    assert_eq!(state.configuration().find_provider_record(&target.profile.id).unwrap().file_hash, target.file_hash);
    assert_eq!(gateway.active_profile_id_for(AppKind::Claude), Some(primary.profile.id));
}

#[test]
fn activation_cannot_create_an_unversioned_profile() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    assert!(prepare_profile_save_data(&state, &gateway, None, &draft("New"), None, ProfileSaveIntent::Activate).is_err());
}

#[test]
fn cancelled_preparation_cannot_be_taken_and_does_not_write_configuration() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, primary) = providers(&state, &gateway);
    let path = state.target(AppKind::Claude).unwrap();
    let before = std::fs::read(&path).unwrap();
    let preparations = ProfileSavePreparations::default();
    let id = preparations.issue(activation(&state, &gateway, &target)).unwrap();
    preparations.cancel(&id).unwrap();
    preparations.cancel(&id).unwrap();
    assert!(preparations.take(&id).is_err());
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert_eq!(state.configuration().find_provider_record(&target.profile.id).unwrap().file_hash, target.file_hash);
    assert_eq!(gateway.active_profile_id_for(AppKind::Claude), Some(primary.profile.id));
}
