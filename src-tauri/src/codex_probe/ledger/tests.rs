use super::*;

fn temp_ledger() -> (tempfile::TempDir, ProbeLedger) {
    let directory = tempfile::tempdir().unwrap();
    let ledger = ProbeLedger::new(directory.path());
    (directory, ledger)
}

fn batch(id: &str, started_at: &str, profile: Option<(&str, &str)>) -> NewProbeBatch {
    NewProbeBatch {
        id: id.to_string(),
        started_at: started_at.to_string(),
        planned_runs: 3,
        question: ProbeQuestionSnapshot {
            id: "divisible-or-contains-3".to_string(),
            label: "计数题".to_string(),
            text: "题目全文".to_string(),
            expected_answer: "45".to_string(),
        },
        grading_version: crate::codex_probe::ProbeGrading::Integer,
        cli_version: Some("codex-cli 0.5.0".to_string()),
        config: ProbeConfigRecord {
            profile_id: profile.map(|(id, _)| id.to_string()),
            profile_name: profile.map(|(_, name)| name.to_string()),
            fingerprint: "fingerprint-a".to_string(),
            ..Default::default()
        },
    }
}

fn passed_run(seq: u32, total: Option<u64>) -> ProbeRunRecord {
    ProbeRunRecord {
        seq,
        status: ProbeRunStatus::Passed,
        session_id: Some(format!("11111111-1111-4111-8111-11111111{seq:04}")),
        final_answer: Some("45".to_string()),
        reported_model: Some("gpt-test".to_string()),
        duration_ms: Some(1_200),
        reasoning_tokens: Some(64),
        total_tokens: total,
        execution_error: None,
        usage_error: None,
    }
}

fn rfc3339(days_ago: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::days(days_ago))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[test]
fn batches_and_runs_round_trip_with_snapshots() {
    let (_directory, ledger) = temp_ledger();
    ledger.insert_batch(&batch("b1", &rfc3339(0), Some(("p1", "主供应商")))).unwrap();
    ledger.start_run("b1", 1).unwrap();
    ledger.save_run_session("b1", 1, "11111111-1111-4111-8111-111111110001").unwrap();
    ledger.save_run_result("b1", &passed_run(1, Some(210))).unwrap();
    // A real zero stays zero; unknown usage stays None.
    let mut zero = passed_run(2, None);
    zero.total_tokens = Some(0);
    zero.status = ProbeRunStatus::Failed;
    ledger.start_run("b1", 2).unwrap();
    ledger.save_run_result("b1", &zero).unwrap();
    ledger.finish_batch("b1", ProbeBatchStatus::Completed, None, &[]).unwrap();

    let loaded = ledger.load_snapshot(Some("b1")).unwrap().expect("batch").0;
    assert_eq!(loaded.status, ProbeBatchStatus::Completed);
    assert!(loaded.finished_at.is_some());
    assert_eq!(loaded.question.text, "题目全文");
    assert_eq!(loaded.config.profile_name.as_deref(), Some("主供应商"));
    let runs = ledger.load_snapshot(Some("b1")).unwrap().unwrap().1;
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].status, ProbeRunStatus::Passed);
    assert_eq!(runs[0].total_tokens, Some(210));
    assert_eq!(runs[1].total_tokens, Some(0));
    assert_eq!(runs[1].status, ProbeRunStatus::Failed);
    // The newest batch is the current one, finished or not.
    assert_eq!(ledger.load_snapshot(None).unwrap().expect("current").0.id, "b1");
    assert!(ledger.load_snapshot(Some("missing")).unwrap().is_none());
}

#[test]
fn visual_answers_survive_reopening_and_share_history_deletion() {
    use crate::codex_probe::{resolve_question, ProbeGrading};
    for (question, raw) in [
        (resolve_question("custom", Some("画一幅 SVG".into()), None, Some(ProbeGrading::SvgVisual)).unwrap(),
            "```svg\n<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"10\"/></svg>\n```\n"),
        (resolve_question("pelican-riding-bicycle", None, None, None).unwrap(),
            "```html\n<html><head><style>svg{width:100%}</style></head><body><svg></svg><script>requestAnimationFrame(()=>{});</script></body></html>\n```\n"),
    ] {
        let (directory, ledger) = temp_ledger();
        let grading = question.grading;
        let mut visual = batch("visual", &rfc3339(0), None);
        visual.question = ProbeQuestionSnapshot { id: question.id, label: question.label,
            text: question.prompt, expected_answer: question.expected_answer };
        visual.grading_version = grading;
        ledger.insert_batch(&visual).unwrap();
        ledger.start_run("visual", 1).unwrap();
        ledger.finish_batch("visual", ProbeBatchStatus::Completed, None, &[ProbeRunRecord {
            seq: 1, status: ProbeRunStatus::Undetermined, final_answer: Some(raw.into()),
            total_tokens: Some(210), ..Default::default()
        }]).unwrap();

        let reopened = ProbeLedger::new(directory.path());
        let cached = reopened.load_snapshot(None).unwrap().unwrap().0;
        assert_eq!(cached.grading_version, grading);
        assert_eq!(cached.question.text, visual.question.text);
        assert!(cached.question.expected_answer.is_empty());
        assert_eq!(reopened.load_snapshot(Some("visual")).unwrap().unwrap().1[0].final_answer.as_deref(), Some(raw));
        let history = reopened.list_history(&ProbeHistoryQuery { offset: 0, limit: 20,
            days: None, profile: ProbeProfileFilter::All, status: None }).unwrap();
        assert_eq!(history.items[0].grading_version, grading);
        assert_eq!(history.items[0].judged_count, 0);
        assert_eq!(history.items[0].passed_count, 0);
        assert_eq!(history.items[0].total_tokens, Some(210));
        assert_eq!(reopened.delete_batches(&["visual".into()]).unwrap().removed, 1);
        assert!(reopened.load_snapshot(None).unwrap().is_none());
        assert!(reopened.load_snapshot(Some("visual")).unwrap().is_none());
    }
}

#[test]
fn a_finished_batch_keeps_its_first_terminal_state() {
    let (_directory, ledger) = temp_ledger();
    ledger.insert_batch(&batch("b1", &rfc3339(0), None)).unwrap();
    ledger.finish_batch("b1", ProbeBatchStatus::Completed, None, &[]).unwrap();
    assert!(ledger.finish_batch("b1", ProbeBatchStatus::Cancelled, Some("late".into()), &[]).is_err());
    let loaded = ledger.load_snapshot(Some("b1")).unwrap().unwrap().0;
    assert_eq!(loaded.status, ProbeBatchStatus::Completed);
    assert_eq!(loaded.status_error, None);
}

#[test]
fn current_snapshot_prefers_the_newest_started_batch() {
    let (_directory, ledger) = temp_ledger();
    ledger.insert_batch(&batch("older", &rfc3339(2), None)).unwrap();
    ledger.insert_batch(&batch("newer", &rfc3339(1), None)).unwrap();
    assert_eq!(ledger.load_snapshot(None).unwrap().unwrap().0.id, "newer");
}

fn history_fixture() -> (tempfile::TempDir, ProbeLedger) {
    let (directory, ledger) = temp_ledger();
    ledger.insert_batch(&batch("old-linked", &rfc3339(40), Some(("p1", "供应商 A")))).unwrap();
    ledger.insert_batch(&batch("old-unlinked", &rfc3339(39), None)).unwrap();
    ledger.insert_batch(&batch("new-a", &rfc3339(2), Some(("p1", "供应商 A")))).unwrap();
    ledger.insert_batch(&batch("new-b", &rfc3339(1), Some(("p2", "供应商 B")))).unwrap();
    ledger.finish_batch("new-b", ProbeBatchStatus::Failed, Some("失败".into()), &[]).unwrap();
    for (id, seq) in [("new-a", 1u32), ("new-a", 2)] {
        ledger.start_run(id, seq).unwrap();
        ledger.save_run_result(id, &passed_run(seq, Some(100))).unwrap();
    }
    ledger.start_run("new-a", 3).unwrap();
    let failed = ProbeRunRecord { status: ProbeRunStatus::Failed, ..passed_run(3, None) };
    ledger.save_run_result("new-a", &failed).unwrap();
    ledger.finish_batch("new-a", ProbeBatchStatus::Completed, None, &[]).unwrap();

    (directory, ledger)
}

#[test]
fn history_orders_pages_and_filters_by_time_profile_and_status() {
    let (_directory, ledger) = history_fixture();
    let page = ledger.list_history(&ProbeHistoryQuery {
        offset: 0,
        limit: 10,
        days: None,
        profile: ProbeProfileFilter::All,
        status: None,
    })
    .unwrap();
    assert_eq!(page.total, 4);
    assert_eq!(
        page.items.iter().map(|item| item.batch_id.as_str()).collect::<Vec<_>>(),
        ["new-b", "new-a", "old-unlinked", "old-linked"]
    );
    let new_a = &page.items[1];
    assert_eq!(new_a.passed_count, 2);
    assert_eq!(new_a.judged_count, 3);
    assert_eq!(new_a.run_count, 3);
    assert_eq!(new_a.recorded_runs, 2);
    assert_eq!(new_a.total_tokens, Some(200));
    assert_eq!(new_a.profile_name.as_deref(), Some("供应商 A"));

    // Last-7-days window drops the two old batches.
    let page = ledger.list_history(&ProbeHistoryQuery {
        offset: 0,
        limit: 10,
        days: Some(7),
        profile: ProbeProfileFilter::All,
        status: None,
    })
    .unwrap();
    assert_eq!(page.total, 2);

    // Unlinked keeps only batches without a profile snapshot.
    let page = ledger.list_history(&ProbeHistoryQuery {
        offset: 0,
        limit: 10,
        days: None,
        profile: ProbeProfileFilter::Unlinked,
        status: None,
    })
    .unwrap();
    assert_eq!(page.items.iter().map(|item| item.batch_id.as_str()).collect::<Vec<_>>(), ["old-unlinked"]);

    // Profile + status filters compose.
    let page = ledger.list_history(&ProbeHistoryQuery {
        offset: 0,
        limit: 10,
        days: None,
        profile: ProbeProfileFilter::Id("p1".to_string()),
        status: Some(ProbeBatchStatus::Completed),
    })
    .unwrap();
    assert_eq!(page.items.iter().map(|item| item.batch_id.as_str()).collect::<Vec<_>>(), ["new-a"]);

    // Paging walks the same ordering.
    let page = ledger.list_history(&ProbeHistoryQuery {
        offset: 1,
        limit: 2,
        days: None,
        profile: ProbeProfileFilter::All,
        status: None,
    })
    .unwrap();
    assert_eq!(page.total, 4);
    assert_eq!(
        page.items.iter().map(|item| item.batch_id.as_str()).collect::<Vec<_>>(),
        ["new-a", "old-unlinked"]
    );

    let profiles = ledger.list_history_profiles().unwrap();
    assert_eq!(profiles.len(), 2);
    assert!(profiles.iter().any(|profile| profile.profile_id == "p1"));
}

#[test]
fn deletion_refuses_running_batches_and_removes_only_probe_records() {
    let (_directory, ledger) = temp_ledger();
    ledger.insert_batch(&batch("finished", &rfc3339(1), None)).unwrap();
    ledger.start_run("finished", 1).unwrap();
    ledger.save_run_result("finished", &passed_run(1, Some(9))).unwrap();
    ledger.finish_batch("finished", ProbeBatchStatus::Completed, None, &[]).unwrap();
    ledger.insert_batch(&batch("live", &rfc3339(0), None)).unwrap();
    ledger.start_run("live", 1).unwrap();

    assert!(ledger.delete_batches(&[]).is_err());
    let error = ledger.delete_batches(&["finished".into(), "live".into()]).unwrap_err();
    assert!(error.contains("运行中"), "{error}");
    // The refused request deleted nothing.
    assert!(ledger.load_snapshot(Some("finished")).unwrap().is_some());

    assert_eq!(ledger.delete_batches(&["finished".into()]).unwrap().removed, 1);
    assert!(ledger.load_snapshot(Some("finished")).unwrap().is_none());
    assert!(ledger.load_snapshot(Some("live")).unwrap().is_some());
    assert_eq!(ledger.delete_batches(&["finished".into()]).unwrap().removed, 0);
}

#[test]
fn startup_recovery_interrupts_only_running_batches_and_never_grades() {
    let (_directory, ledger) = temp_ledger();
    ledger.insert_batch(&batch("crashed", &rfc3339(0), None)).unwrap();
    ledger.start_run("crashed", 1).unwrap();
    ledger.save_run_result("crashed", &passed_run(1, Some(50))).unwrap();
    ledger.start_run("crashed", 2).unwrap();
    ledger.save_run_session("crashed", 2, "22222222-2222-4222-8222-222222222222").unwrap();
    ledger.insert_batch(&batch("no-session", &rfc3339(0), None)).unwrap();
    ledger.start_run("no-session", 1).unwrap();
    ledger.insert_batch(&batch("done", &rfc3339(1), None)).unwrap();
    ledger.finish_batch("done", ProbeBatchStatus::Completed, None, &[]).unwrap();

    let recovered = ledger
        .recover_interrupted_with_usage(|session_id| {
            assert_eq!(session_id, "22222222-2222-4222-8222-222222222222");
            Ok(crate::codex_metering::SessionTokenSummary {
                input: 10,
                cached_input: 0,
                output: 5,
                reasoning_output: Some(7),
                total: Some(77),
                model: Some("gpt-test".to_string()),
            })
        })
        .unwrap();
    assert_eq!(recovered, 2);

    let crashed = ledger.load_snapshot(Some("crashed")).unwrap().unwrap().0;
    assert_eq!(crashed.status, ProbeBatchStatus::Interrupted);
    assert!(crashed.finished_at.is_some());
    let runs = ledger.load_snapshot(Some("crashed")).unwrap().unwrap().1;
    // The finished run keeps its grade.
    assert_eq!(runs[0].status, ProbeRunStatus::Passed);
    // The interrupted run with a saved session gets usage back — no grading.
    assert_eq!(runs[1].status, ProbeRunStatus::Undetermined);
    assert_eq!(runs[1].total_tokens, Some(77));
    assert_eq!(runs[1].reasoning_tokens, Some(7));
    assert!(runs[1].execution_error.is_some());
    // Without a session id nothing is invented.
    let missing = ledger.load_snapshot(Some("no-session")).unwrap().unwrap().1;
    assert_eq!(missing[0].status, ProbeRunStatus::Undetermined);
    assert_eq!(missing[0].total_tokens, None);
    assert!(missing[0].execution_error.is_some());
    // Finished batches are untouched.
    assert_eq!(
        ledger.load_snapshot(Some("done")).unwrap().unwrap().0.status,
        ProbeBatchStatus::Completed
    );

    // A usage read that fails records the reason instead of guessing.
    ledger.insert_batch(&batch("crashed-2", &rfc3339(0), None)).unwrap();
    ledger.start_run("crashed-2", 1).unwrap();
    ledger.save_run_session("crashed-2", 1, "33333333-3333-4333-8333-333333333333").unwrap();
    ledger
        .recover_interrupted_with_usage(|_| Err("未找到".to_string()))
        .unwrap();
    let runs = ledger.load_snapshot(Some("crashed-2")).unwrap().unwrap().1;
    assert_eq!(runs[0].status, ProbeRunStatus::Undetermined);
    assert!(runs[0].usage_error.as_deref().unwrap().contains("未找到"));
}

#[test]
fn an_unrecognized_database_version_is_rejected_without_touching_the_file() {
    let directory = tempfile::tempdir().unwrap();
    let ledger = ProbeLedger::new(directory.path());
    ledger.insert_batch(&batch("b1", &rfc3339(0), None)).unwrap();
    drop(ledger);
    let path = directory.path().join("codex/probes.sqlite3");
    let connection = Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);

    let ledger = ProbeLedger::new(directory.path());
    let error = ledger.load_snapshot(Some("b1")).unwrap_err();
    assert!(error.contains("不受支持"), "{error}");
    assert!(path.exists());
    let size = std::fs::metadata(&path).unwrap().len();
    assert!(size > 0);
}

#[test]
fn terminal_write_failure_rolls_back_the_last_answer_and_preserves_retry() {
    let (directory, ledger) = temp_ledger();
    ledger.insert_batch(&batch("atomic", &rfc3339(0), None)).unwrap();
    ledger.start_run("atomic", 1).unwrap();
    let connection = Connection::open(directory.path().join("codex/probes.sqlite3")).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_finish BEFORE UPDATE OF status ON probe_batches
        BEGIN SELECT RAISE(ABORT, 'disk failure'); END;").unwrap();
    assert!(ledger.finish_batch("atomic", ProbeBatchStatus::Completed, None, &[passed_run(1, Some(12))]).is_err());
    assert_eq!(ledger.load_snapshot(Some("atomic")).unwrap().unwrap().0.status, ProbeBatchStatus::Running);
    assert_eq!(ledger.load_snapshot(Some("atomic")).unwrap().unwrap().1[0].status, ProbeRunStatus::Running);
    assert!(ledger.load_snapshot(Some("atomic")).unwrap().unwrap().1[0].final_answer.is_none());
    connection.execute_batch("DROP TRIGGER fail_finish").unwrap();
    ledger.finish_batch("atomic", ProbeBatchStatus::Completed, None, &[passed_run(1, Some(12))]).unwrap();
    assert_eq!(ledger.load_snapshot(Some("atomic")).unwrap().unwrap().1[0].final_answer.as_deref(), Some("45"));
    assert_eq!(ledger.load_snapshot(Some("atomic")).unwrap().unwrap().0.status, ProbeBatchStatus::Completed);
}

#[test]
fn incomplete_results_cannot_be_archived_or_deleted() {
    let (_directory, ledger) = temp_ledger();
    ledger.insert_batch(&batch("pending", &rfc3339(0), None)).unwrap();
    ledger.start_run("pending", 1).unwrap();
    assert!(ledger.start_run("pending", 2).is_err());
    assert!(ledger.finish_batch("pending", ProbeBatchStatus::Cancelled, None, &[]).is_err());
    assert!(ledger.delete_batches(&["pending".into()]).is_err());
    assert!(ledger.save_run_result("pending", &passed_run(2, None)).is_err());
}

#[test]
fn run_creation_requires_the_next_contiguous_sequence() {
    let (_directory, ledger) = temp_ledger();
    ledger.insert_batch(&batch("ordered", &rfc3339(0), None)).unwrap();
    assert!(ledger.start_run("ordered", 0).is_err());
    assert!(ledger.start_run("ordered", 2).is_err());
    ledger.start_run("ordered", 1).unwrap();
    assert!(ledger.start_run("ordered", 1).is_err());
    ledger.save_run_result("ordered", &passed_run(1, None)).unwrap();
    assert!(ledger.start_run("ordered", 3).is_err());
    ledger.start_run("ordered", 2).unwrap();
    let (_, runs) = ledger.load_snapshot(Some("ordered")).unwrap().unwrap();
    assert_eq!(runs.iter().map(|run| run.seq).collect::<Vec<_>>(), vec![1, 2]);
}

#[test]
fn invalid_statuses_and_unsigned_counters_are_reported_without_substitutes() {
    for (sql, expected) in [
        ("UPDATE probe_batches SET status='unknown'", "批次状态"),
        ("UPDATE probe_runs SET status='unknown'", "运行状态"),
        ("UPDATE probe_runs SET total_tokens=-1", "数据库错误"),
        ("UPDATE probe_runs SET seq=4294967296", "数据库错误"),
    ] {
        let (_directory, ledger) = temp_ledger();
        ledger.insert_batch(&batch("invalid", &rfc3339(0), None)).unwrap();
        ledger.start_run("invalid", 1).unwrap();
        let connection = ledger.open_write().unwrap();
        connection.execute_batch("PRAGMA ignore_check_constraints=ON").unwrap();
        connection.execute_batch(sql).unwrap();
        let error = ledger.load_snapshot(Some("invalid")).unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn batch_wire_contract_uses_batch_id_and_profile_filters_keep_latest_name() {
    let (_directory, ledger) = temp_ledger();
    ledger.insert_batch(&batch("older", &rfc3339(2), Some(("p", "旧名称")))).unwrap();
    ledger.insert_batch(&batch("newer", &rfc3339(0), Some(("p", "新名称")))).unwrap();
    let record = ledger.load_snapshot(Some("newer")).unwrap().unwrap().0;
    let json = serde_json::to_value(record).unwrap();
    assert_eq!(json["batchId"], "newer");
    assert!(json.get("id").is_none());
    let profiles = ledger.list_history_profiles().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].profile_name, "新名称");
}
