use super::*;
use std::fs;

fn entry(at: &str, level: RuntimeLogSeverity, action: RuntimeLogAction) -> RuntimeLogEntry {
    RuntimeLogEntry {
        at: at.to_string(),
        level,
        action,
        error_code: (level == RuntimeLogSeverity::Error)
            .then(|| "operation-failed".to_string()),
    }
}

#[test]
fn log_levels_apply_standard_thresholds() {
    assert!(RuntimeLogLevel::Debug.allows(RuntimeLogSeverity::Debug));
    assert!(RuntimeLogLevel::Debug.allows(RuntimeLogSeverity::Info));
    assert!(RuntimeLogLevel::Debug.allows(RuntimeLogSeverity::Warn));
    assert!(RuntimeLogLevel::Debug.allows(RuntimeLogSeverity::Error));

    assert!(!RuntimeLogLevel::Info.allows(RuntimeLogSeverity::Debug));
    assert!(RuntimeLogLevel::Info.allows(RuntimeLogSeverity::Info));
    assert!(RuntimeLogLevel::Info.allows(RuntimeLogSeverity::Warn));
    assert!(RuntimeLogLevel::Info.allows(RuntimeLogSeverity::Error));

    assert!(!RuntimeLogLevel::Warn.allows(RuntimeLogSeverity::Debug));
    assert!(!RuntimeLogLevel::Warn.allows(RuntimeLogSeverity::Info));
    assert!(RuntimeLogLevel::Warn.allows(RuntimeLogSeverity::Warn));
    assert!(RuntimeLogLevel::Warn.allows(RuntimeLogSeverity::Error));

    assert!(!RuntimeLogLevel::Error.allows(RuntimeLogSeverity::Debug));
    assert!(!RuntimeLogLevel::Error.allows(RuntimeLogSeverity::Info));
    assert!(!RuntimeLogLevel::Error.allows(RuntimeLogSeverity::Warn));
    assert!(RuntimeLogLevel::Error.allows(RuntimeLogSeverity::Error));

    assert!(!RuntimeLogLevel::Silent.allows(RuntimeLogSeverity::Debug));
    assert!(!RuntimeLogLevel::Silent.allows(RuntimeLogSeverity::Info));
    assert!(!RuntimeLogLevel::Silent.allows(RuntimeLogSeverity::Warn));
    assert!(!RuntimeLogLevel::Silent.allows(RuntimeLogSeverity::Error));
}

#[test]
fn reads_current_and_rotated_files_in_reverse_time_order() {
    let directory = tempfile::tempdir().expect("temporary log directory");
    let older = entry(
        "2026-08-31T01:02:03.000Z",
        RuntimeLogSeverity::Info,
        RuntimeLogAction::AppStarted,
    );
    let newer = entry(
        "2026-08-31T02:03:04.000Z",
        RuntimeLogSeverity::Error,
        RuntimeLogAction::ConfigurationSwitched,
    );
    fs::write(
        directory
            .path()
            .join("agent-switchboard_2026-08-31_01-00-00.log"),
        serde_json::to_string(&older).expect("serialize older"),
    )
    .expect("write archived log");
    fs::write(
        directory.path().join("agent-switchboard.log"),
        serde_json::to_string(&newer).expect("serialize newer"),
    )
    .expect("write current log");

    let entries = list_from_dir(directory.path(), None).expect("read logs").entries;
    assert_eq!(entries, vec![newer, older]);
}

#[test]
fn reads_all_event_severities_except_silent() {
    let directory = tempfile::tempdir().expect("temporary log directory");
    let debug = entry(
        "2026-08-31T01:02:03.000Z",
        RuntimeLogSeverity::Debug,
        RuntimeLogAction::AppStarted,
    );
    let warn = entry(
        "2026-08-31T02:03:04.000Z",
        RuntimeLogSeverity::Warn,
        RuntimeLogAction::ApplicationDataRepaired,
    );
    fs::write(
        directory.path().join("agent-switchboard.log"),
        format!(
            "{}\n{}\n{{\"at\":\"2026-08-31T03:04:05.000Z\",\"level\":\"silent\",\"action\":\"appStarted\"}}",
            serde_json::to_string(&debug).expect("serialize debug"),
            serde_json::to_string(&warn).expect("serialize warn"),
        ),
    )
    .expect("write log entries");

    let entries = list_from_dir(directory.path(), None).expect("read logs").entries;
    assert_eq!(entries, vec![warn, debug]);
}

#[test]
fn ignores_malformed_and_non_contract_log_lines() {
    let directory = tempfile::tempdir().expect("temporary log directory");
    fs::write(
        directory.path().join("agent-switchboard.log"),
        concat!(
            "not json\n",
            "{\"at\":\"2026-08-31T01:02:03.000Z\",\"level\":\"info\",\"action\":\"appStarted\",\"message\":\"sk-secret\"}\n",
            "{\"at\":\"bad-time\",\"level\":\"error\",\"action\":\"appStarted\",\"errorCode\":\"operation-failed\"}\n"
        ),
    )
    .expect("write malformed lines");

    let result = list_from_dir(directory.path(), None).expect("read logs");
    assert!(result.entries.is_empty());
    assert_eq!(result.skipped, 3);
}

#[test]
fn retains_only_the_newest_display_entries() {
    let directory = tempfile::tempdir().expect("temporary log directory");
    let path = directory.path().join("agent-switchboard.log");
    let lines = (0..=MAX_VISIBLE_ENTRIES)
        .map(|index| {
            serde_json::to_string(&entry(
                &format!(
                    "2026-08-31T00:{:02}:{:02}.000Z",
                    (index / 60) % 60,
                    index % 60
                ),
                RuntimeLogSeverity::Info,
                RuntimeLogAction::ProfileCreated,
            ))
            .expect("serialize entry")
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(path, lines).expect("write log entries");

    let entries = list_from_dir(directory.path(), None).expect("read logs").entries;
    assert_eq!(entries.len(), MAX_VISIBLE_ENTRIES);
    assert_eq!(entries[0].at, "2026-08-31T00:08:20.000Z");
    assert_eq!(
        entries.last().map(|entry| entry.at.as_str()),
        Some("2026-08-31T00:00:01.000Z")
    );
}

#[test]
fn level_filter_finds_errors_older_than_the_latest_five_hundred_other_entries() {
    let directory = tempfile::tempdir().unwrap();
    let error = entry("2026-08-30T00:00:00.000Z", RuntimeLogSeverity::Error, RuntimeLogAction::AppStarted);
    let info = entry("2026-08-31T00:00:00.000Z", RuntimeLogSeverity::Info, RuntimeLogAction::AppStarted);
    let mut records = vec![serde_json::to_string(&error).unwrap()];
    records.extend(std::iter::repeat_n(serde_json::to_string(&info).unwrap(), 800));
    fs::write(directory.path().join("agent-switchboard.log"), records.join("\n")).unwrap();
    let filtered = list_from_dir(directory.path(), Some(RuntimeLogSeverity::Error)).unwrap();
    assert_eq!(filtered.total, 1);
    assert_eq!(filtered.entries, vec![error]);
    let all = list_from_dir(directory.path(), None).unwrap();
    assert_eq!(all.total, 801);
    assert_eq!(all.entries.len(), MAX_VISIBLE_ENTRIES);
}
