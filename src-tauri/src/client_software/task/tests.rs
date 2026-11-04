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
