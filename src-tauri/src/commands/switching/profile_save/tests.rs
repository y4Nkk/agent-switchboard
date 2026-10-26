use super::*;
use crate::{gateway::GatewayController, local_state::LocalState};
use asb_core::{AppKind, RouteMode, UpstreamProtocol};

mod activation;

fn draft(name: &str) -> ProviderDraft {
    ProviderDraft {
        app: AppKind::Claude, route_mode: RouteMode::Custom, name: name.into(),
        base_url: Some("https://claude.example".into()), connection: Default::default(),
        api_key: "fixture-key".into(), authentication: None,
        upstream_protocol: Some(UpstreamProtocol::AnthropicMessages), responses_options: None,
        max_output_tokens: None.into(), model: Some("fixture-model".into()), model_options: None,
        parameters: asb_core::ownership::default_provider_parameters(AppKind::Claude),
        claude_fragment: Default::default(), notes: None, website_url: None, display: None,
        usage_query: None, official_quota_refresh_interval_minutes: None,
    }
}

fn providers(state: &LocalState, gateway: &GatewayController) -> (ProviderRecord, ProviderRecord) {
    let target = state.configuration().create_provider(draft("Standby")).unwrap();
    let primary = state.configuration().create_provider(draft("Primary")).unwrap();
    let mut policy = crate::gateway::failover::ClaudeFailoverPolicy::default();
    policy.enabled = true;
    policy.max_retries = 1;
    policy.provider_ids = vec![target.profile.id.clone()];
    crate::gateway::failover::save(state.root(), &policy).unwrap();
    let target_path = state.target(AppKind::Claude).unwrap();
    std::fs::write(&target_path, "{}").unwrap();
    let projection = super::super::plan::build_plan_for_profile(state, gateway, primary.profile.clone(), None).unwrap();
    let preview = super::super::plan::preview_projection(state, &projection).unwrap();
    execute_projection(state, gateway, &projection, &preview.content_hash, &preview.rendered_hash).unwrap();
    (target, primary)
}

fn prepare(state: &LocalState, gateway: &GatewayController, record: &ProviderRecord) -> PreparedProfileSave {
    let mut draft = draft(&record.profile.name);
    draft.api_key = "replacement-fixture-key".into();
    draft.base_url = Some("https://replacement.example".into());
    draft.model = Some("replacement-model".into());
    let (kind, preview, projection_profile, candidate_revision) = prepare_profile_save_data(
        state, gateway, Some(&record.profile.id), &draft, Some(&record.file_hash), ProfileSaveIntent::Save,
    ).unwrap();
    PreparedProfileSave {
        intent: ProfileSaveIntent::Save,
        created_at: Instant::now(), profile_id: Some(record.profile.id.clone()),
        expected_file_hash: Some(record.file_hash.clone()), draft, kind, preview,
        projection_profile, candidate_revision,
    }
}

fn route(gateway: &GatewayController, id: &str) -> crate::gateway::ActiveRoute {
    let active = gateway.inner.routes.read().unwrap()[&AppKind::Claude].clone();
    gateway.inner.candidates_for(AppKind::Claude, &active).into_iter()
        .find(|route| route.profile_id == id).unwrap()
}

#[test]
fn claude_standby_save_replaces_the_running_candidate_after_confirmation() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, primary) = providers(&state, &gateway);
    let prepared = prepare(&state, &gateway, &target);
    assert_eq!(prepared.kind, ProfileSaveKind::SaveAndApply);
    let before = std::fs::read_to_string(state.target(AppKind::Claude).unwrap()).unwrap();
    assert!(commit_prepared_profile_save(&state, &gateway, &prepared, false).is_err());
    assert_eq!(route(&gateway, &target.profile.id).api_key, "fixture-key");
    commit_prepared_profile_save(&state, &gateway, &prepared, true).unwrap();
    let candidate = route(&gateway, &target.profile.id);
    assert_eq!(candidate.api_key, "replacement-fixture-key");
    assert_eq!(candidate.upstream_base_url, "https://replacement.example");
    assert_eq!(candidate.claude_primary_model.as_deref(), Some("replacement-model"));
    assert_eq!(gateway.active_profile_id_for(AppKind::Claude), Some(primary.profile.id));
    assert_eq!(std::fs::read_to_string(state.target(AppKind::Claude).unwrap()).unwrap(), before);
    gateway.shutdown();
}

#[test]
fn claude_active_save_uses_the_prepared_profile_for_its_first_candidate() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (_, primary) = providers(&state, &gateway);
    let prepared = prepare(&state, &gateway, &primary);
    commit_prepared_profile_save(&state, &gateway, &prepared, true).unwrap();
    assert_eq!(route(&gateway, &primary.profile.id).api_key, "replacement-fixture-key");
    gateway.shutdown();
}

#[test]
fn failed_claude_dependent_save_rolls_back_the_exact_edited_provider() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, primary) = providers(&state, &gateway);
    let mut prepared = prepare(&state, &gateway, &target);
    prepared.preview.as_mut().unwrap().rendered_hash = "stale-preview".into();
    assert!(commit_prepared_profile_save(&state, &gateway, &prepared, true).is_err());
    let saved = state.configuration().find_provider_record(&target.profile.id).unwrap();
    assert_eq!(saved.file_hash, target.file_hash);
    assert_eq!(route(&gateway, &target.profile.id).api_key, "fixture-key");
    assert_eq!(gateway.active_profile_id_for(AppKind::Claude), Some(primary.profile.id));
    assert!(state.configuration().pending_profile_save().unwrap().is_none());
    gateway.shutdown();
}

#[test]
fn claude_queue_changes_invalidate_a_prepared_dependent_save() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, _) = providers(&state, &gateway);
    let other = state.configuration().create_provider(draft("Other")).unwrap();
    let mut policy = crate::gateway::failover::load(state.root()).unwrap();
    policy.provider_ids.push(other.profile.id);
    crate::gateway::failover::save(state.root(), &policy).unwrap();
    let prepared = prepare(&state, &gateway, &target);
    policy.provider_ids.reverse();
    crate::gateway::failover::save(state.root(), &policy).unwrap();
    let result = commit_prepared_profile_save(&state, &gateway, &prepared, true);
    assert_eq!(result.unwrap_err().code, "profile-save-stale");
    assert_eq!(route(&gateway, &target.profile.id).api_key, "fixture-key");
    gateway.shutdown();
}

#[test]
fn claude_recovery_replaces_candidates_without_a_client_file_change() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().join("state"));
    let gateway = GatewayController::start(&state);
    let (target, primary) = providers(&state, &gateway);
    let prepared = prepare(&state, &gateway, &target);
    let preview = prepared.preview.as_ref().unwrap();
    assert_eq!(preview.content_hash, preview.rendered_hash);
    let (app, before) = state.configuration().load_provider_file(&target.profile.id).unwrap();
    let candidate = ProviderProfile::from_draft(target.profile.id.clone(), prepared.draft.clone());
    super::super::profile_rollback::save(&state, app, &before, &candidate,
        &preview.content_hash, &preview.rendered_hash).unwrap();
    state.configuration().begin_profile_save(&PendingProfileSave {
        profile_id: target.profile.id.clone(), projection_profile_id: primary.profile.id.clone(),
        app, previous_file_hash: target.file_hash.clone(),
    }).unwrap();
    state.configuration().update_provider(&target.profile.id, prepared.draft, &target.file_hash).unwrap();
    super::super::recovery::recover_saved_profile(&state, &gateway).unwrap();
    assert_eq!(route(&gateway, &target.profile.id).api_key, "replacement-fixture-key");
    assert_eq!(gateway.active_profile_id_for(AppKind::Claude), Some(primary.profile.id));
    assert!(state.configuration().pending_profile_save().unwrap().is_none());
    gateway.shutdown();
}
