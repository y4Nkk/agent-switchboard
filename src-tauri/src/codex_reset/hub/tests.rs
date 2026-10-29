use super::*;
use serde_json::json;

fn forecast() -> serde_json::Value {
    json!({"updated_at":"2026-10-28T08:58:48.779Z", "confidence":"low",
        "probabilities":{"rounded_24h":46,"rounded_48h":71,
            "range_24h":{"lower":0.298,"upper":0.748},"range_48h":{"lower":0.526,"upper":0.917}},
        "last_reset_at":"2026-10-28T06:35:38.000Z", "age_days":0.1,
        "time_window":{"start_hour":4,"end_hour":7,"timezone":"UTC"},
        "cadence":{"recent_median_days":1.5,"recent_sample":5,"weighted_mean_days":4,"accelerating":true},
        "wait_comparison":{"wait_days":0.1,"sample":8,"shorter_share":0,"median_days":2},
        "context_copy":{"zh":"上面的概率只来自历史。"}})
}

#[test]
fn actual_forecast_wire_names_normalize_into_the_current_internal_contract() {
    let parsed = parse_forecast(&forecast().to_string()).unwrap();
    assert_eq!(parsed.chance_24h.percent, 46);
    assert_eq!(parsed.chance_48h.range_lower, 0.526);
    assert_eq!(parsed.window.as_ref().unwrap().start_hour, 4);
    assert_eq!(parsed.cadence.as_ref().unwrap().sample, 5);
    assert_eq!(parsed.wait.as_ref().unwrap().shorter_share, 0.0);
    let internal = serde_json::to_value(parsed).unwrap();
    assert!(internal.get("updatedAt").is_some());
    assert!(internal.get("chance24h").is_some());
    assert!(internal.get("updated_at").is_none());
}

#[test]
fn actual_timeline_platform_and_feed_wire_names_are_read_without_aliases() {
    let timeline = json!({"updated_at":"2026-10-28T08:41:50.315Z","events":[{
        "id":"2115273253711409208","date":"2026-10-28","type":"reset","summary":"reset",
        "announced_at":"2026-10-28T06:35:38.000Z","confirmed":true,
        "url":"https://x.com/thsottiaux/status/2115273253711409208"}],"stats":{"reset":5,"credits":2}});
    let timeline = parse_timeline(&timeline.to_string()).unwrap();
    assert_eq!(timeline.events[0].kind, HubEventKind::Reset);
    assert_eq!(timeline.resets, 5);
    let platform = json!({"checked_at":"2026-10-28T08:41:49.536Z","stale":false,
        "current":{"surfaces":[{"id":"cli","label":"Codex CLI","status":"operational"}]},
        "incidents":[{"name":"Degraded performance","status":"resolved","impact":"minor",
            "started_at":"2026-10-21T02:26:28.000Z","resolved_at":"2026-10-21T03:28:00.000Z",
            "source_url":"https://status.openai.com/incidents/fixture"}],
        "stats":{"days":90,"incident_count":1,"resolved_incident_count":1,"open_incident_count":0,
            "median_recovery_minutes":61.5}});
    let platform = parse_platform(&platform.to_string()).unwrap();
    assert_eq!(platform.surfaces[0].status, HubSurfaceStatus::Operational);
    assert_eq!(platform.stats.median_recovery_minutes, Some(61.5));
    assert!(platform.incidents[0].resolved);
    let feed = json!({"fetched_at":"2026-10-28T08:39:12.979Z","stale":false,
        "signal":{"at":"2026-10-22T06:55:54.000Z","summary":"reset signal",
            "url":"https://x.com/thsottiaux/status/2113162413008884099","active":false},
        "banked_observations":[{"at":"2026-10-28T06:35:38.000Z","text":"banked"}]});
    let feed = parse_radar(&feed.to_string()).unwrap();
    assert_eq!(feed.banked[0].text, "banked");
    assert!(!feed.signal.as_ref().unwrap().active);
    let status = CodexResetHubStatus {
        source_url: HUB_SOURCE_URL.into(), generated_at: "2026-10-28T08:58:48.779Z".into(),
        checked_at: "2026-10-28T09:00:00Z".into(), source_warning: None,
        forecast: parse_forecast(&forecast().to_string()).unwrap(), timeline, platform, feed,
    };
    status.validate_cached().unwrap();
    let cached = serde_json::to_vec(&status).unwrap();
    serde_json::from_slice::<CodexResetHubStatus>(&cached).unwrap().validate_cached().unwrap();
}

#[test]
fn missing_required_wire_fields_report_the_endpoint_and_actual_field() {
    let mut value = forecast();
    let updated = value.as_object_mut().unwrap().remove("updated_at").unwrap();
    value["updatedAt"] = updated;
    let error = parse_forecast(&value.to_string()).unwrap_err();
    assert!(error.contains("forecast"));
    assert!(error.contains("updated_at"));
    assert!(parse_timeline(r#"{"updatedAt":"2026-10-28T00:00:00Z"}"#).is_err());
    assert!(parse_radar(r#"{"fetchedAt":"2026-10-28T00:00:00Z"}"#).is_err());
}
