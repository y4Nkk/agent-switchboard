use super::*;

#[test]
fn new_claude_fixed_self_route_uses_the_session_identity() {
    use asb_core::contracts::{ClaudeModelRoute, ClaudeModelRoutes, ClaudeModelSettings, ModelOptions};
    let directory = tempfile::tempdir().unwrap();
    let store = crate::config_store::ConfigStore::new(directory.path().join("state"));
    let id = uuid::Uuid::new_v4().to_string();
    let mut input = draft("Self target");
    input.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
        routes: ClaudeModelRoutes {
            subagent: Some(ClaudeModelRoute::Provider {
                profile_id: id.clone(), model: "fixture-model".into(), one_m: false,
            }), ..Default::default()
        }, ..Default::default()
    }));
    let created = store.create_provider(id.clone(), input).unwrap();
    assert_eq!(created.profile.id, id);
    let Some(ModelOptions::Claude(models)) = created.profile.model_options else { panic!("missing models") };
    assert_eq!(models.routes.subagent.unwrap().profile_id(), Some(id.as_str()));
    assert_eq!(store.list_providers().unwrap().len(), 1);
}

fn saved_plan(state: &LocalState, gateway: &GatewayController, record: &ProviderRecord,
    draft: ProviderDraft) -> PreparedProfileSave {
    let target = ProfileSaveTarget::Update {
        profile_id: record.profile.id.clone(), expected_file_hash: record.file_hash.clone(),
    };
    let (kind, preview, projection_profile, candidate_revision) = prepare_profile_save_data(
        state, gateway, &target, &draft,
    ).unwrap();
    PreparedProfileSave { created_at: Instant::now(), target, draft, kind, preview,
        projection_profile, candidate_revision }
}

#[test]
fn saving_unreferenced_inactive_profile_never_activates_it() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, primary) = providers(&state, &gateway);
    crate::gateway::failover::save(state.root(), &Default::default()).unwrap();
    let path = state.target(AppKind::Claude).unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut changed = draft(&target.profile.name);
    changed.model = Some("changed-model".into());
    let prepared = saved_plan(&state, &gateway, &target, changed);
    assert_eq!(prepared.kind, ProfileSaveKind::SaveOnly);
    assert!(prepared.preview.is_none());
    commit_prepared_profile_save(&state, &gateway, &prepared, false).unwrap();
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert_eq!(gateway.active_profile_id_for(AppKind::Claude), Some(primary.profile.id));
}

#[test]
fn active_route_save_requires_confirmation_and_rejects_stale_client() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (_, primary) = providers(&state, &gateway);
    let mut changed = draft(&primary.profile.name);
    changed.model = Some("changed-model".into());
    let prepared = saved_plan(&state, &gateway, &primary, changed);
    assert_eq!(prepared.kind, ProfileSaveKind::SaveAndApply);
    assert!(commit_prepared_profile_save(&state, &gateway, &prepared, false).is_err());
    std::fs::write(state.target(AppKind::Claude).unwrap(), "{\"external\":true}").unwrap();
    assert!(commit_prepared_profile_save(&state, &gateway, &prepared, true).is_err());
    assert_eq!(state.configuration().find_provider_record(&primary.profile.id).unwrap().file_hash, primary.file_hash);
    assert_eq!(gateway.active_profile_id_for(AppKind::Claude), Some(primary.profile.id));
}

#[test]
fn cancelled_save_preparation_is_not_consumable() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, _) = providers(&state, &gateway);
    let preparations = ProfileSavePreparations::default();
    let id = preparations.issue(saved_plan(&state, &gateway, &target, draft(&target.profile.name))).unwrap();
    preparations.cancel(&id).unwrap();
    assert!(preparations.take(&id).is_err());
}
