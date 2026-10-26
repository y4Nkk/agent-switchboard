use super::*;
use crate::{gateway::GatewayController, local_state::LocalState};
use asb_core::AppKind;

mod activation;

fn providers(state: &LocalState, gateway: &GatewayController) -> (CodexProviderRecord, CodexProviderFile) {
    let target = state.configuration().create_codex_provider(crate::codex_common::test_draft()).unwrap();
    let mut draft = crate::codex_common::test_draft();
    draft.name = "Primary".into();
    let primary = state.configuration().create_codex_provider(draft).unwrap();
    let primary = state.configuration().find_codex_provider_file(&primary.profile.id).unwrap();
    let mut policy = crate::gateway::codex::policy::CodexGatewayPolicy::default();
    policy.takeover = true;
    policy.enabled = true;
    policy.provider_ids = vec![target.profile.id.clone()];
    crate::gateway::codex::policy::save(state.root(), &policy).unwrap();
    super::dependency_tests::activate(state, gateway, primary.clone());
    (target, primary)
}

fn prepare(state: &LocalState, gateway: &GatewayController, target: &CodexProviderRecord) -> PreparedCodexProfileSave {
    let mut draft = crate::codex_common::test_draft();
    draft.api_key = "replacement-fixture-key".into();
    draft.endpoint.0 = "https://replacement.example/v1".into();
    draft.model_routes = vec![asb_core::contracts::CodexModelRoute {
        client_model: "relay-model".into(), upstream_model: "replacement-model".into(),
    }];
    let (kind, preview, projection_profile, candidate_revision) = prepare_data(
        state, gateway, &target.profile.id, &draft, &target.file_hash, ProfileSaveIntent::Save,
    ).unwrap();
    PreparedCodexProfileSave {
        intent: ProfileSaveIntent::Save,
        created_at: Instant::now(), profile_id: target.profile.id.clone(),
        expected_file_hash: target.file_hash.clone(), draft, kind, preview,
        projection_profile, candidate_revision,
    }
}

fn route(gateway: &GatewayController, profile_id: &str) -> crate::gateway::ActiveRoute {
    let primary = gateway.inner.routes.read().unwrap()[&AppKind::Codex].clone();
    gateway.inner.candidates_for(AppKind::Codex, &primary).into_iter()
        .find(|route| route.profile_id == profile_id).unwrap()
}

fn assert_updated(gateway: &GatewayController, target: &str, primary: &str) {
    let route = route(gateway, target);
    assert_eq!(gateway.active_profile_id_for(AppKind::Codex).as_deref(), Some(primary));
    assert_eq!(route.api_key, "replacement-fixture-key");
    assert_eq!(route.upstream_base_url, "https://replacement.example/v1");
    assert_eq!(route.codex.unwrap().resolve_model("relay-model").unwrap(), "replacement-model");
}

#[test]
fn queued_provider_save_requires_confirmation_and_replaces_the_live_snapshot() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, primary) = providers(&state, &gateway);
    let prepared = prepare(&state, &gateway, &target);
    assert_eq!(prepared.kind, ProfileSaveKind::SaveAndApply);
    let before = std::fs::read_to_string(state.target(AppKind::Codex).unwrap()).unwrap();
    assert!(commit(&state, &gateway, &prepared, false).is_err());
    assert_eq!(route(&gateway, &target.profile.id).api_key, target.profile.api_key);
    commit(&state, &gateway, &prepared, true).unwrap();
    assert_updated(&gateway, &target.profile.id, &primary.profile.id);
    assert_eq!(std::fs::read_to_string(state.target(AppKind::Codex).unwrap()).unwrap(), before);
    assert!(state.configuration().pending_profile_save().unwrap().is_none());
    gateway.shutdown();
}

#[test]
fn failed_queued_save_restores_the_provider_and_the_running_candidate() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, primary) = providers(&state, &gateway);
    let mut prepared = prepare(&state, &gateway, &target);
    prepared.preview.as_mut().unwrap().rendered_hash = "stale-confirmation".into();
    assert!(commit(&state, &gateway, &prepared, true).is_err());
    let current = state.configuration().find_codex_provider_file(&target.profile.id).unwrap();
    assert_eq!(current.profile.api_key, target.profile.api_key);
    assert_eq!(route(&gateway, &target.profile.id).api_key, target.profile.api_key);
    assert_eq!(gateway.active_profile_id_for(AppKind::Codex), Some(primary.profile.id));
    assert!(state.configuration().pending_profile_save().unwrap().is_none());
    gateway.shutdown();
}

#[test]
fn queue_reordering_invalidates_a_prepared_provider_save() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, _) = providers(&state, &gateway);
    let other = state.configuration().create_codex_provider(crate::codex_common::test_draft()).unwrap();
    let (mut policy, _) = crate::gateway::codex::policy::load(state.root()).unwrap();
    policy.provider_ids.push(other.profile.id);
    crate::gateway::codex::policy::save(state.root(), &policy).unwrap();
    let prepared = prepare(&state, &gateway, &target);
    policy.provider_ids.reverse();
    crate::gateway::codex::policy::save(state.root(), &policy).unwrap();
    assert_eq!(commit(&state, &gateway, &prepared, true).unwrap_err().code, "codex-profile-save-stale");
    assert_eq!(route(&gateway, &target.profile.id).api_key, target.profile.api_key);
    gateway.shutdown();
}

#[test]
fn recovery_refreshes_candidates_even_when_the_client_file_did_not_change() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, primary) = providers(&state, &gateway);
    let prepared = prepare(&state, &gateway, &target);
    let preview = prepared.preview.as_ref().unwrap();
    assert_eq!(preview.content_hash, preview.rendered_hash);
    let before = state.configuration().find_codex_provider_file(&target.profile.id).unwrap();
    let candidate = prepared.draft.clone().into_file(target.profile.id.clone(), before.position);
    super::super::profile_rollback::save_codex(&state, &before, &candidate,
        &preview.content_hash, &preview.rendered_hash).unwrap();
    state.configuration().begin_profile_save(&PendingProfileSave {
        profile_id: target.profile.id.clone(), projection_profile_id: primary.profile.id.clone(),
        app: AppKind::Codex, previous_file_hash: target.file_hash.clone(),
    }).unwrap();
    state.configuration().update_codex_provider(&target.profile.id, prepared.draft, &target.file_hash).unwrap();
    assert_eq!(route(&gateway, &target.profile.id).api_key, target.profile.api_key);
    super::super::recovery::recover_saved_profile(&state, &gateway).unwrap();
    assert_updated(&gateway, &target.profile.id, &primary.profile.id);
    assert!(state.configuration().pending_profile_save().unwrap().is_none());
    gateway.shutdown();
}

#[test]
fn a_disabled_queue_does_not_apply_an_unrelated_saved_provider() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, _) = providers(&state, &gateway);
    let (mut policy, _) = crate::gateway::codex::policy::load(state.root()).unwrap();
    policy.enabled = false;
    crate::gateway::codex::policy::save(state.root(), &policy).unwrap();
    let prepared = prepare(&state, &gateway, &target);
    assert_eq!(prepared.kind, ProfileSaveKind::SaveOnly);
    assert!(prepared.preview.is_none());
    gateway.shutdown();
}
