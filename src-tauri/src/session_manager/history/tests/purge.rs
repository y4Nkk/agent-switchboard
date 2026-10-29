use super::*;

fn record(at: &str) -> Value {
    let mut value = user("same message");
    value["timestamp"] = json!(at);
    value
}

fn stale_fixture() -> Fixture {
    let mut fixture = Fixture::new();
    fixture.write(&[record("2020-01-01T00:00:00Z")]);
    let (_, issues) = fixture.sync();
    assert!(issues.is_empty(), "{issues:?}");
    fixture
}

fn guarded_delete(fixture: &Fixture) -> Result<(), String> {
    super::super::delete(
        &fixture.root, &[ID.to_string()], Some(purge_cutoff("2021-01-01").unwrap()),
    )
}

#[test]
fn newly_captured_old_sessions_are_eligible_without_changing_saved_at() {
    let fixture = stale_fixture();
    let saved: String = fixture.connection.query_row("SELECT saved_at FROM transcripts", [], |row| row.get(0)).unwrap();
    assert!(saved.as_str() > "2021-01-01");
    assert_eq!(preview_purge(&fixture.root, "2021-01-01").unwrap().sessions, 1);
}

#[test]
fn activity_unknown_in_the_native_records_is_protected() {
    let mut fixture = Fixture::new();
    let mut value = user("unknown activity");
    value.as_object_mut().unwrap().remove("timestamp");
    fixture.write(&[value]);
    fixture.sync();
    assert_eq!(preview_purge(&fixture.root, "2021-01-01").unwrap().sessions, 0);
    assert!(fixture.path().exists());
}

#[test]
fn a_recently_recaptured_session_is_rejected_inside_the_deletion_gate() {
    let mut fixture = stale_fixture();
    assert_eq!(preview_purge(&fixture.root, "2021-01-01").unwrap().sessions, 1);
    fixture.write(&[record("2026-01-01T00:00:00Z")]);
    fixture.sync();
    let failure = guarded_delete(&fixture).unwrap_err();
    assert!(failure.contains("最后活动时间"));
    assert!(fixture.path().exists());
}

#[test]
fn uncaptured_same_length_changes_are_preserved_instead_of_deleted() {
    let fixture = stale_fixture();
    let before = fs::metadata(fixture.path()).unwrap().len();
    fixture.write(&[record("2026-01-01T00:00:00Z")]);
    assert_eq!(fs::metadata(fixture.path()).unwrap().len(), before);
    assert!(guarded_delete(&fixture).is_err());
    assert!(fixture.path().exists());
    let tombstones: i64 = fixture.connection.query_row("SELECT count(*) FROM deleted_sessions", [], |row| row.get(0)).unwrap();
    assert_eq!(tombstones, 0);
}

#[test]
fn freshly_captured_activity_is_excluded_from_bulk_deletion() {
    let mut fixture = stale_fixture();
    fixture.write(&[record("2026-01-01T00:00:00Z")]);
    let (sources, issues) = fixture.sync();
    let result = crate::session_manager::purge_resolved(
        &fixture.root, &sources, &issues, purge_cutoff("2021-01-01").unwrap(),
    ).unwrap();
    assert_eq!(result.removed, 0);
    assert!(fixture.path().exists());
}

#[test]
fn a_new_uncaptured_subagent_protects_the_whole_original_directory() {
    let fixture = stale_fixture();
    let directory = fixture.path().with_extension("").join("subagents");
    fs::create_dir_all(&directory).unwrap();
    let child = directory.join("agent-new.jsonl");
    fs::write(&child, lines(&[record("2026-01-01T00:00:00Z")])).unwrap();
    let failure = guarded_delete(&fixture).unwrap_err();
    assert!(failure.contains("子会话"));
    assert!(child.exists());
    assert!(fixture.path().exists());
}

#[test]
fn latest_child_activity_protects_its_stale_parent() {
    let mut fixture = stale_fixture();
    let directory = fixture.path().with_extension("").join("subagents");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("agent-active.jsonl"), lines(&[record("2026-01-01T00:00:00Z")])).unwrap();
    let (_, issues) = fixture.sync();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(preview_purge(&fixture.root, "2021-01-01").unwrap().sessions, 0);
}
