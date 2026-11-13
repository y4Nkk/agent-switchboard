use super::*;
use asb_core::contracts::{ClaudeModelRoute, ClaudeModelRoutes, ClaudeModelSettings, ModelOptions};
use serde_json::json;

fn setup(state: &LocalState, gateway: &GatewayController) -> (ProviderRecord, ProviderRecord, ProviderDraft) {
    let target = state.configuration().create_provider(uuid::Uuid::new_v4().to_string(), draft("Target")).unwrap();
    let mut source = draft("Source");
    source.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
        routes: ClaudeModelRoutes { fable:Some(ClaudeModelRoute::Provider {
            profile_id:target.profile.id.clone(),model:"fixture-model".into(),one_m:false,
        }), ..Default::default() }, ..Default::default()
    }));
    let primary = state.configuration().create_provider(uuid::Uuid::new_v4().to_string(), source.clone()).unwrap();
    std::fs::write(state.target(AppKind::Claude).unwrap(), "{}").unwrap();
    let projection = super::super::super::plan::build_plan_for_profile(state, gateway, primary.profile.clone(), None).unwrap();
    let preview = super::super::super::plan::preview_projection(state, &projection).unwrap();
    execute_projection(state, gateway, &projection, &preview.content_hash, &preview.rendered_hash).unwrap();
    (target, primary, source)
}

fn prepared(state: &LocalState, gateway: &GatewayController, record: &ProviderRecord, draft: ProviderDraft) -> PreparedProfileSave {
    let target = ProfileSaveTarget::Update {
        profile_id: record.profile.id.clone(), expected_file_hash: record.file_hash.clone(),
    };
    let (kind, preview, projection_profile, candidate_revision) = prepare_profile_save_data(
        state, gateway, &target, &draft,
    ).unwrap();
    PreparedProfileSave { created_at:Instant::now(),target,draft,kind,preview,projection_profile,candidate_revision }
}

#[test]
fn saving_a_referenced_target_updates_its_owner_without_switching_primary_or_needing_failover() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state")); let gateway = GatewayController::start(&state);
    let (target, source, _) = setup(&state,&gateway);
    let mut changed = draft("Target"); changed.api_key = "new-target-key".into();
    changed.base_url = Some("https://target-new.invalid".into());
    let pending = prepared(&state,&gateway,&target,changed);
    assert_eq!(pending.kind,ProfileSaveKind::SaveAndApply);
    assert_eq!(pending.projection_profile.as_ref().unwrap().id,source.profile.id);
    assert!(commit_prepared_profile_save(&state,&gateway,&pending,false).is_err());
    commit_prepared_profile_save(&state,&gateway,&pending,true).unwrap();
    assert_eq!(gateway.active_profile_id_for(AppKind::Claude),Some(source.profile.id));
    let candidates = gateway.inner.candidates_for(AppKind::Claude, &gateway.inner.routes.read().unwrap()[&AppKind::Claude]);
    let (routes,_,fixed) = gateway.resolve_claude_target(candidates,serde_json::to_vec(&json!({"model":"asb-claude-fable"})).unwrap()).unwrap();
    assert!(fixed); assert_eq!(routes[0].api_key,"new-target-key");
    assert_eq!(routes[0].upstream_base_url,"https://target-new.invalid");
    gateway.shutdown();
}

#[test]
fn target_credential_change_invalidates_an_owner_preview_even_when_client_file_did_not_change() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state")); let gateway = GatewayController::start(&state);
    let (target,source,mut source_draft) = setup(&state,&gateway);
    source_draft.model = Some("changed-local-model".into());
    let pending = prepared(&state,&gateway,&source,source_draft);
    let before = std::fs::read(state.target(AppKind::Claude).unwrap()).unwrap();
    let mut changed = draft("Target"); changed.api_key = "external-target-key".into();
    state.configuration().update_provider(&target.profile.id,changed,&target.file_hash).unwrap();
    assert!(commit_prepared_profile_save(&state,&gateway,&pending,true).is_err());
    assert_eq!(std::fs::read(state.target(AppKind::Claude).unwrap()).unwrap(),before);
    assert_eq!(state.configuration().find_provider(&source.profile.id).unwrap(),source.profile);
    gateway.shutdown();
}

#[test]
fn failed_dependency_write_restores_the_target_and_preserves_primary() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state")); let gateway = GatewayController::start(&state);
    let (target,source,_) = setup(&state,&gateway);
    let mut changed = draft("Target"); changed.api_key = "new-target-key".into();
    let mut pending = prepared(&state,&gateway,&target,changed);
    pending.preview.as_mut().unwrap().rendered_hash = "invalid-render-hash".into();
    assert!(commit_prepared_profile_save(&state,&gateway,&pending,true).is_err());
    assert_eq!(state.configuration().find_provider(&target.profile.id).unwrap(),target.profile);
    assert_eq!(gateway.active_profile_id_for(AppKind::Claude),Some(source.profile.id));
    gateway.shutdown();
}
