use super::{snapshot, ConfigStore};

struct DiagnosticLogger;
impl log::Log for DiagnosticLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool { metadata.target() == "agent-switchboard.runtime" }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) { println!("{}", record.args()); }
    }
    fn flush(&self) {}
}
static DIAGNOSTIC_LOGGER: DiagnosticLogger = DiagnosticLogger;

#[test]
fn native_error_feedback_preserves_causes_and_configuration_bytes() {
    use asb_core::contracts::{AppKind, ModelOptions};
    use crate::commands::error::store_error;
    let directory = tempfile::tempdir().unwrap();
    let store = ConfigStore::new(directory.path().join("state"));
    let path = store.client_settings_path(AppKind::Codex);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut errors = Vec::new();
    for (text, expected) in [("{}", "missing field `settings`"),
        (r#"{"settings":"private-credential-value"}"#, "invalid type: string"),
        (r#"{"settings" {}}"#, "expected `:`"),
        (r#"{"settings": {"#, "EOF while parsing an object"),
        (r#"{"settings":{},"legacyMode":true}"#, "unknown field `legacyMode`")] {
        std::fs::write(&path, text).unwrap();
        let error = store_error(store.get_client_settings(AppKind::Codex).unwrap_err());
        assert_eq!(error.code, "profile-store-unsupported");
        assert!(error.message.contains(expected), "{}", error.message);
        assert!(error.message.contains(&path.display().to_string()));
        assert!(!error.message.contains("private-credential-value"));
        assert_eq!(std::fs::read(&path).unwrap(), text.as_bytes());
        let repair_error = store.repair().unwrap_err();
        assert!(repair_error.contains(expected), "{repair_error}");
        assert!(!repair_error.contains("private-credential-value"));
        assert_eq!(std::fs::read(&path).unwrap(), text.as_bytes());
        errors.push(error);
    }
    let path = store.providers_dir(AppKind::Claude).join("00000000-0000-0000-0000-000000000001.json");
    let error = super::error::parse_file::<ModelOptions>(r#"{"kind":"claude","primaryOneM":false}"#, &path).unwrap_err();
    let error = store_error(error);
    assert!(error.message.contains("missing field `routes`"));
    errors.push(error);
    let request = serde_json::from_value(serde_json::json!({
        "query": { "kind": "script", "source": "(() => { while (true) {} })()", "refreshIntervalMinutes": 0 },
        "apiKey": "isolated-key", "baseUrl": null, "upstreamProtocol": "responses", "authentication": null,
    })).unwrap();
    let error = tauri::async_runtime::block_on(crate::commands::test_usage_query(request)).unwrap_err();
    assert_eq!(error.code, "usage-query-failed");
    assert!(error.message.contains("250 毫秒限额"));
    errors.push(error);
    println!("configuration-error-feedback:{}", serde_json::to_string(&errors).unwrap());
}

#[test]
#[ignore = "manual read-only inspection of an isolated configuration copy"]
fn inspect_configuration_copy() {
    let sandbox = std::path::PathBuf::from(std::env::var_os("ASB_PREPARATION_SANDBOX").expect("isolated sandbox"));
    assert!(sandbox.join(".asb-preparation-sandbox").is_file());
    let store = ConfigStore::new(sandbox.join("state"));
    match snapshot::read_configuration_snapshot(&store) {
        Ok(snapshot) => {
            snapshot::validate_snapshot(&snapshot).expect("configuration snapshot validation");
            println!("Current configuration valid: {} providers", snapshot.provider_count());
        }
        Err(error) => panic!("Current configuration rejected: {error}"),
    }
}

#[test]
#[ignore = "manual measurement of read determinism during isolated process scheduling pauses"]
fn measure_configuration_reads() {
    use std::{fs, path::PathBuf, time::{Duration, Instant}};
    log::set_logger(&DIAGNOSTIC_LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Debug);
    let sandbox = PathBuf::from(std::env::var_os("ASB_PREPARATION_SANDBOX").expect("isolated sandbox"));
    assert!(sandbox.join(".asb-preparation-sandbox").is_file());
    let store = ConfigStore::new(sandbox.join("state"));
    let original = store.data_revision().unwrap();
    let before = snapshot::read_configuration_snapshot(&store).expect("valid starting snapshot");
    snapshot::validate_snapshot(&before).unwrap();
    fs::write(sandbox.join("read-ready"), b"ready").unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut reads = 0;
    let mut failures = Vec::new();
    while Instant::now() < deadline {
        if let Err(error) = snapshot::read_configuration_snapshot(&store) {
            let command = crate::commands::error::store_error(error);
            failures.push(serde_json::json!({ "code": command.code, "message": command.message }));
            if failures.len() == 3 { break; }
        }
        reads += 1;
    }
    let unchanged = store.data_revision().unwrap() == original;
    fs::write(sandbox.join("read-result.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "reads": reads, "failures": failures, "configurationUnchanged": unchanged,
    })).unwrap()).unwrap();
    assert!(unchanged);
}
