use super::*;

#[test]
fn an_uncaptured_same_length_rewrite_is_never_deleted() {
    let mut fixture = Fixture::new();
    fixture.write(&[user("initial text")]); fixture.sync();
    let bytes = fs::metadata(fixture.path()).unwrap().len();
    fixture.write(&[user("changed text")]);
    assert_eq!(fs::metadata(fixture.path()).unwrap().len(), bytes);
    assert!(super::super::delete(&fixture.root, &[ID.into()]).is_err());
    assert!(fixture.path().exists());
    let tombstones: i64 = fixture.connection.query_row("SELECT count(*) FROM deleted_sessions", [], |row| row.get(0)).unwrap();
    assert_eq!(tombstones, 0);
}

#[test]
fn an_uncaptured_subagent_blocks_removal_of_the_parent_directory() {
    let mut fixture = Fixture::new(); fixture.write(&[user("parent")]); fixture.sync();
    let directory = fixture.path().with_extension("").join("subagents"); fs::create_dir_all(&directory).unwrap();
    let child = directory.join("agent-new.jsonl"); fs::write(&child, lines(&[user("new child")])).unwrap();
    let error = super::super::delete(&fixture.root, &[ID.into()]).unwrap_err();
    assert!(error.contains("子会话")); assert!(fixture.path().exists() && child.exists());
}
