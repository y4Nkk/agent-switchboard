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
