use super::*;
use crate::session_manager::{fixture_source, read_index::ReadIndex, records};
use serde_json::json;
use std::fs;
use std::io::Write;

fn message(id: &str, output: u64) -> String {
    format!("{}\n", json!({"type":"assistant","timestamp":"2026-10-22T02:00:00Z",
        "message":{"id":id,"model":"model","stop_reason":"end_turn",
            "usage":{"input_tokens":10,"output_tokens":output}}}))
}

fn all(reports: &[ModelUsageReport]) -> u64 {
    reports.iter().find(|report| report.range == ModelUsageRange::All).unwrap().groups.iter().map(|group| group.total_tokens).sum()
}

#[test]
fn unchanged_sources_decode_nothing_and_append_only_decodes_new_records() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("state");
    let path = temp.path().join("session.jsonl");
    fs::write(&path,message("first",2)).unwrap();
    let source = fixture_source(&path,AppKind::Claude,"session");
    assert_eq!(all(&fixture_reports(&root,vec![source.clone()],vec![],Local::now()).unwrap()),12);
    let before = records::decoded_record_count();
    assert_eq!(all(&fixture_reports(&root,vec![source.clone()],vec![],Local::now()).unwrap()),12);
    assert_eq!(records::decoded_record_count(),before);
    write!(fs::OpenOptions::new().append(true).open(&path).unwrap(),"{}",message("second",4)).unwrap();
    assert_eq!(all(&fixture_reports(&root,vec![source],vec![],Local::now()).unwrap()),26);
    assert_eq!(records::decoded_record_count() - before,1);
}

#[test]
fn deleting_one_duplicate_source_keeps_the_call_until_its_last_source_disappears() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("state");
    let first = temp.path().join("first.jsonl");
    let second = temp.path().join("second.jsonl");
    fs::write(&first,message("same",2)).unwrap();
    fs::write(&second,message("same",2)).unwrap();
    let one = fixture_source(&first,AppKind::Claude,"session");
    let two = fixture_source(&second,AppKind::Claude,"session");
    assert_eq!(all(&fixture_reports(&root,vec![one,two.clone()],vec![],Local::now()).unwrap()),12);
    assert_eq!(all(&fixture_reports(&root,vec![two],vec![],Local::now()).unwrap()),12);
    assert_eq!(all(&fixture_reports(&root,vec![],vec![],Local::now()).unwrap()),0);
}

#[test]
fn rewrites_and_partial_tail_completion_replace_the_old_contribution() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("state");
    let path = temp.path().join("session.jsonl");
    let source = fixture_source(&path,AppKind::Claude,"session");
    fs::write(&path,message("first",2)).unwrap();
    fixture_reports(&root,vec![source.clone()],vec![],Local::now()).unwrap();
    fs::write(&path,message("first",9)).unwrap();
    assert_eq!(all(&fixture_reports(&root,vec![source.clone()],vec![],Local::now()).unwrap()),19);
    let second = message("second",4);
    let cut = second.len()/2;
    fs::write(&path,message("first",2)+&second[..cut]).unwrap();
    assert_eq!(all(&fixture_reports(&root,vec![source.clone()],vec![],Local::now()).unwrap()),12);
    fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(second[cut..].as_bytes()).unwrap();
    assert_eq!(all(&fixture_reports(&root,vec![source],vec![],Local::now()).unwrap()),26);
}

#[test]
fn a_failed_source_transaction_leaves_cursor_calls_and_buckets_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("state");
    let path = temp.path().join("session.jsonl");
    fs::write(&path,message("first",2)).unwrap();
    let source = fixture_source(&path,AppKind::Claude,"session");
    fixture_reports(&root,vec![source.clone()],vec![],Local::now()).unwrap();
    let index = ReadIndex::open(&root).unwrap();
    let original: String = index.connection().query_row("SELECT cursor FROM usage_sources",[],|row|row.get(0)).unwrap();
    index.connection().execute_batch("CREATE TEMP TRIGGER reject_usage BEFORE INSERT ON usage_calls BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    write!(fs::OpenOptions::new().append(true).open(&path).unwrap(),"{}",message("second",4)).unwrap();
    let manifest = crate::session_manager::usage_sources::Manifest { sources: vec![source],issues:vec![],complete:true };
    assert!(refresh(&index,"fixture",manifest,Local::now,&progress::Progress::new(&|_|{})).is_err());
    let cursor: String = index.connection().query_row("SELECT cursor FROM usage_sources",[],|row|row.get(0)).unwrap();
    assert_eq!(cursor,original);
    assert_eq!(index.connection().query_row::<u64,_,_>("SELECT count(*) FROM usage_calls",[],|row|row.get(0)).unwrap(),1);
    assert_eq!(buckets::read(index.connection(),"fixture").unwrap().iter().map(|bucket|bucket.tokens.total).sum::<u64>(),12);
}

#[test]
fn a_calendar_change_reprojects_calls_without_decoding_original_records() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    fs::write(&path,message("first",2)).unwrap();
    let source = fixture_source(&path,AppKind::Claude,"session");
    fixture_reports(temp.path(),vec![source.clone()],vec![],Local::now()).unwrap();
    let index = ReadIndex::open(temp.path()).unwrap();
    index.connection().execute("UPDATE usage_calendar SET zone='another-calendar'",[]).unwrap();
    index.connection().execute("UPDATE usage_buckets SET day='1999-01-01'",[]).unwrap();
    let before = records::decoded_record_count();
    let reports = fixture_reports(temp.path(),vec![source],vec![],Local::now()).unwrap();
    assert_eq!(records::decoded_record_count(),before);
    assert_eq!(all(&reports),12);
    assert_eq!(reports.last().unwrap().days[0].date,parse::local_date("2026-10-22T02:00:00Z").unwrap().to_string());
}

#[test]
fn a_rewrite_during_verification_retries_even_with_the_same_file_stamp() {
    use std::cell::Cell;
    use asb_core::contracts::ModelUsagePhase;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    fs::write(&path,message("first",2)).unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let rewritten = Cell::new(false);
    let publish = |value: asb_core::contracts::ModelUsageProgress| {
        if value.phase == ModelUsagePhase::Verifying && value.total_bytes.is_some() && !rewritten.replace(true) {
            fs::write(&path,message("first",4)).unwrap();
            fs::File::options().write(true).open(&path).unwrap().set_modified(modified).unwrap();
        }
    };
    let delta = stream::read(&path,AppKind::Claude,"session",None,&progress::Progress::new(&publish)).unwrap();
    let ingest::Observation::Claude(candidate) = &delta.observations[0].1 else { panic!("Claude receipt expected"); };
    assert!(rewritten.get());
    assert_eq!(candidate.event.tokens.total,14);
}
