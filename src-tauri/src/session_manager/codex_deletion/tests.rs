use super::*;
use crate::session_manager::fixture_source;
use std::fs;

const PARENT: &str = "00000000-0000-4000-8000-000000000001";
const CHILD: &str = "00000000-0000-4000-8000-000000000002";
const GRANDCHILD: &str = "00000000-0000-4000-8000-000000000003";
const FORK: &str = "00000000-0000-4000-8000-000000000004";

fn scope(root: &Path) -> Scope { Scope { home: root.into(), database: root.join("state_5.sqlite") } }

fn registered(root: &Path, id: &str, parent: Option<&str>) -> SessionSource {
    let database = root.join("state_5.sqlite");
    let connection = Connection::open(&database).unwrap();
    connection.execute_batch("CREATE TABLE IF NOT EXISTS threads(id TEXT PRIMARY KEY)").unwrap();
    connection.execute("INSERT INTO threads(id) VALUES(?1)", [id]).unwrap();
    let path = root.join(format!("{id}.jsonl"));
    fs::write(&path, format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\"}}}}\n")).unwrap();
    let mut source = fixture_source(&path, AppKind::Codex, id);
    source.native_database = Some(database);
    source.meta.parent_session_id = parent.map(str::to_string);
    source
}

fn erase_native_record(source: &SessionSource) {
    let connection = Connection::open(source.native_database.as_ref().unwrap()).unwrap();
    connection.execute("DELETE FROM threads WHERE id=?1", [&source.meta.session_id]).unwrap();
    for path in &source.paths { fs::remove_file(path).unwrap(); }
}

#[test]
fn missing_rollout_never_counts_as_deleted_while_native_registration_remains() {
    let directory = tempfile::tempdir().unwrap();
    let source = registered(directory.path(), PARENT, None);
    fs::remove_file(&source.paths[0]).unwrap();
    assert!(!Verification::open(&scope(directory.path()), &source).unwrap().removed(&source).unwrap());
}

#[test]
fn native_registration_and_every_fragment_must_be_removed() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = registered(directory.path(), PARENT, None);
    erase_native_record(&source);
    source.paths.push(directory.path().join("archive.jsonl.zst"));
    fs::write(&source.paths[1], b"retained fragment").unwrap();
    assert!(!Verification::open(&scope(directory.path()), &source).unwrap().removed(&source).unwrap());
    fs::remove_file(&source.paths[1]).unwrap();
    assert!(Verification::open(&scope(directory.path()), &source).unwrap().removed(&source).unwrap());
}

#[test]
fn remaining_paginated_history_prevents_a_success_report() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = registered(directory.path(), PARENT, None);
    erase_native_record(&source);
    let history = directory.path().join("thread_history_1.sqlite");
    let connection = Connection::open(&history).unwrap();
    for table in ["thread_turns", "thread_items", "thread_realtime_items", "thread_history_projection_state"] {
        connection.execute_batch(&format!("CREATE TABLE {table}(thread_id TEXT)")).unwrap();
    }
    source.native_history = Some(history);
    for table in ["thread_turns", "thread_items", "thread_realtime_items", "thread_history_projection_state"] {
        connection.execute(&format!("INSERT INTO {table} VALUES(?1)"), [PARENT]).unwrap();
        assert!(!Verification::open(&scope(directory.path()), &source).unwrap().removed(&source).unwrap(), "{table}");
        connection.execute(&format!("DELETE FROM {table} WHERE thread_id=?1"), [PARENT]).unwrap();
    }
    assert!(Verification::open(&scope(directory.path()), &source).unwrap().removed(&source).unwrap());
}

#[test]
fn disappeared_native_database_preserves_the_execution_and_verification_errors() {
    let directory = tempfile::tempdir().unwrap();
    let source = registered(directory.path(), PARENT, None);
    fs::remove_file(source.native_database.as_ref().unwrap()).unwrap();
    let error = complete(directory.path(), &source, &[], &scope(directory.path()), Some("native deletion failed".into())).unwrap_err();
    assert!(error.contains("native deletion failed") && error.contains("历史库已消失"));
    assert!(!source.native_database.as_ref().unwrap().exists());
    assert!(source.paths[0].exists());
}

#[test]
fn damaged_native_database_is_reported_without_rebuilding_it() {
    let directory = tempfile::tempdir().unwrap();
    let source = registered(directory.path(), PARENT, None);
    let database = source.native_database.as_ref().unwrap();
    fs::write(database, b"damaged native database").unwrap();
    let error = match Verification::open(&scope(directory.path()), &source) {
        Ok(verification) => verification.removed(&source).unwrap_err(),
        Err(error) => error,
    };
    assert!(!error.is_empty());
    assert_eq!(fs::read(database).unwrap(), b"damaged native database");
    assert!(source.paths[0].exists());
}

#[test]
fn invalid_codex_identity_never_uses_direct_file_deletion_even_when_unregistered() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("record.jsonl");
    fs::write(&path, b"original transcript").unwrap();
    let source = fixture_source(&path, AppKind::Codex, "not-a-native-uuid");
    let request = super::super::SessionDeleteRequest { app: AppKind::Codex, session_id: source.meta.session_id.clone() };
    let outcomes = super::super::delete_resolved(directory.path(), &[source], &[], &[request], None);
    assert!(!outcomes[0].deleted);
    assert!(outcomes[0].error.as_ref().unwrap().contains("UUID"));
    assert_eq!(fs::read(path).unwrap(), b"original transcript");
}

#[test]
fn changed_codex_home_cannot_retarget_an_unregistered_record() {
    let _clients = crate::test_client_paths::redirect_client_paths();
    let codex_home = PathBuf::from(std::env::var_os("CODEX_HOME").unwrap());
    fs::write(codex_home.join("config.toml"), "sqlite_home = '.'\n").unwrap();
    let old_home = tempfile::tempdir().unwrap();
    let path = old_home.path().join("sessions").join(format!("{PARENT}.jsonl"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"original transcript").unwrap();
    let source = fixture_source(&path, AppKind::Codex, PARENT);
    let error = match Scope::resolve(&source) {
        Ok(_) => panic!("changed client home must be rejected"),
        Err(error) => error,
    };
    assert!(error.contains("原始位置已变化"));
    assert_eq!(fs::read(path).unwrap(), b"original transcript");
}

#[test]
fn completion_reports_removed_descendants_and_keeps_an_independent_fork() {
    let directory = tempfile::tempdir().unwrap();
    let parent = registered(directory.path(), PARENT, None);
    let child = registered(directory.path(), CHILD, Some(PARENT));
    let grandchild = registered(directory.path(), GRANDCHILD, Some(CHILD));
    let fork = registered(directory.path(), FORK, Some(PARENT));
    for source in [&parent, &child, &grandchild] { erase_native_record(source); }
    let sources = vec![grandchild, child, fork.clone()];
    let outcomes = complete(directory.path(), &parent, &sources, &scope(directory.path()), None).unwrap();
    assert_eq!(outcomes.iter().map(|outcome| outcome.session_id.as_str()).collect::<Vec<_>>(), [PARENT, CHILD, GRANDCHILD]);
    assert!(outcomes.iter().all(|outcome| outcome.deleted && outcome.error.is_none()));
    assert!(fork.paths[0].exists());
    assert!(!Verification::open(&scope(directory.path()), &fork).unwrap().removed(&fork).unwrap());
}

#[test]
fn related_sessions_are_transitive_without_including_siblings_or_other_clients() {
    let directory = tempfile::tempdir().unwrap();
    let source = |app, id: &str, parent: Option<&str>| {
        let mut source = fixture_source(&directory.path().join(id), app, id);
        source.meta.parent_session_id = parent.map(str::to_string);
        source
    };
    let root = source(AppKind::Codex, "parent", None);
    let sources = vec![source(AppKind::Codex, "grandchild", Some("child")), source(AppKind::Codex, "child", Some("parent")),
        source(AppKind::Codex, "sibling", Some("other")), source(AppKind::Claude, "claude-child", Some("parent"))];
    assert_eq!(related_sources(&root, &sources).iter().map(|source| source.meta.session_id.as_str()).collect::<Vec<_>>(),
        ["parent", "child", "grandchild"]);
}

#[test]
fn deletable_native_metadata_is_not_overwritten_by_rollout_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let mut source = registered(directory.path(), PARENT, None);
    source.meta.title = "native title".into();
    source.meta.last_active_at = Some("2026-11-09T01:00:00Z".into());
    let mut file = fs::OpenOptions::new().append(true).open(&source.paths[0]).unwrap();
    use std::io::Write;
    writeln!(file, "{{\"type\":\"event_msg\",\"timestamp\":\"2026-11-10T01:00:00Z\",\"payload\":{{\"type\":\"user_message\",\"message\":\"rollout title\"}}}}").unwrap();
    drop(file);
    source.paths.clear();
    let mut indexed = std::collections::HashMap::from([((AppKind::Codex, PARENT.into()), source)]);
    let mut issues = Vec::new();
    super::super::sources::scan_roots(&[(AppKind::Codex, directory.path().into())], &mut indexed, &mut issues);
    assert!(issues.is_empty(), "{issues:?}");
    let meta = &indexed.values().next().unwrap().meta;
    assert!(meta.can_delete);
    assert_eq!(meta.title, "native title");
    assert_eq!(meta.last_active_at.as_deref(), Some("2026-11-09T01:00:00Z"));
}
