use super::*;
use super::questions::CUSTOM_QUESTION_ID;
use std::path::Path;

pub(super) fn write_rollout(root: &std::path::Path, id: &str, total: u64) -> std::path::PathBuf {
    let directory = root.join("sessions").join("t");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("rollout-{id}.jsonl"));
    let tokens = serde_json::json!({"input_tokens":total-90,"output_tokens":90,
        "cached_input_tokens":30,"cache_write_input_tokens":0,"reasoning_output_tokens":77,"total_tokens":total});
    let lines = [
        serde_json::json!({"type":"session_meta", "payload":{"id": id}}),
        serde_json::json!({"type":"turn_context", "payload":{"turn_id":"probe-turn","model":"gpt-test"}}),
        serde_json::json!({"type":"token_usage_record", "payload":{
            "thread_id":id,"session_id":"probe-session","turn_id":"probe-turn","root_turn_id":"probe-root",
            "response_id":"probe-response","usage":tokens,"turn_token_usage":tokens,"thread_token_usage":tokens}}),
    ];
    std::fs::write(&path, lines.map(|line| line.to_string()).join("\n") + "\n").unwrap();
    path
}

#[test]
fn answers_are_whole_nonnegative_integers_not_substrings() {
    use super::questions::answer_matches;
    assert!(answer_matches(" 45\n", "45"));
    assert!(answer_matches("00045", "45"));
    for answer in ["145", "45.5", "-45", "45 44", "not 45", "", "**45**"] {
        assert!(!answer_matches(answer, "45"), "{answer}");
    }
    let question = resolve_question(CUSTOM_QUESTION_ID, Some("题目".into()), Some("000".into()),
        Some(ProbeGrading::Integer)).unwrap();
    assert_eq!(question.expected_answer, "0");
}

#[test]
fn builtin_questions_have_unique_ids_and_answers_matching_their_format() {
    let mut ids = Vec::new();
    for question in QUESTIONS.iter() {
        assert!(!question.id.is_empty());
        assert!(!question.label.is_empty());
        assert!(!question.prompt.is_empty());
        match question.grading {
            ProbeGrading::Integer => {
                assert!(!question.expected_answer.is_empty());
                assert!(question.expected_answer.bytes().all(|byte| byte.is_ascii_digit()));
            }
            ProbeGrading::SvgVisual | ProbeGrading::HtmlVisual => assert!(question.expected_answer.is_empty()),
        }
        ids.push(question.id);
    }
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), QUESTIONS.len());
    assert_eq!(QUESTIONS[0].id, "pelican-riding-bicycle");
    assert_eq!(QUESTIONS[0].max_runs, 1);
    for question in QUESTIONS.iter().skip(1) {
        assert_eq!(question.max_runs, super::MAX_RUN_COUNT);
    }
}

#[test]
fn resolve_question_validates_builtin_and_custom_selections() {
    let builtin = resolve_question(QUESTIONS[0].id, None, None, None).expect("builtin question");
    assert_eq!(builtin.id, QUESTIONS[0].id);
    assert_eq!(builtin.label, QUESTIONS[0].label);
    assert_eq!(builtin.expected_answer, QUESTIONS[0].expected_answer);

    let custom = resolve_question(
        CUSTOM_QUESTION_ID,
        Some("  题目文本 ".to_string()),
        Some(" 21 ".to_string()),
        Some(ProbeGrading::Integer),
    )
    .expect("custom question");
    assert_eq!(custom.id, CUSTOM_QUESTION_ID);
    assert_eq!(custom.prompt, "题目文本");
    assert_eq!(custom.expected_answer, "21");

    assert!(resolve_question("unknown", None, None, None).is_err());
    assert!(resolve_question(CUSTOM_QUESTION_ID, None, None, Some(ProbeGrading::Integer)).is_err());
    assert!(resolve_question(CUSTOM_QUESTION_ID, Some("题目".into()), None, Some(ProbeGrading::Integer)).is_err());
    assert!(resolve_question(CUSTOM_QUESTION_ID, Some("题目".into()), Some("x".into()),
        Some(ProbeGrading::Integer)).is_err());
}

#[test]
fn custom_svg_answers_keep_their_visual_contract_without_an_integer_reference() {
    let question = resolve_question(CUSTOM_QUESTION_ID, Some("画一幅 SVG".into()),
        None, Some(ProbeGrading::SvgVisual)).unwrap();
    assert_eq!(question.grading, ProbeGrading::SvgVisual);
    assert_eq!(question.execution_prompt(), "画一幅 SVG");
    assert!(question.expected_answer.is_empty());
    let custom = resolve_question(CUSTOM_QUESTION_ID, Some(question.prompt.clone()),
        Some(question.expected_answer.clone()), Some(question.grading)).unwrap();
    assert_eq!(custom.execution_prompt(), question.execution_prompt());
    assert_eq!(custom.grading, question.grading);
    assert!(resolve_question(CUSTOM_QUESTION_ID, Some("画图".into()), Some("45".into()),
        Some(ProbeGrading::SvgVisual)).is_err());
    assert!(resolve_question(CUSTOM_QUESTION_ID, Some("画图".into()), None, None).is_err());
    assert!(ProbeGrading::parse("unknown").is_none());
}

#[test]
fn linuxdo_animation_keeps_its_full_prompt_and_html_output_contract() {
    let question = resolve_question("pelican-riding-bicycle", None, None, None).unwrap();
    assert_eq!(QUESTIONS.iter().filter(|item| item.id.contains("pelican")).count(), 1);
    assert!(resolve_question("pelican-riding-bicycle-animation", None, None, None).is_err());
    assert_eq!(question.grading, ProbeGrading::HtmlVisual);
    assert_eq!(question.prompt, "生成html，内容是svg绘制鹈鹕骑自行车2D动画，不用进行测试。\n直接在最终回答中输出完整 HTML 源码，不要只返回文件路径或完成说明。");
    assert_eq!(question.execution_prompt(), question.prompt);
    assert!(question.expected_answer.is_empty());
    assert!(QUESTIONS.iter().find(|item| item.id == question.id)
        .unwrap().reference_visual.is_some());
    assert_eq!(QUESTIONS.iter().filter(|item| item.reference_visual.is_some()).count(), 1);
    let custom = resolve_question(CUSTOM_QUESTION_ID, Some(question.prompt.clone()),
        None, Some(question.grading)).unwrap();
    assert_eq!(custom.execution_prompt(), question.execution_prompt());
    assert_eq!(custom.grading, question.grading);
    assert!(resolve_question(CUSTOM_QUESTION_ID, Some("生成 HTML".into()), Some("45".into()),
        Some(ProbeGrading::HtmlVisual)).is_err());
    for grading in [ProbeGrading::Integer, ProbeGrading::SvgVisual, ProbeGrading::HtmlVisual] {
        assert_eq!(ProbeGrading::parse(grading.as_str()), Some(grading));
    }
}

#[test]
fn fingerprint_changes_only_when_the_client_files_change() {
    let _guard = crate::test_client_paths::redirect_client_paths();
    let local = crate::local_state::LocalState::from_root(Path::new("state").to_path_buf());
    let target = local.target(asb_core::AppKind::Codex).unwrap();
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, "model = \"gpt-5\"\n").unwrap();
    let first = snapshot::fingerprint(&local, None).unwrap();
    assert_eq!(snapshot::fingerprint(&local, None).unwrap(), first);
    std::fs::write(&target, "model = \"gpt-5.1\"\n").unwrap();
    assert_ne!(snapshot::fingerprint(&local, None).unwrap(), first);
    // The auth file participates: same config, different key material.
    std::fs::write(&target, "model = \"gpt-5\"\n").unwrap();
    assert_eq!(snapshot::fingerprint(&local, None).unwrap(), first);
    std::fs::write(target.with_file_name("auth.json"), "{\"OPENAI_API_KEY\":\"sk-x\"}").unwrap();
    assert_ne!(snapshot::fingerprint(&local, None).unwrap(), first);
}

#[test]
fn unidentified_configurations_stay_unlinked_instead_of_guessing() {
    let _guard = crate::test_client_paths::redirect_client_paths();
    let local = crate::local_state::LocalState::from_root(Path::new("state").to_path_buf());
    let target = local.target(asb_core::AppKind::Codex).unwrap();
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, "model = \"gpt-5\"\n").unwrap();
    let record = snapshot::capture(&local, None).expect("snapshot");
    assert_eq!(record.profile_id, None);
    assert_eq!(record.profile_name, None);
    assert!(!record.fingerprint.is_empty());
}

#[test]
fn config_read_errors_are_not_hashed_as_missing_files() {
    let _guard = crate::test_client_paths::redirect_client_paths();
    let local = crate::local_state::LocalState::from_root(Path::new("state").to_path_buf());
    let target = local.target(asb_core::AppKind::Codex).unwrap();
    std::fs::create_dir_all(&target).unwrap();
    assert!(snapshot::fingerprint(&local, None).is_err());
}

#[test]
fn snapshot_reads_actual_model_and_removes_url_credentials() {
    let _guard = crate::test_client_paths::redirect_client_paths();
    let local = crate::local_state::LocalState::from_root(Path::new("state").to_path_buf());
    let target = local.target(asb_core::AppKind::Codex).unwrap();
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, r#"model = "actual-model"
model_reasoning_effort = "high"
openai_base_url = "https://user:secret@example.test/v1?key=secret#secret"
"#).unwrap();
    let config = snapshot::capture(&local, None).unwrap();
    assert_eq!(config.profile_model.as_deref(), Some("actual-model"));
    assert_eq!(config.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(config.connection_identity.as_deref(), Some("https://example.test/v1"));
}
