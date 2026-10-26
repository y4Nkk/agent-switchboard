use super::*;
use crate::{gateway::GatewayController, local_state::LocalState};
use asb_core::{contracts::CodexSubagentRoute, AppKind};

pub(super) fn activate(state: &LocalState, gateway: &GatewayController, file: CodexProviderFile) {
    let target = state.target(AppKind::Codex).unwrap();
    std::fs::write(&target, "model_provider = 'openai'\n").unwrap();
    std::fs::write(target.with_file_name("auth.json"),
        r#"{"auth_mode":"chatgpt","tokens":{"access_token":"at","refresh_token":"rt","id_token":"id"}}"#).unwrap();
    let projection = build_codex_plan(state, gateway, file).unwrap();
    let preview = preview_projection(state, &projection).unwrap();
    execute_projection(state, gateway, &projection, &preview.content_hash, &preview.rendered_hash).unwrap();
}

fn providers(state: &LocalState) -> (CodexProviderRecord, CodexProviderFile) {
    let target = state.configuration().create_codex_provider(crate::codex_common::test_draft()).unwrap();
    let mut source = crate::codex_common::test_draft();
    source.name = "Primary".into();
    source.subagent_route = Some(CodexSubagentRoute {
        profile_id: target.profile.id.clone(), model: "relay-model".into(),
    });
    let source = state.configuration().create_codex_provider(source).unwrap();
    let source = state.configuration().find_codex_provider_file(&source.profile.id).unwrap();
    (target, source)
}

fn prepare(state: &LocalState, gateway: &GatewayController, target: &CodexProviderRecord) -> PreparedCodexProfileSave {
    let mut draft = crate::codex_common::test_draft();
    draft.catalog[0].context_window *= 2;
    let (kind, preview, projection_profile, candidate_revision) = prepare_data(
        state, gateway, &target.profile.id, &draft, &target.file_hash, ProfileSaveIntent::Save,
    ).unwrap();
    PreparedCodexProfileSave { intent: ProfileSaveIntent::Save, created_at: Instant::now(), profile_id: target.profile.id.clone(),
        expected_file_hash: target.file_hash.clone(), draft, kind, preview, projection_profile, candidate_revision }
}

#[test]
fn saving_target_refreshes_primary_catalog_without_switching_owner() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, source) = providers(&state);
    activate(&state, &gateway, source.clone());
    let prepared = prepare(&state, &gateway, &target);
    assert_eq!(prepared.kind, ProfileSaveKind::SaveAndApply);
    assert_eq!(prepared.projection_profile.as_ref().unwrap().profile.id, source.profile.id);
    let preview = prepared.preview.as_ref().unwrap();
    assert_ne!(preview.content_hash, preview.rendered_hash);
    let saved = commit(&state, &gateway, &prepared, true).unwrap();
    assert_eq!(saved.profile.id, target.profile.id);
    let text = std::fs::read_to_string(state.target(AppKind::Codex).unwrap()).unwrap();
    assert_eq!(gateway.active_profile_id(AppKind::Codex, &text).unwrap(), Some(source.profile.id));
    let catalog = gateway.restored_codex_catalog(&state, &text).unwrap().unwrap();
    let document: serde_json::Value = serde_json::from_str(&catalog.content).unwrap();
    let wire = CodexSubagentRoute { profile_id: target.profile.id, model: "relay-model".into() }.wire_id();
    let entry = document["models"].as_array().unwrap().iter()
        .find(|entry| entry["slug"] == wire).unwrap();
    assert_eq!(entry["context_window"], saved.profile.catalog[0].context_window);
    assert!(state.configuration().pending_profile_save().unwrap().is_none());
    gateway.shutdown();
}

#[test]
fn failed_dependent_save_restores_target_and_keeps_primary_owner() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, source) = providers(&state);
    activate(&state, &gateway, source.clone());
    let mut prepared = prepare(&state, &gateway, &target);
    prepared.preview.as_mut().unwrap().rendered_hash = "stale-confirmation".into();
    assert!(commit(&state, &gateway, &prepared, true).is_err());
    let actual = state.configuration().list_codex_providers().unwrap().into_iter()
        .find(|record| record.profile.id == target.profile.id).unwrap();
    assert_eq!(actual.file_hash, target.file_hash);
    let text = std::fs::read_to_string(state.target(AppKind::Codex).unwrap()).unwrap();
    assert_eq!(gateway.active_profile_id(AppKind::Codex, &text).unwrap(), Some(source.profile.id));
    assert!(state.configuration().pending_profile_save().unwrap().is_none());
    gateway.shutdown();
}

#[test]
fn referenced_model_cannot_be_removed_and_self_reference_uses_candidate() {
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let (target, _) = providers(&state);
    let mut candidate = crate::codex_common::test_draft().into_file(target.profile.id, 0);
    candidate.profile.catalog[0].id = "replacement-model".into();
    assert!(dependencies::validate_references(&state, &candidate).is_err());
    let mut self_file = crate::codex_common::test_draft().into_file(Uuid::new_v4().to_string(), 0);
    self_file.profile.subagent_route = Some(CodexSubagentRoute {
        profile_id: self_file.profile.id.clone(), model: "relay-model".into(),
    });
    assert!(dependencies::validate_references(&state, &self_file).is_ok());
}

#[test]
fn interrupted_dependent_save_recovers_the_primary_projection() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, source) = providers(&state);
    activate(&state, &gateway, source.clone());
    let prepared = prepare(&state, &gateway, &target);
    let preview = prepared.preview.as_ref().unwrap();
    let before = state.configuration().find_codex_provider_file(&target.profile.id).unwrap();
    let candidate = prepared.draft.clone().into_file(target.profile.id.clone(), before.position);
    super::super::profile_rollback::save_codex(&state, &before, &candidate,
        &preview.content_hash, &preview.rendered_hash).unwrap();
    state.configuration().begin_profile_save(&PendingProfileSave {
        profile_id: target.profile.id.clone(), projection_profile_id: source.profile.id.clone(),
        app: AppKind::Codex, previous_file_hash: target.file_hash.clone(),
    }).unwrap();
    state.configuration().update_codex_provider(&target.profile.id, prepared.draft, &target.file_hash).unwrap();
    let projection = dependencies::projection(&state, &gateway, &source, &candidate).unwrap();
    let auth = super::super::projection_transaction::auth_intent(
        preview.auth_hash.as_deref(), preview.auth_existed, preview.auth_rendered_hash.as_deref(),
    ).unwrap();
    super::super::projection_transaction::begin(&state, &gateway, &projection,
        &state.target(AppKind::Codex).unwrap(), &preview.content_hash, &preview.rendered_hash, auth).unwrap();
    let recovered = super::super::recovery::recover_saved_profile(&state, &gateway).unwrap();
    assert_eq!(recovered.as_deref(), Some(target.profile.id.as_str()));
    let text = std::fs::read_to_string(state.target(AppKind::Codex).unwrap()).unwrap();
    assert_eq!(asb_switch::sha256_hex(&text), preview.rendered_hash);
    assert_eq!(gateway.active_profile_id(AppKind::Codex, &text).unwrap(), Some(source.profile.id));
    assert!(state.configuration().pending_profile_save().unwrap().is_none());
    assert!(super::super::recovery::recover_saved_profile(&state, &gateway).unwrap().is_none());
    gateway.shutdown();
}
