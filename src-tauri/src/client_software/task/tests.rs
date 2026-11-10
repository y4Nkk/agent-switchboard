use super::*;
use crate::client_software::{process::Chunk, types::{Manager, UpdatePlan}};
use crate::outbound_proxy::{OutboundProxyMode, OutboundProxySettings};
use asb_core::contracts::AppKind;

fn fixture() -> UpdateTask {
    UpdateTask { id: uuid::Uuid::new_v4().to_string(), revision: 1, phase: Phase::Preparing,
        started_at: "2026-11-04T00:00:00Z".into(), finished_at: None, actual_version: None,
        cancellable: true, log: Vec::new(), error: None, process_id: None, process_birth: None,
        plan: UpdatePlan { id: uuid::Uuid::new_v4().to_string(), installation_id: "fixture".into(),
            app: AppKind::Codex, launcher: "fixture/codex".into(), manager: Manager::Native,
            from_version: "0.165.0".into(), target_version: "0.166.0".into(), exact_target: true,
            channel: "latest".into(), proxy_mode: "none".into(), expires_at: "2026-11-04T00:05:00Z".into() } }
}

#[test]
fn journal_replaces_atomically_and_rejects_foreign_or_invalid_records() {
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().into());
    assert!(journal::read(&state).unwrap().is_none());
    let mut task = fixture();
    journal::save(&state, &task).unwrap();
    task.phase = Phase::Verifying; task.revision += 1;
    journal::save(&state, &task).unwrap();
    let saved = journal::read(&state).unwrap().unwrap();
    assert_eq!(saved.phase, Phase::Verifying);
    assert_eq!(saved.revision, 2);
    assert!(!directory.path().join("client-software/operation.tmp").exists());
    let path = directory.path().join("client-software/operation.json");
    std::fs::write(&path, serde_json::to_vec(&serde_json::json!({"version": 99, "operation": task})).unwrap()).unwrap();
    assert!(journal::read(&state).is_err());
    std::fs::write(&path, b"{broken").unwrap();
    assert!(journal::read(&state).is_err());
    std::fs::write(&path, vec![b' '; 262_145]).unwrap();
    assert!(journal::read(&state).is_err());
}

#[test]
fn obsolete_journals_are_rejected_without_changing_the_original_record() {
    for (phase, diagnostic) in [(Phase::Preparing, None), (Phase::Failed, Some("historical update diagnostic")),
        (Phase::NeedsAttention, Some("Cannot verify process 42: EnumerationRestricted"))] {
        let directory = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(directory.path().into());
        let mut expected = fixture();
        expected.phase = phase;
        expected.revision = 7;
        expected.process_id = Some(42);
        expected.process_birth = Some("123456789".into());
        journal::save(&state, &expected).unwrap();
        let path = directory.path().join("client-software/operation.json");
        let mut old = serde_json::json!({ "version": 1, "operation": &expected });
        old["operation"]["error"] = serde_json::json!(diagnostic);
        std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(journal::read(&state).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn invalid_journal_error_shapes_are_rejected_without_overwriting_the_record() {
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().into());
    let task = fixture();
    journal::save(&state, &task).unwrap();
    let path = directory.path().join("client-software/operation.json");
    for (version, error) in [(1, serde_json::json!(42)), (1, serde_json::json!({})),
        (2, serde_json::json!("obsolete text error"))] {
        let mut value = serde_json::json!({ "version": version, "operation": &task });
        value["operation"]["error"] = error;
        let bytes = serde_json::to_vec(&value).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        assert!(journal::read(&state).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn a_background_occupancy_failure_is_preserved_as_a_structured_task_error() {
    for app in [AppKind::Codex, AppKind::Claude] {
        let directory = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(directory.path().into());
        let manager = SoftwareManager::default();
        let mut task = fixture();
        task.plan.app = app;
        manager.lock().unwrap().task = Some(task);
        finish(&manager, &state, Err(SoftwareError::ProcessInUse { app, pid: 42 })).unwrap();
        let restored = journal::read(&state).unwrap().unwrap();
        assert_eq!(restored.plan.app, app);
        assert_eq!(restored.phase, Phase::Failed);
        assert!(!restored.cancellable);
        assert!(restored.finished_at.is_some());
        let error = restored.error.unwrap();
        assert_eq!(error.key, "software.processInUse");
        assert_eq!(error.params, Some(serde_json::json!({ "app": app.label(), "pid": 42 })));
        let value: serde_json::Value = serde_json::from_slice(
            &std::fs::read(directory.path().join("client-software/operation.json")).unwrap()).unwrap();
        assert_eq!(value["version"], 2);
        assert!(value["operation"]["error"].is_object());
    }
}

#[test]
fn oversized_structured_records_never_replace_a_readable_journal() {
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().into());
    let mut task = fixture();
    journal::save(&state, &task).unwrap();
    let path = directory.path().join("client-software/operation.json");
    let saved = std::fs::read(&path).unwrap();
    task.log = vec!["x".repeat(262_144)];
    assert!(journal::save(&state, &task).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    assert!(journal::read(&state).unwrap().is_some());
}

#[test]
fn cancellation_cannot_cross_the_installer_handoff_and_changes_advance_revisions() {
    let directory = tempfile::tempdir().unwrap();
    let state = LocalState::from_root(directory.path().into());
    let manager = SoftwareManager::default();
    let mut task = fixture(); let id = task.id.clone();
    manager.lock().unwrap().task = Some(task.clone());
    cancel(&manager, &id).unwrap();
    assert!(manager.cancel.load(Ordering::SeqCst));
    manager.cancel.store(false, Ordering::SeqCst);
    task.phase = Phase::RunningUpdater; task.cancellable = false;
    manager.lock().unwrap().task = Some(task);
    cancel(&manager, &id).unwrap();
    assert!(!manager.cancel.load(Ordering::SeqCst));
    change(&manager, &state, |task| task.phase = Phase::Verifying).unwrap();
    assert_eq!(journal::read(&state).unwrap().unwrap().revision, 2);
}

#[test]
fn separate_output_streams_redact_split_credentials_before_persistence() {
    let mut task = fixture(); let mut output = output::Output::default();
    let proxy = OutboundProxySettings { mode: OutboundProxyMode::Manual,
        url: Some("http://alice:s%65cret@proxy.invalid:8080".into()), ..Default::default() };
    output.push(Chunk { stderr: false, bytes: b"connecting http://alice:s%65".to_vec() }, &mut task, &proxy);
    output.push(Chunk { stderr: true, bytes: b"progress\n".to_vec() }, &mut task, &proxy);
    output.push(Chunk { stderr: false, bytes: b"cret@proxy.invalid:8080 password secret\n".to_vec() }, &mut task, &proxy);
    assert!(task.log.iter().any(|line| line == "progress"));
    assert!(!task.log.join("\n").contains("secret"));
    assert!(!task.log.join("\n").contains("s%65cret"));
    for _ in 0..120 { output.push(Chunk { stderr: false, bytes: b"progress\n".to_vec() }, &mut task, &proxy); }
    output.push(Chunk { stderr: true, bytes: vec![b'a'; 20_000] }, &mut task, &proxy);
    output.finish(&mut task, &proxy);
    assert!(task.log.len() <= 100);
    assert!(task.log.iter().all(|line| line.len() <= 512));
}

#[test]
fn failed_identity_commit_never_starts_the_installer() {
    let directory = tempfile::tempdir().unwrap();
    let marker = directory.path().join("installer-started.txt");
    #[cfg(windows)]
    let mut command = {
        let mut command = std::process::Command::new("powershell.exe");
        command.args(["-NoProfile", "-NonInteractive", "-Command", "[IO.File]::WriteAllText($env:ASB_TEST_MARKER, 'started')"]);
        command
    };
    #[cfg(unix)]
    let mut command = {
        let mut command = std::process::Command::new("/bin/sh");
        command.args(["-c", "printf started > \"$ASB_TEST_MARKER\""]);
        command
    };
    command.env("ASB_TEST_MARKER", &marker);
    let result = spawn::recorded(&mut command, |_, _| Err("fixture journal rejected".into()));
    assert!(result.is_err());
    assert!(!marker.exists());
}

#[cfg(windows)]
#[test]
fn windows_updater_preserves_arguments_streams_and_exit_status() {
    let script = r#"
        if ($null -ne [Console]::In.ReadLine()) { exit 99 }
        [Console]::Out.WriteLine('stdout: two "quotes" C:\path with spaces\')
        [Console]::Error.WriteLine('stderr: installer diagnostic')
        [IO.File]::WriteAllText($env:ASB_TEST_MARKER, 'started')
        $status = [int]$env:ASB_TEST_EXIT
        exit $status
    "#;
    let system = std::env::var_os("SystemRoot").unwrap();
    for status in [0, 23] {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("installer started.txt");
        let mut command = std::process::Command::new(std::path::PathBuf::from(&system)
            .join("System32/WindowsPowerShell/v1.0/powershell.exe"));
        command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", script])
            .env("ASB_TEST_MARKER", &marker).env("ASB_TEST_EXIT", status.to_string());
        let mut recorded = false;
        let child = spawn::recorded(&mut command, |_, _| { recorded = true; Ok(()) }).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(recorded);
        assert_eq!(output.status.code(), Some(status));
        assert_eq!(std::fs::read_to_string(marker).unwrap(), "started");
        assert!(String::from_utf8_lossy(&output.stdout).contains("stdout: two \"quotes\" C:\\path with spaces\\"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("stderr: installer diagnostic"));
    }
}

#[cfg(windows)]
#[test]
fn windows_updater_finishes_after_its_output_receiver_disconnects() {
    let directory = tempfile::tempdir().unwrap();
    let marker = directory.path().join("installer finished.txt");
    let system = std::env::var_os("SystemRoot").unwrap();
    let mut command = std::process::Command::new(std::path::PathBuf::from(system)
        .join("System32/WindowsPowerShell/v1.0/powershell.exe"));
    let script = r#"
        $chunk = 'x' * 8192
        for ($index = 0; $index -lt 64; $index++) {
            [Console]::Out.WriteLine($chunk)
            [Console]::Error.WriteLine($chunk)
        }
        [IO.File]::WriteAllText($env:ASB_TEST_MARKER, 'finished')
        exit 23
    "#;
    command.args(["-NoProfile", "-NonInteractive", "-Command", script]).env("ASB_TEST_MARKER", &marker);
    let mut child = spawn::recorded(&mut command, |_, _| Ok(())).unwrap();
    drop(child.stdout.take());
    drop(child.stderr.take());
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() { break status; }
        if std::time::Instant::now() >= deadline {
            crate::process_control::terminate_process_tree(&mut child);
            panic!("fixture installer blocked after output receiver disconnected");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(23));
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "finished");
}
