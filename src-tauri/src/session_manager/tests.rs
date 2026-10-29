use super::*;
use tempfile::tempdir;

struct ScanEvidence {
    sessions: Vec<SessionMeta>,
    issues: Vec<SessionIssue>,
}

fn scan_session_roots(roots: &[(AppKind, PathBuf)], titles: &HashMap<String, String>) -> ScanEvidence {
    let (sources, issues) = scan_session_source_roots(roots, titles);
    ScanEvidence { sessions: sources.into_iter().map(|source| source.meta).collect(), issues }
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    fs::write(path, content).expect("write session");
}

#[test]
fn scans_supported_sources_without_exposing_paths() {
    let temp = tempdir().expect("temp");
    let codex = temp.path().join("codex").join("a.jsonl");
    let claude = temp.path().join("claude").join("project").join("b.jsonl");
    write(
        &codex,
        concat!(
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"codex-123\",\"cwd\":\"C:/work/relay\",\"timestamp\":\"2026-08-01T10:00:00Z\"}}\n",
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":\"修复会话页面\"}}\n"
        ),
    );
    write(
        &claude,
        concat!(
            "{\"type\":\"user\",\"sessionId\":\"claude-456\",\"cwd\":\"C:/work/claude\",\"timestamp\":\"2026-08-02T10:00:00Z\",\"message\":{\"role\":\"user\",\"content\":\"整理历史\"}}\n",
            "{\"type\":\"custom-title\",\"customTitle\":\"归档整理\"}\n"
        ),
    );

    let scan = scan_session_roots(
        &[
            (AppKind::Codex, temp.path().join("codex")),
            (AppKind::Claude, temp.path().join("claude")),
        ],
        &HashMap::new(),
    );

    assert!(scan.issues.is_empty());
    assert_eq!(scan.sessions.len(), 2);
    let claude = scan
        .sessions
        .iter()
        .find(|session| session.app == AppKind::Claude)
        .expect("Claude session");
    let codex = scan
        .sessions
        .iter()
        .find(|session| session.app == AppKind::Codex)
        .expect("Codex session");
    assert_eq!(claude.title, "归档整理");
    assert_eq!(claude.resume_command.as_deref(), Some("claude --resume claude-456"));
    assert_eq!(codex.resume_command.as_deref(), Some("codex resume codex-123"));
}

#[test]
fn claude_discovery_identifies_workflow_journals_as_convertible_event_sources() {
    let temp = tempdir().expect("temp");
    let root = temp.path().join("projects");
    let project = root.join("encoded-project");
    write(
        &project.join("claude-session.jsonl"),
        "{\"type\":\"user\",\"sessionId\":\"claude-session\",\"message\":{\"role\":\"user\",\"content\":\"主会话\"}}\n",
    );
    write(
        &project.join("claude-session").join("subagents").join("workflows")
            .join("wf-example").join("journal.jsonl"),
        "{\"type\":\"workflow-journal\",\"content\":\"internal\"}\n",
    );

    let scan = scan_session_roots(&[(AppKind::Claude, root)], &HashMap::new());

    assert_eq!(scan.issues.len(), 1);
    assert!(scan.issues[0].message.contains("workflow 事件日志"));
    assert!(scan.issues[0].conversion_id.is_some());
    assert_eq!(scan.sessions.len(), 1);
    assert_eq!(scan.sessions[0].session_id, "claude-session");
}

#[test]
fn unknown_identity_failures_remain_visible_without_a_conversion_action() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("projects");
    write(&root.join("project/unknown.jsonl"), "{\"type\":\"unidentified\"}\n");
    let scan = scan_session_roots(&[(AppKind::Claude, root)], &HashMap::new());
    assert!(scan.sessions.is_empty());
    assert_eq!(scan.issues.len(), 1);
    assert!(scan.issues[0].message.contains("不含可识别的会话身份"));
    assert!(scan.issues[0].conversion_id.is_none());
}

#[test]
fn duplicate_session_ids_share_metadata_and_keep_all_fragments() {
    let temp = tempdir().expect("temp");
    let active_root = temp.path().join("codex").join("sessions");
    let archived_root = temp.path().join("codex").join("archived_sessions");
    write(
        &active_root.join("active.jsonl"),
        concat!(
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"codex-duplicate\",\"cwd\":\"C:/work/relay\",\"timestamp\":\"2026-08-01T10:00:00Z\"}}\n",
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":\"活动会话\"}}\n"
        ),
    );
    write(
        &archived_root.join("archived.jsonl"),
        concat!(
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"codex-duplicate\",\"cwd\":\"C:/work/relay\",\"timestamp\":\"2026-07-01T10:00:00Z\"}}\n",
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":\"归档副本\"}}\n"
        ),
    );

    let scan = scan_session_roots(
        &[
            (AppKind::Codex, active_root.clone()),
            (AppKind::Codex, archived_root),
        ],
        &HashMap::new(),
    );

    assert_eq!(scan.sessions.len(), 1);
    assert_eq!(scan.sessions[0].session_id, "codex-duplicate");
    assert_eq!(scan.sessions[0].title, "活动会话");

    // A rename recorded beside the transcript replaces the derived title
    // for the Codex record only; the summary keeps the first prompt.
    let renamed = scan_session_roots(
        &[(AppKind::Codex, active_root)],
        &HashMap::from([(
            "codex-duplicate".to_string(),
            "  部署线程（已重命名）  ".to_string(),
        )]),
    );
    assert_eq!(renamed.sessions[0].title, "部署线程（已重命名）");
    assert_eq!(renamed.sessions[0].summary, "活动会话");
}

#[test]
fn preserves_plain_messages_and_structured_tool_calls() {
    let temp = tempdir().expect("temp");
    let path = temp.path().join("session.jsonl");
    write(
        &path,
        concat!(
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"codex-123\"}}\n",
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"你好\"}]}}\n",
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"已完成\"}]}}\n",
            "{\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\",\"name\":\"shell\"}}\n"
        ),
    );

    let messages = collect_messages(&path).expect("read messages");
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[2].role, "tool");
    assert!(messages[2].content.contains("shell"));
    assert_eq!(messages[0].content, "你好");
    assert_eq!(messages[1].role, "assistant");
}

#[test]
fn invalid_session_ids_are_not_made_resumable() {
    assert!(!parser::valid_session_id("session; shutdown"));
    assert!(!parser::valid_session_id(""));
    assert!(parser::valid_session_id(
        "019f4b74-c859-7e72-bb0c-9f83347954fb"
    ));
}

#[test]
fn discovers_metadata_and_real_requests_after_long_injected_context() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("late.jsonl");
    let mut records = vec![serde_json::json!({"type":"user","message":{"role":"user","content":"# AGENTS.md instructions for fixture"}}); 100];
    records.push(serde_json::json!({"type":"user","sessionId":"late-1","cwd":"C:/work/late",
        "timestamp":"2026-10-16T02:00:00Z","message":{"role":"user",
        "content":"# Context from my IDE setup:\n## My request for Codex: 真正请求"}}));
    write(&path, &records.iter().map(|record| format!("{record}\n")).collect::<String>());
    let scan = scan_session_roots(&[(AppKind::Claude, temp.path().to_path_buf())], &HashMap::new());
    assert!(scan.issues.is_empty());
    assert_eq!(scan.sessions[0].session_id, "late-1");
    assert_eq!(scan.sessions[0].summary, "真正请求");
    assert_eq!(scan.sessions[0].last_active_at.as_deref(), Some("2026-10-16T02:00:00+00:00"));
}

#[test]
fn claude_subagents_have_separate_identity_and_resume_their_parent() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("projects");
    for (path, text) in [(root.join("project/main.jsonl"), "main"),
        (root.join("project/main/subagents/agent-one.jsonl"), "child")] {
        write(&path, &format!("{}\n", serde_json::json!({"type":"user","sessionId":"main",
            "message":{"role":"user","content":text}})));
    }
    let scan = scan_session_roots(&[(AppKind::Claude, root)], &HashMap::new());
    assert!(scan.issues.is_empty());
    assert_eq!(scan.sessions.len(), 2);
    let child = scan.sessions.iter().find(|session| session.parent_session_id.is_some()).unwrap();
    assert_eq!(child.session_id, "main-agent-one");
    assert_eq!(child.resume_command.as_deref(), Some("claude --resume main"));
}

#[test]
fn continuation_fragments_keep_new_messages_and_deduplicate_replayed_prefixes() {
    let temp = tempdir().unwrap();
    let first = temp.path().join("a.jsonl");
    let next = temp.path().join("b.jsonl");
    let header = format!("{}\n", serde_json::json!({"type":"session_meta","payload":{"id":"same"}}));
    let user = format!("{}\n", serde_json::json!({"role":"user","content":"question"}));
    let answer = format!("{}\n", serde_json::json!({"role":"assistant","content":"answer"}));
    write(&first, &(header.clone() + &user));
    write(&next, &(header + &user + &answer));
    let (sources, issues) = scan_session_source_roots(&[(AppKind::Codex, temp.path().to_path_buf())], &HashMap::new());
    assert!(issues.is_empty());
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].paths.len(), 2);
    let mut messages = Vec::new();
    sources[0].visit(|message| { messages.push(message); Ok(std::ops::ControlFlow::Continue(())) }).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content, "answer");
}

#[test]
fn raw_fragments_follow_event_time_across_active_and_archive_directories() {
    let temp = tempdir().unwrap();
    for (name, day, text) in [("a-archive/new.jsonl", 16, "new"), ("z-active/old.jsonl", 15, "old")] {
        write(&temp.path().join(name), &format!("{}\n{}\n",
            serde_json::json!({"type":"session_meta","payload":{"id":"same"}}),
            serde_json::json!({"role":"user","content":text,"timestamp":format!("2026-10-{day}T00:00:00Z")})));
    }
    let (sources, issues) = scan_session_source_roots(&[(AppKind::Codex, temp.path().to_path_buf())], &HashMap::new());
    assert!(issues.is_empty());
    let mut contents = Vec::new();
    sources[0].visit(|message| { contents.push(message.content); Ok(std::ops::ControlFlow::Continue(())) }).unwrap();
    assert_eq!(contents, ["old", "new"]);
}

#[test]
fn deleting_claude_sidecars_cleans_only_metadata_whose_entire_source_is_removed() {
    let temp = tempdir().unwrap();
    let main = fixture_source(&temp.path().join("main.jsonl"), AppKind::Claude, "main");
    let inside = fixture_source(&temp.path().join("main/subagents/agent-one.jsonl"), AppKind::Claude, "main-agent-one");
    let mut copied = fixture_source(&temp.path().join("main/subagents/agent-two.jsonl"), AppKind::Claude, "main-agent-two");
    copied.paths.push(temp.path().join("retained/subagents/agent-two.jsonl"));
    let sources = vec![main.clone(), inside, copied];
    assert_eq!(removed_organization_ids(&main, &sources), ["main", "main-agent-one"]);
}

#[test]
fn removes_only_the_resolved_codex_record_without_touching_siblings() {
    let temp = tempdir().expect("temp");
    let record = temp.path().join("session-1.jsonl");
    let sidecar = temp.path().join("session-1");
    write(&record, "{\"type\":\"user\"}\n");
    fs::create_dir_all(sidecar.join("parts")).expect("create sidecar");
    write(&sidecar.join("parts").join("chunk.bin"), "payload");
    let unrelated = temp.path().join("session-2.jsonl");
    write(&unrelated, "{\"type\":\"user\"}\n");

    remove_session_source(&record).expect("delete raw record");
    assert!(!record.exists(), "会话记录应被删除");
    assert!(sidecar.exists(), "原生 Codex 附属目录由客户端拥有");
    assert!(unrelated.exists(), "无关记录必须保留");

    let codex_record = temp.path().join("rollout-1.jsonl");
    let codex_sidecar = temp.path().join("rollout-1");
    write(&codex_record, "{\"type\":\"session_meta\"}\n");
    fs::create_dir_all(&codex_sidecar).expect("create codex sidecar");

    remove_session_source(&codex_record).expect("delete codex record");
    assert!(!codex_record.exists(), "会话记录应被删除");
    assert!(codex_sidecar.exists(), "Codex 不清理同名目录");
}

#[test]
fn a_resolved_record_that_is_already_absent_can_finish_metadata_cleanup() {
    let temp = tempdir().expect("temp");
    remove_session_source(&temp.path().join("gone.jsonl")).unwrap();
}

#[test]
fn batch_deletion_reports_each_request_and_removes_only_resolved_records() {
    let temp = tempdir().expect("temp");
    let root = temp.path().join("codex").join("sessions");
    for (file, id) in [("one.jsonl", "codex-one"), ("two.jsonl", "codex-two")] {
        write(
            &root.join(file),
            &format!(
                "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\",\"cwd\":\"C:/work\",\"timestamp\":\"2026-08-01T10:00:00Z\"}}}}\n\
                 {{\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"user\",\"content\":\"问题\"}}}}\n"
            ),
        );
    }
    let (sources, issues) =
        scan_session_source_roots(&[(AppKind::Codex, root.clone())], &HashMap::new());
    let request = |app, id: &str| SessionDeleteRequest {
        app,
        session_id: id.to_string(),
    };
    let outcomes = delete_resolved(
        temp.path(),
        &sources,
        &issues,
        &[
            request(AppKind::Codex, "codex-one"),
            request(AppKind::Codex, "codex-one"),
            request(AppKind::Codex, "codex-missing"),
            request(AppKind::Claude, "codex-two"),
            request(AppKind::Codex, "bad id;"),
        ],
        None,
    );

    assert_eq!(outcomes.len(), 4, "重复请求只回答一次");
    assert!(outcomes[0].deleted && outcomes[0].error.is_none());
    assert!(!outcomes[1].deleted);
    assert!(outcomes[1]
        .error
        .as_deref()
        .unwrap()
        .contains("找不到指定会话"));
    assert!(
        !outcomes[2].deleted,
        "同一 ID 属于另一客户端时不得跨客户端删除"
    );
    assert_eq!(outcomes[3].error.as_deref(), Some("会话 ID 无效"));
    assert!(!root.join("one.jsonl").exists());
    assert!(
        root.join("two.jsonl").exists(),
        "未解析到的请求不得影响其他记录"
    );
}

#[test]
fn resume_arguments_keep_the_client_command_and_id_separate() {
    assert_eq!(
        resume_arguments(AppKind::Codex, "codex-123"),
        ["codex", "resume", "codex-123"]
    );
    assert_eq!(
        resume_arguments(AppKind::Claude, "claude-456"),
        ["claude", "--resume", "claude-456"]
    );
}

#[test]
fn shell_resume_command_keeps_the_executable_and_arguments_separate() {
    assert_eq!(
        resume::shell_command(&["codex", "resume", "codex-123"]),
        "'codex' 'resume' 'codex-123'"
    );
    assert_eq!(
        resume::shell_command(&["claude", "--resume", "id-with-'quote"]),
        "'claude' '--resume' 'id-with-'\\''quote'"
    );
}
