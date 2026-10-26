use super::*;
use asb_core::contracts::ModelUsageRange;
use chrono::TimeZone;

#[test]
fn a_snapshot_contains_every_calendar_range_and_returns_only_the_requested_one() {
    let temporary = tempfile::tempdir().unwrap();
    let reports = crate::model_usage::reports(temporary.path(), Vec::new(), Vec::new(), Local::now()).unwrap();
    assert_eq!(reports.len(), 4);
    let cached_at = Utc::now().to_rfc3339();
    let value = read(Snapshot { cached_at, reports }, ModelUsageRequest {
        range: ModelUsageRange::Last30Days, force_refresh: false,
    }, ModelUsageFreshness::Cached).unwrap();
    assert_eq!(value.report.range, ModelUsageRange::Last30Days);
    assert_eq!(value.freshness, ModelUsageFreshness::Cached);
}

#[test]
fn refresh_deadline_never_crosses_the_local_midnight() {
    let before = Local.with_ymd_and_hms(2026, 10, 22, 23, 59, 30).single().unwrap();
    let after = DateTime::parse_from_rfc3339(&refresh_after(&before.to_rfc3339()).unwrap()).unwrap().with_timezone(&Local);
    assert_eq!(after.date_naive(), before.date_naive().succ_opt().unwrap());
    assert_eq!(after.time(), chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
}

#[test]
fn the_source_revision_changes_with_file_append_or_deletion() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    std::fs::write(&path, "first\n").unwrap();
    let source = crate::session_manager::fixture_source(&path, asb_core::contracts::AppKind::Codex, "session");
    let first = source_revision(temp.path(), &[source.clone()], &[]).unwrap();
    std::fs::write(&path, "first\nsecond\n").unwrap();
    let second = source_revision(temp.path(), &[source.clone()], &[]).unwrap();
    assert_ne!(first, second);
    std::fs::remove_file(&path).unwrap();
    assert_ne!(second, source_revision(temp.path(), &[source], &[]).unwrap());
}

#[test]
fn changing_sources_return_verified_totals_and_an_early_refresh_without_publishing_a_stale_cache() {
    use asb_core::contracts::AppKind;
    use std::io::Write;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("session.jsonl");
    let entry = serde_json::json!({"type":"assistant","timestamp":"2026-10-22T02:00:00Z",
        "message":{"id":"first","model":"model","usage":{"input_tokens":10,"output_tokens":2}}});
    std::fs::write(&path, format!("{entry}\n")).unwrap();
    let source = crate::session_manager::fixture_source(&path, AppKind::Claude, "session");
    let revision = source_revision(temp.path(), &[source.clone()], &[]).unwrap();
    let reports = crate::model_usage::reports(temp.path(), vec![source.clone()], vec![], Local::now()).unwrap();
    writeln!(std::fs::File::options().append(true).open(&path).unwrap(), "{{\"type\":\"user\"}}").unwrap();
    let current_revision = source_revision(temp.path(), &[source], &[]).unwrap();
    assert_ne!(revision, current_revision);
    let index = ReadIndex::open(temp.path()).unwrap();
    let snapshot = Snapshot { cached_at: Utc::now().to_rfc3339(), reports };
    let reading = finish_refresh(&index, "fixture", &revision, &current_revision, snapshot,
        ModelUsageRequest { range: ModelUsageRange::All, force_refresh: true }).unwrap();
    assert_eq!(reading.report.groups[0].total_tokens, 12);
    assert_eq!(reading.freshness, ModelUsageFreshness::Fresh);
    let refresh = DateTime::parse_from_rfc3339(&reading.refresh_after).unwrap().with_timezone(&Utc);
    assert!(refresh <= Utc::now() + Duration::seconds(10));
    assert!(index.get::<Snapshot>("usage-reports", "fixture", &revision).unwrap().is_none());
}
