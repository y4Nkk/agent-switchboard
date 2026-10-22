use super::*;
use std::fs;
use tempfile::tempdir;

fn meta(app: AppKind, id: &str) -> SessionMeta {
    SessionMeta { app, session_id: id.into(), title: "来源标题".into(), summary: String::new(),
        project_dir: Some("/project".into()), created_at: None, last_active_at: None,
        resume_command: None, alias: None, pinned: false, tags: Vec::new(), parent_session_id: None, can_delete: true, preservation: None }
}

#[test]
fn stores_one_local_organization_per_client_session_and_removes_empty_rows() {
    let temp = tempdir().unwrap();
    assert!(load(temp.path()).unwrap().is_empty());
    let codex = meta(AppKind::Codex, "same-id");
    let claude = meta(AppKind::Claude, "same-id");
    let rows = update_resolved(temp.path(), vec![codex.clone(), claude.clone()],
        &SessionOrganizationChange::Pin { pinned: true }).unwrap();
    assert!(rows.iter().all(|row| row.pinned));
    let rows = update_resolved(temp.path(), vec![codex.clone()],
        &SessionOrganizationChange::Alias { alias: Some("  本地别名  ".into()) }).unwrap();
    assert_eq!(rows[0].alias.as_deref(), Some("本地别名"));
    assert_eq!(rows[0].title, "来源标题");
    update_resolved(temp.path(), vec![codex.clone()], &SessionOrganizationChange::Alias { alias: None }).unwrap();
    update_resolved(temp.path(), vec![codex], &SessionOrganizationChange::Pin { pinned: false }).unwrap();
    let saved = load(temp.path()).unwrap();
    assert_eq!(saved.len(), 1);
    assert!(saved[&(AppKind::Claude, "same-id".into())].pinned);
}

#[test]
fn batches_rollback_every_change_when_one_session_exceeds_tag_limit() {
    let temp = tempdir().unwrap();
    let first = meta(AppKind::Codex, "first");
    let second = meta(AppKind::Codex, "second");
    update_resolved(temp.path(), vec![second.clone()], &SessionOrganizationChange::SetTags {
        tags: (0..20).map(|index| format!("tag{index:02}")).collect(),
    }).unwrap();
    assert!(update_resolved(temp.path(), vec![first, second], &SessionOrganizationChange::AddTags {
        tags: vec!["overflow".into()],
    }).is_err());
    let saved = load(temp.path()).unwrap();
    assert_eq!(saved.len(), 1);
    assert!(!saved[&(AppKind::Codex, "second".into())].tags.contains(&"overflow".into()));
}

#[test]
fn tags_are_normalized_and_incremental_operations_preserve_other_tags() {
    let mut value = Organization::default();
    update_value(&mut value, &SessionOrganizationChange::SetTags { tags: vec![" beta ".into(), "alpha".into(), "beta".into()] }).unwrap();
    assert_eq!(value.tags, ["alpha", "beta"]);
    update_value(&mut value, &SessionOrganizationChange::AddTags { tags: vec!["gamma".into()] }).unwrap();
    update_value(&mut value, &SessionOrganizationChange::RemoveTags { tags: vec!["beta".into()] }).unwrap();
    assert_eq!(value.tags, ["alpha", "gamma"]);
    assert!(update_value(&mut value, &SessionOrganizationChange::SetTags { tags: vec!["".into()] }).is_err());
    assert!(update_value(&mut value, &SessionOrganizationChange::Alias { alias: Some("a".repeat(121)) }).is_err());
}

#[test]
fn rejects_invalid_ids_unknown_fields_and_batch_alias_requests() {
    let invalid = SessionDeleteRequest { app: AppKind::Codex, session_id: "../../wrong".into() };
    assert!(validate_request(&[invalid], &SessionOrganizationChange::Pin { pinned: true }).is_err());
    let request = SessionDeleteRequest { app: AppKind::Codex, session_id: "valid".into() };
    assert!(validate_request(&[request.clone(), request], &SessionOrganizationChange::Alias { alias: None }).is_err());
    assert!(serde_json::from_str::<SessionOrganizationChange>(r#"{"kind":"pin","pinned":true,"alias":"unowned"}"#).is_err());
}

#[test]
fn deletion_rolls_back_on_source_failure_and_keeps_bookmark_database() {
    let temp = tempdir().unwrap();
    let session = meta(AppKind::Codex, "s1");
    update_resolved(temp.path(), vec![session], &SessionOrganizationChange::Pin { pinned: true }).unwrap();
    assert!(delete_with_sources(temp.path(), AppKind::Codex, &["s1".into()], || Err("source removal failed".into())).is_err());
    assert_eq!(load(temp.path()).unwrap().len(), 1);
    let source = temp.path().join("transcript.jsonl");
    fs::write(&source, "original source").unwrap();
    let collections = super::super::layout::directory(temp.path()).join("collections.sqlite3");
    fs::write(&collections, "bookmark snapshot remains owned separately").unwrap();
    delete_with_sources(temp.path(), AppKind::Codex, &["s1".into()], || fs::remove_file(&source).map_err(|error| error.to_string())).unwrap();
    assert!(load(temp.path()).unwrap().is_empty());
    assert!(!source.exists());
    assert_eq!(fs::read_to_string(collections).unwrap(), "bookmark snapshot remains owned separately");
}

#[test]
fn foreign_or_empty_database_is_rejected_without_replacement() {
    for content in [b"".as_slice(), b"invalid sqlite data".as_slice()] {
        let temp = tempdir().unwrap();
        let path = super::super::layout::directory(temp.path()).join("organization.sqlite3");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        assert!(load(temp.path()).is_err());
        assert_eq!(fs::read(path).unwrap(), content);
    }
}

#[test]
fn simultaneous_first_writes_preserve_both_sessions() {
    let temp = tempdir().unwrap();
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        for id in ["first", "second"] {
            let barrier = &barrier;
            let root = temp.path();
            scope.spawn(move || {
                barrier.wait();
                update_resolved(root, vec![meta(AppKind::Codex, id)], &SessionOrganizationChange::Pin { pinned: true }).unwrap();
            });
        }
    });
    assert_eq!(load(temp.path()).unwrap().len(), 2);
}

#[test]
fn sidecar_metadata_is_removed_in_the_same_transaction_and_unrelated_rows_remain() {
    let temp = tempdir().unwrap();
    let metas = ["main", "main-agent-one", "other"].map(|id| meta(AppKind::Claude, id));
    update_resolved(temp.path(), metas.to_vec(), &SessionOrganizationChange::Pin { pinned: true }).unwrap();
    delete_with_sources(temp.path(), AppKind::Claude, &["main".into(), "main-agent-one".into()], || Ok(())).unwrap();
    let rows = load(temp.path()).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows.contains_key(&(AppKind::Claude, "other".into())));
}
