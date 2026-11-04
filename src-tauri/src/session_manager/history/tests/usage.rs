use super::*;
use std::cell::Cell;

#[test]
fn committed_usage_reads_only_the_append_without_entering_the_capture_gate() {
    let mut fixture = Fixture::new();
    fixture.write(&[user("first"), assistant("first-call", 3, 5)]);
    let (first, issues) = fixture.sync();
    assert!(issues.is_empty());
    let old = first[0].retained[0].clone();
    let offset = fs::metadata(fixture.path()).unwrap().len();
    crate::model_usage::fixture_reports(&fixture.root, first, vec![], chrono::Local::now()).unwrap();
    fixture.write(&[user("first"), assistant("first-call", 3, 5), assistant("next-call", 7, 11)]);
    let (next, issues) = fixture.sync();
    assert!(issues.is_empty());
    let saved = &next[0].retained[0];
    assert_eq!(saved.id, old.id);
    assert!(visit_committed_usage(&old, offset, |_| {}, |_,_,_| Ok(())).is_err());
    let expected = fs::metadata(fixture.path()).unwrap().len() - offset;
    let total = Cell::new(0);
    let read = Cell::new(0);
    fs::remove_file(fixture.path()).unwrap();
    let _capture = GATE.lock().unwrap();
    assert_eq!(committed_sources(&fixture.root).unwrap().0.len(), 1);
    visit_committed_usage(saved, offset, |bytes| total.set(bytes), |_, position, raw| {
        assert_eq!(position, offset + read.get());
        read.set(read.get() + raw.len() as u64);
        Ok(())
    }).unwrap();
    assert_eq!(read.get(), expected);
    assert_eq!(total.get(), expected);
    let report = crate::model_usage::fixture_reports(&fixture.root, next, vec![], chrono::Local::now()).unwrap();
    assert_eq!(report.last().unwrap().groups[0].total_tokens, 26);
}

#[test]
fn an_unterminated_saved_record_is_rebuilt_when_its_line_is_extended() {
    let mut fixture = Fixture::new();
    fs::write(fixture.path(), lines(&[user("first")]) + &assistant("call", 3, 5).to_string()).unwrap();
    let (sources, _) = fixture.sync();
    crate::model_usage::fixture_reports(&fixture.root, sources, vec![], chrono::Local::now()).unwrap();
    fixture.write(&[user("first"), assistant("call", 3, 5), assistant("next", 7, 11)]);
    let (sources, issues) = fixture.sync();
    let reports = crate::model_usage::fixture_reports(&fixture.root, sources, issues, chrono::Local::now()).unwrap();
    assert_eq!(reports.last().unwrap().groups[0].total_tokens, 26);
    assert!(reports.iter().all(|report| report.issues.is_empty()));
}
