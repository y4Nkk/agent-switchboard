use super::*;
use asb_core::contracts::ModelUsageRange;
use chrono::TimeZone;

fn empty_reports(root: &Path) -> Vec<ModelUsageReport> {
    crate::model_usage::fixture_reports(root, vec![], vec![], Local::now()).unwrap()
}

#[test]
fn an_expired_snapshot_reads_without_discovering_sources_or_running_a_task() {
    let temp = tempfile::tempdir().unwrap();
    let index = ReadIndex::open(temp.path()).unwrap();
    assert!(read_snapshot(&index, "fixture").unwrap().is_none());
    let reports = empty_reports(temp.path());
    let at = (Utc::now() - Duration::days(2)).to_rfc3339();
    index.connection().execute("INSERT INTO usage_snapshot VALUES(?1,?2,?3)", params!["fixture",at,key(&reports).unwrap()]).unwrap();
    let read = read_snapshot(&index, "fixture").unwrap().unwrap();
    assert_eq!(read.reports, reports);
    assert_eq!(read.freshness, ModelUsageFreshness::Cached);
    assert!(read_snapshot(&index, "another-source").unwrap().is_none());
}

#[test]
fn malformed_or_incomplete_summaries_are_not_exposed_as_current_readings() {
    let temp = tempfile::tempdir().unwrap();
    let mut reports = empty_reports(temp.path());
    reports.pop();
    assert!(reading(reports, &Utc::now().to_rfc3339(), ModelUsageFreshness::Cached, MODEL_USAGE_REFRESH_INTERVAL).is_err());
    let index = ReadIndex::open(temp.path()).unwrap();
    index.connection().execute("INSERT INTO usage_snapshot VALUES('fixture','invalid','{')", []).unwrap();
    assert!(read_snapshot(&index,"fixture").is_err());
}

#[test]
fn refresh_deadline_respects_midnight_and_the_short_catchup_interval() {
    let now = Local.with_ymd_and_hms(2026,10,22,23,59,30).single().unwrap();
    let next = DateTime::parse_from_rfc3339(&refresh_deadline(&now.to_rfc3339(), MODEL_USAGE_REFRESH_INTERVAL).unwrap()).unwrap().with_timezone(&Local);
    assert_eq!(next.date_naive(), now.date_naive().succ_opt().unwrap());
    assert_eq!(next.time(), chrono::NaiveTime::from_hms_opt(0,0,0).unwrap());
    let short = DateTime::parse_from_rfc3339(&refresh_deadline(&now.to_rfc3339(),Duration::seconds(10)).unwrap()).unwrap();
    assert_eq!(short.signed_duration_since(now),Duration::seconds(10));
}

#[test]
fn all_ranges_are_published_as_one_read() {
    let temp = tempfile::tempdir().unwrap();
    let read = reading(empty_reports(temp.path()), &Utc::now().to_rfc3339(), ModelUsageFreshness::Fresh, MODEL_USAGE_REFRESH_INTERVAL).unwrap();
    assert_eq!(read.reports.iter().map(|report| report.range).collect::<Vec<_>>(),
        vec![ModelUsageRange::Today,ModelUsageRange::Last7Days,ModelUsageRange::Last30Days,ModelUsageRange::All]);
}
