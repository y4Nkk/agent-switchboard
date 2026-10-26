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
fn prefix_index_rejects_rewrites_and_truncation_before_publishing() {
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
        assert!(reading.is_err());
        assert!(index.previous::<String>("prefix", &key(&source).unwrap()).unwrap().is_none());
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
