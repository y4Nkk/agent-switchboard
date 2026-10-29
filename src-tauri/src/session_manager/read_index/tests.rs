use super::*;
use std::io::Write;

#[test]
fn prefix_index_captures_a_fixed_range_while_the_source_keeps_appending() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.jsonl");
    fs::write(&source, b"first\n").unwrap();
    let index = ReadIndex::open(&temp.path().join("state")).unwrap();
    let first: String = index.file_prefix("prefix", &source, &|| Ok(()), |reader| {
        writeln!(fs::OpenOptions::new().append(true).open(&source).unwrap(), "second").unwrap();
        let mut value = String::new();
        reader.read_to_string(&mut value).map_err(|error| error.to_string())?;
        Ok(value)
    }).unwrap();
    assert_eq!(first, "first\n");
    let second: String = index.file_prefix("prefix", &source, &|| Ok(()), |reader| {
        let mut value = String::new();
        reader.read_to_string(&mut value).map_err(|error| error.to_string())?;
        Ok(value)
    }).unwrap();
    assert_eq!(second, "first\nsecond\n");
}

#[test]
fn prefix_index_rebuilds_changed_sources_before_publishing() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.jsonl");
    let index = ReadIndex::open(&temp.path().join("state")).unwrap();
    for changed in [b"other\n".as_slice(), b""] {
        fs::write(&source, b"first\n").unwrap();
        let reading: Result<String, String> = index.file_prefix("prefix", &source, &|| Ok(()), |reader| {
            let mut value = String::new();
            reader.read_to_string(&mut value).map_err(|error| error.to_string())?;
            fs::write(&source, changed).unwrap();
            Ok(value)
        });
        assert_eq!(reading.unwrap().as_bytes(), changed);
        assert_eq!(index.previous::<String>("prefix", &key(&source).unwrap()).unwrap().unwrap().as_bytes(), changed);
    }
}
use std::cell::Cell;

#[test]
fn file_projection_survives_reopen_and_only_rebuilds_a_changed_source() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("state");
    let source = temporary.path().join("source.jsonl");
    fs::write(&source, "first\n").unwrap();
    let builds = Cell::new(0);
    let build = || { builds.set(builds.get() + 1); Ok::<_, String>("value".to_string()) };
    let index = ReadIndex::open(&root).unwrap();
    assert_eq!(index.file("test", &source, build).unwrap(), "value");
    drop(index);
    let index = ReadIndex::open(&root).unwrap();
    assert_eq!(index.file("test", &source, build).unwrap(), "value");
    assert_eq!(builds.get(), 1);
    fs::write(&source, "first\nsecond\n").unwrap();
    index.file("test", &source, build).unwrap();
    assert_eq!(builds.get(), 2);
    assert_eq!(fs::read_to_string(source).unwrap(), "first\nsecond\n");
}

#[test]
fn an_unknown_database_is_preserved_and_never_reinterpreted_as_an_empty_index() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = layout::directory(temporary.path());
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("read-index.sqlite3");
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE material(value TEXT); PRAGMA user_version=1;").unwrap();
    drop(connection);
    let before = fs::read(&path).unwrap();
    assert!(ReadIndex::open(temporary.path()).is_err());
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn a_corrupt_projection_rebuilds_itself_without_changing_other_cache_entries() {
    let temp = tempfile::tempdir().unwrap();
    let index = ReadIndex::open(temp.path()).unwrap();
    index.put("test", "good", "version", &"retained").unwrap();
    index.connection.execute("INSERT INTO entries(kind,cache_key,revision,data) VALUES('test','bad','version','{')", []).unwrap();
    assert!(index.get::<String>("test", "bad", "version").unwrap().is_none());
    assert_eq!(index.get::<String>("test", "good", "version").unwrap().as_deref(), Some("retained"));
}

#[test]
fn known_legacy_projections_are_retired_once_without_touching_business_material() {
    let temp = tempfile::tempdir().unwrap();
    let index = ReadIndex::open(temp.path()).unwrap();
    index.put("identity", "source", "unchanged", &Option::<String>::None).unwrap();
    index.connection.pragma_update(None, "user_version", 1).unwrap();
    fs::write(temp.path().join("collections-material"), b"keep").unwrap();
    drop(index);
    let index = ReadIndex::open(temp.path()).unwrap();
    assert!(index.previous::<Option<String>>("identity", "source").unwrap().is_none());
    assert_eq!(fs::read(temp.path().join("collections-material")).unwrap(), b"keep");
    assert_eq!(index.connection.pragma_query_value::<u32, _>(None, "user_version", |row| row.get(0)).unwrap(), 2);
}

#[test]
fn repeated_rewrites_stop_after_one_retry_and_do_not_cache_a_partial_result() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.jsonl");
    fs::write(&source, b"first\n").unwrap();
    let index = ReadIndex::open(temp.path()).unwrap();
    let calls = Cell::new(0);
    let result: Result<String, String> = index.file_prefix("test", &source, &|| Ok(()), |reader| {
        let mut text = String::new();
        reader.read_to_string(&mut text).map_err(|error| error.to_string())?;
        calls.set(calls.get() + 1);
        fs::write(&source, if calls.get() == 1 { b"second\n".as_slice() } else { b"third\n".as_slice() }).unwrap();
        Ok(text)
    });
    assert!(result.is_err());
    assert_eq!(calls.get(), 2);
    assert!(index.previous::<String>("test", &key(&source).unwrap()).unwrap().is_none());
}
