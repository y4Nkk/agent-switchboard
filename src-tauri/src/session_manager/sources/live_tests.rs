use super::*;
use std::{cell::Cell, io::Write};

#[test]
fn continuous_appends_do_not_remove_a_known_session_from_discovery() {
    let _guard = crate::test_client_paths::redirect_client_paths();
    let root = tempfile::tempdir().unwrap();
    let client = PathBuf::from(std::env::var_os("CODEX_HOME").unwrap());
    let directory = client.join("sessions");
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join("session.jsonl");
    fs::write(&path, "{\"type\":\"session_meta\",\"payload\":{\"id\":\"stable-session\"}}\n").unwrap();
    let appends = Cell::new(0);
    let checkpoint = || {
        appends.set(appends.get() + 1);
        writeln!(fs::OpenOptions::new().append(true).open(&path).unwrap(),
            "{{\"type\":\"event_msg\",\"payload\":{{\"type\":\"task_started\"}}}}").unwrap();
        Ok(())
    };
    let (sessions, issues) = scan_observed(root.path(), Some(AppKind::Codex), None, &checkpoint).unwrap();
    assert!(appends.get() > 2);
    assert_eq!(sessions.len(), 1, "{issues:?}");
    assert_eq!(sessions[0].meta.session_id, "stable-session");
    assert!(!issues.iter().any(|issue| issue.message.contains("来源在读取期间发生变化")));
}
