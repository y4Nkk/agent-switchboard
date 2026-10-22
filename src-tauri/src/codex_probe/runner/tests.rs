use super::*;
use crate::codex_probe::{resolve_question, tests::write_rollout, ProbeRunOutcome, QUESTIONS};
use std::sync::Mutex;

const ID: &str = "11111111-1111-4111-8111-111111111111";

fn idle_session_hook() -> Arc<dyn Fn(&str) + Send + Sync> {
    Arc::new(|_| {})
}

fn exit_status(success: bool) -> ExitStatus {
    #[cfg(windows)]
    { use std::os::windows::process::ExitStatusExt; ExitStatus::from_raw(if success { 0 } else { 1 }) }
    #[cfg(unix)]
    { use std::os::unix::process::ExitStatusExt; ExitStatus::from_raw(if success { 0 } else { 256 }) }
}

fn events() -> String {
    format!(r#"{{"type":"thread.started","thread_id":"{ID}"}}
{{"type":"turn.completed"}}"#)
}

#[test]
fn runtime_budgets_follow_catalog_questions_and_custom_formats() {
    let pelican = resolve_question("pelican-riding-bicycle", None, None, None).unwrap();
    assert_eq!(pelican.timeout_seconds, 900);
    for entry in QUESTIONS.iter().filter(|entry| entry.grading == ProbeGrading::Integer) {
        let question = resolve_question(entry.id, None, None, None).unwrap();
        assert_eq!(question.timeout_seconds, entry.timeout_seconds);
        assert_eq!(question.timeout_seconds, 180);
    }
    for (grading, expected) in [
        (ProbeGrading::Integer, 300), (ProbeGrading::SvgVisual, 600), (ProbeGrading::HtmlVisual, 900),
    ] {
        let answer = (grading == ProbeGrading::Integer).then(|| "45".into());
        let custom = resolve_question("custom", Some("Question".into()), answer, Some(grading)).unwrap();
        assert_eq!(custom.timeout_seconds, expected);
        assert_eq!(custom.timeout_seconds, grading.custom_timeout_seconds());
    }
}

#[test]
fn runner_uses_the_final_answer_file_and_the_emitted_session_id() {
    let _guard = crate::test_client_paths::redirect_client_paths();
    let home = PathBuf::from(std::env::var_os("CODEX_HOME").unwrap());
    write_rollout(&home, ID, 190);
    write_rollout(&home, "22222222-2222-4222-8222-222222222222", 99999);
    #[cfg(windows)]
    let (stub, script) = (home.join("codex-stub.cmd"), format!(
        "@echo off\r\nif not \"%~2\"==\"--json\" exit /b 2\r\nif not \"%~4\"==\"--output-last-message\" exit /b 2\r\nmore >nul\r\n>\"%~5\" echo 45\r\necho {}\r\necho {}\r\nexit /b 0\r\n",
        events().lines().next().unwrap(), r#"{"type":"turn.completed"}"#,
    ));
    #[cfg(unix)]
    let (stub, script) = (home.join("codex-stub.sh"), format!(
        "#!/bin/sh\n[ \"$2\" = '--json' ] || exit 2\n[ \"$4\" = '--output-last-message' ] || exit 2\ncat >/dev/null\nprintf '45\\n' > \"$5\"\ncat <<'EVENTS'\n{}\nEVENTS\n",
        events(),
    ));
    std::fs::write(&stub, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let previous = std::env::var_os("ASB_TEST_CODEX_BIN");
    std::env::set_var("ASB_TEST_CODEX_BIN", &stub);
    let question = resolve_question("divisible-or-contains-3", None, None, None).unwrap();
    let reported: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&reported);
    let hook: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(move |id| {
        sink.lock().unwrap().push(id.to_string());
    });
    let clock = RunProgress::new(1, question.timeout_seconds);
    let deadline = clock.deadline();
    let result = run_once(&question, &AtomicBool::new(false), &hook, &|| Ok(()), &clock);
    match previous {
        Some(value) => std::env::set_var("ASB_TEST_CODEX_BIN", value),
        None => std::env::remove_var("ASB_TEST_CODEX_BIN"),
    }
    let result = result.unwrap();
    assert_eq!(clock.deadline(), deadline);
    assert_eq!(clock.view(false).phase, super::super::progress::ProbeRunPhase::Collecting);
    assert_eq!(result.outcome, ProbeRunOutcome::Passed);
    assert_eq!(result.final_answer.as_deref(), Some("45"));
    assert_eq!(result.session_id.as_deref(), Some(ID));
    // The session id was reported while the CLI was still running.
    assert_eq!(*reported.lock().unwrap(), vec![ID.to_string()]);
    assert_eq!(result.total_tokens, Some(190));
    assert_eq!(result.reasoning_tokens, Some(77));
    assert!(result.error.is_none());
    assert!(result.usage_error.is_none());
}

#[test]
fn only_a_successful_final_answer_passes_regardless_of_diagnostics() {
    let _guard = crate::test_client_paths::redirect_client_paths();
    let home = PathBuf::from(std::env::var_os("CODEX_HOME").unwrap());
    write_rollout(&home, ID, 190);
    let workdir = Workdir::create().unwrap();
    let answer = workdir.0.join("answer.txt");
    let question = resolve_question("divisible-or-contains-3", None, None, None).unwrap();
    for (text, success, timeout, expected) in [
        ("45\n", true, false, ProbeRunOutcome::Passed), ("44", true, false, ProbeRunOutcome::Failed),
        ("-45", true, false, ProbeRunOutcome::Failed), ("45.5", true, false, ProbeRunOutcome::Failed),
        ("45 44", true, false, ProbeRunOutcome::Failed), ("45", false, false, ProbeRunOutcome::Undetermined),
        ("45", true, true, ProbeRunOutcome::Undetermined),
    ] {
        std::fs::write(&answer, text).unwrap();
        let result = collect_result(&question, &answer, &events(), "diagnostic 45",
            exit_status(success), timeout, Instant::now(), None);
        assert_eq!(result.outcome, expected, "{text}, success={success}, timeout={timeout}");
        assert_eq!(result.total_tokens, Some(190));
        if !success || timeout { assert!(result.error.is_some()); }
        if timeout { assert_eq!(result.error.as_deref(), Some("单次运行超时（上限 3 分钟）")); }
    }
}

#[test]
fn timed_out_visual_answers_keep_the_answer_and_the_actual_limit_without_a_verdict() {
    let _guard = crate::test_client_paths::redirect_client_paths();
    let home = PathBuf::from(std::env::var_os("CODEX_HOME").unwrap());
    write_rollout(&home, ID, 190);
    let workdir = Workdir::create().unwrap();
    let answer = workdir.0.join("answer.txt");
    let text = "<!doctype html><html><body><svg></svg></body></html>";
    std::fs::write(&answer, text).unwrap();
    let question = resolve_question("pelican-riding-bicycle", None, None, None).unwrap();
    let result = collect_result(&question, &answer, &events(), "",
        exit_status(false), true, Instant::now(), None);
    assert_eq!(result.outcome, ProbeRunOutcome::Undetermined);
    assert_eq!(result.final_answer.as_deref(), Some(text));
    assert_eq!(result.error.as_deref(), Some("单次运行超时（上限 15 分钟）"));
    assert_eq!(result.session_id.as_deref(), Some(ID));
    assert_eq!(result.total_tokens, Some(190));
}

#[test]
fn missing_answer_or_incomplete_protocol_cannot_pass() {
    let _guard = crate::test_client_paths::redirect_client_paths();
    let home = PathBuf::from(std::env::var_os("CODEX_HOME").unwrap());
    write_rollout(&home, ID, 190);
    let workdir = Workdir::create().unwrap();
    let answer = workdir.0.join("answer.txt");
    let question = resolve_question("divisible-or-contains-3", None, None, None).unwrap();
    let missing = collect_result(&question, &answer, &events(), "45",
        exit_status(true), false, Instant::now(), None);
    assert_eq!(missing.outcome, ProbeRunOutcome::Undetermined);
    assert!(missing.error.is_some());
    std::fs::write(&answer, "45").unwrap();
    for output in ["45".to_string(), format!(r#"{{"type":"thread.started","thread_id":"{ID}"}}"#),
        format!("{}\n{}", events(), r#"{"type":"turn.failed","error":{"message":"failed"}}"#)] {
        let result = collect_result(&question, &answer, &output, "45",
            exit_status(true), false, Instant::now(), None);
        assert_eq!(result.outcome, ProbeRunOutcome::Undetermined);
        assert!(result.error.is_some());
    }
}

#[test]
fn missing_usage_stays_unknown_without_changing_the_answer_verdict() {
    let _guard = crate::test_client_paths::redirect_client_paths();
    let workdir = Workdir::create().unwrap();
    let answer = workdir.0.join("answer.txt");
    std::fs::write(&answer, "45").unwrap();
    let question = resolve_question("divisible-or-contains-3", None, None, None).unwrap();
    let result = collect_result(&question, &answer, &events(), "",
        exit_status(true), false, Instant::now(), None);
    assert_eq!(result.outcome, ProbeRunOutcome::Passed);
    assert_eq!(result.total_tokens, None);
    assert_eq!(result.reasoning_tokens, None);
    assert_eq!(result.model, None);
    assert!(result.usage_error.is_some());
}

#[test]
fn visual_answers_are_preserved_without_an_automatic_verdict() {
    let _guard = crate::test_client_paths::redirect_client_paths();
    let workdir = Workdir::create().unwrap();
    let answer = workdir.0.join("answer.txt");
    let svg = "  ```svg\n<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"10\"/></svg>\n```\n";
    let html = "  ```html\n<!doctype html><html><head><style>svg{width:100%}</style></head><body><svg></svg><script>requestAnimationFrame(()=>{});</script></body></html>\n```\n";
    for question in [
        resolve_question("custom", Some("画一幅 SVG".into()), None, Some(ProbeGrading::SvgVisual)).unwrap(),
        resolve_question("pelican-riding-bicycle", None, None, None).unwrap(),
    ] {
        for text in [svg, html, "45", "not visual markup"] {
            std::fs::write(&answer, text).unwrap();
            let result = collect_result(&question, &answer, &events(), "",
                exit_status(true), false, Instant::now(), None);
            assert_eq!(result.outcome, ProbeRunOutcome::Undetermined);
            assert_eq!(result.final_answer.as_deref(), Some(text));
            assert!(result.error.is_none());
        }
        let failed = collect_result(&question, &answer, &events(), "execution failed",
            exit_status(false), false, Instant::now(), None);
        assert_eq!(failed.outcome, ProbeRunOutcome::Undetermined);
        assert!(failed.error.is_some());
    }
}

#[test]
fn a_broken_stdout_still_reports_the_early_session_id() {
    // The streaming capture saved a session id, but the strict full-output
    // parse fails; the run stays undetermined with the id preserved.
    let _guard = crate::test_client_paths::redirect_client_paths();
    let workdir = Workdir::create().unwrap();
    let answer = workdir.0.join("answer.txt");
    std::fs::write(&answer, "45").unwrap();
    let question = resolve_question("divisible-or-contains-3", None, None, None).unwrap();
    let broken = format!("not json{{\"type\":\"thread.started\",\"thread_id\":\"{ID}\"}}");
    let result = collect_result(&question, &answer, &broken, "",
        exit_status(true), false, Instant::now(), Some(ID));
    assert_eq!(result.outcome, ProbeRunOutcome::Undetermined);
    assert_eq!(result.session_id.as_deref(), Some(ID));
}

#[test]
fn session_ids_are_extracted_only_from_thread_started_lines() {
    assert_eq!(session_id_in_line(&format!(r#"{{"type":"thread.started","thread_id":"{ID}"}}"#)),
        Some(ID.to_string()));
    assert_eq!(session_id_in_line(r#"{"type":"turn.completed"}"#), None);
    assert_eq!(session_id_in_line(r#"{"type":"thread.started","thread_id":"not-a-uuid"}"#), None);
    assert_eq!(session_id_in_line("45"), None);
    assert_eq!(session_id_in_line(""), None);
}

#[test]
fn cancelled_before_start_never_launches_a_process() {
    let question = resolve_question("divisible-or-contains-3", None, None, None).unwrap();
    assert!(matches!(
        run_once(&question, &AtomicBool::new(true), &idle_session_hook(), &|| Ok(()),
            &RunProgress::new(1, question.timeout_seconds)),
        Err(ProbeAbort::Cancelled)
    ));
}

#[test]
fn temporary_outputs_are_removed_when_the_workdir_is_dropped() {
    let path = {
        let workdir = Workdir::create().unwrap();
        std::fs::write(workdir.0.join("answer.txt"), "45").unwrap();
        workdir.0.clone()
    };
    assert!(!path.exists());
}

#[test]
fn configuration_change_after_process_exit_still_prevents_grading() {
    let _guard = crate::test_client_paths::redirect_client_paths();
    #[cfg(windows)]
    let mut command = { let mut command = Command::new("cmd"); command.args(["/C", "exit", "0"]); command };
    #[cfg(unix)]
    let mut command = { let mut command = Command::new("sh"); command.args(["-c", "exit 0"]); command };
    let (mut child, lifetime) = super::super::process::spawn(&mut command).unwrap();
    child.wait().unwrap();
    let result = wait_for_exit(&mut child, &AtomicBool::new(false), Instant::now() + Duration::from_secs(1),
        &|| Err(ProbeAbort::ConfigChanged));
    drop(lifetime);
    assert!(matches!(result, Err(ProbeAbort::ConfigChanged)));
}
