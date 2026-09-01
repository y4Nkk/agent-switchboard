//! Transactional switch tests. Everything runs inside `tempfile` temporary
//! directories; no real user configuration is ever touched.

use asb_core::contracts::{
    AppKind, ClaudeModelSettings, ConfigValue, ModelOptions, ProviderProfile, SwitchPlan,
};
use asb_core::ownership::default_common_settings;
use asb_core::test_support::{CLAUDE_JSON, CODEX_TOML};
use asb_switch::io::{FsIo, SwitchIo};
use asb_switch::lockfile;
use asb_switch::{read_preview, sha256_hex, RecoveryOutcome, RestoreOutcome, SwitchError};
use std::cell::Cell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn codex_plan(name: &str, base_url: &str, model: &str, cred: &str) -> SwitchPlan {
    let mut common = default_common_settings(AppKind::Codex);
    common
        .settings
        .insert("disable_response_storage".into(), ConfigValue::Bool(true));
    SwitchPlan {
        profile: ProviderProfile {
            id: format!("id-{name}"),
            app: AppKind::Codex,
            route_mode: asb_core::RouteMode::Custom,
            name: name.into(),
            model: Some(model.into()),
            base_url: Some(base_url.into()),
            api_key: cred.into(),
            model_options: None,
            notes: None,
            website_url: None,
            usage_query: None,
        },
        common,
    }
}

fn claude_plan(name: &str, base_url: &str, model: &str) -> SwitchPlan {
    let mut common = default_common_settings(AppKind::Claude);
    common
        .settings
        .insert("alwaysThinkingEnabled".into(), ConfigValue::Bool(true));
    SwitchPlan {
        profile: ProviderProfile {
            id: format!("id-{name}"),
            app: AppKind::Claude,
            route_mode: asb_core::RouteMode::Custom,
            name: name.into(),
            model: Some(model.into()),
            base_url: Some(base_url.into()),
            api_key: "test-api-key".into(),
            model_options: None,
            notes: None,
            website_url: None,
            usage_query: None,
        },
        common,
    }
}

/// Ordinary executor tests must still provide the explicit state commit
/// closure required by the production API. This helper supplies the accepted
/// state write so individual tests can focus on their own transaction path.
fn execute<Io: SwitchIo>(
    io: &Io,
    request: &asb_switch::SwitchRequest,
) -> Result<asb_switch::SwitchOutcome, SwitchError> {
    asb_switch::execute(io, request, |_| Ok(()))
}

/// Ordinary restore tests also provide the required state commit closure.
fn restore<Io: SwitchIo>(
    io: &Io,
    backup: &asb_core::BackupRecord,
    target: &Path,
) -> Result<RestoreOutcome, SwitchError> {
    asb_switch::restore(io, backup, target, |_| Ok(()))
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn setup(app: AppKind, content: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let (file, initial) = match app {
        AppKind::Codex => ("config.toml", content),
        AppKind::Claude => ("settings.json", content),
    };
    let target = dir.path().join("live").join(file);
    write(&target, initial);
    let backup_dir = dir.path().join("backups");
    (dir, target, backup_dir)
}

/// Wraps FsIo with deterministic failure injection at a chosen stage.
struct FailingIo {
    fail_stage: Cell<Option<&'static str>>,
    renamed: Cell<bool>,
    /// When set, the first read of the live file after a rename returns
    /// corrupted content, simulating a post-verify failure with the file
    /// already replaced. One-shot: the restore path must read cleanly.
    corrupt_once: Cell<bool>,
    corrupt_after_rename: bool,
    /// Corrupts only the executor's backup re-read response without changing
    /// the isolated temporary directory on disk.
    corrupt_backup_read: bool,
    /// Corrupts only the executor's temporary-file re-read response.
    corrupt_temp_read: bool,
    /// Simulates a host process editing the live Codex target while the
    /// executor is preparing its temporary candidate.
    mutate_live_after_temp_write: bool,
}

impl FailingIo {
    fn new() -> Self {
        Self {
            fail_stage: Cell::new(None),
            renamed: Cell::new(false),
            corrupt_once: Cell::new(false),
            corrupt_after_rename: false,
            corrupt_backup_read: false,
            corrupt_temp_read: false,
            mutate_live_after_temp_write: false,
        }
    }

    fn check(&self, stage: &'static str) -> io::Result<()> {
        if self.fail_stage.get() == Some(stage) {
            return Err(io::Error::other(format!("injected failure at {stage}")));
        }
        Ok(())
    }

    fn is_live_target(&self, path: &Path) -> bool {
        path.file_name()
            .map(|n| n == "config.toml")
            .unwrap_or(false)
    }
}

impl SwitchIo for FailingIo {
    fn read_file(&self, path: &Path) -> io::Result<String> {
        let text = fs::read_to_string(path)?;
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if self.corrupt_backup_read && name.ends_with(".bak") {
            return Ok(format!("{text}\ncorrupted backup read"));
        }
        if self.corrupt_temp_read && (name.contains(".asb-tmp") || name.contains(".asb-restore")) {
            return Ok(format!("{text}\ncorrupted temporary read"));
        }
        if self.corrupt_after_rename
            && !self.corrupt_once.get()
            && self.renamed.get()
            && self.is_live_target(path)
        {
            self.corrupt_once.set(true);
            return Ok(text + "\n# corrupted");
        }
        Ok(text)
    }

    fn write_new_file(&self, path: &Path, content: &str) -> io::Result<()> {
        // Only the switch temp file may be forced to fail; lock acquisition
        // and backup creation must keep working.
        if path.to_string_lossy().ends_with(".asb-tmp") {
            self.check("temp-write")?;
        }
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(content.as_bytes())?;
        if self.mutate_live_after_temp_write && path.to_string_lossy().ends_with(".asb-tmp") {
            fs::write(
                path.parent().expect("temp parent").join("config.toml"),
                format!("{CODEX_TOML}\nhost_changed_during_switch = true\n"),
            )?;
        }
        Ok(())
    }

    fn write_file_replace(&self, path: &Path, content: &str) -> io::Result<()> {
        fs::write(path, content)
    }

    fn rename_replace(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.check("atomic-replace")?;
        fs::rename(from, to)?;
        self.renamed.set(true);
        Ok(())
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().ends_with(".asb-lock"))
        {
            self.check("lock-release")?;
        }
        fs::remove_file(path)
    }

    fn ensure_dir(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        let mut out = Vec::new();
        for entry in fs::read_dir(path)? {
            out.push(entry?.path());
        }
        Ok(out)
    }

    fn now_rfc3339(&self) -> String {
        FsIo.now_rfc3339()
    }
}

#[test]
fn switch_a_to_b_to_restore_preserves_every_host_field() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let original = fs::read_to_string(&target).unwrap();
    let io = FsIo;

    let plan_b = codex_plan(
        "Relay B",
        "https://relay-b.internal/v1",
        "gpt-5.2",
        "CODEX_RELAY_B_KEY",
    );
    let fp = read_preview(&io, &target, &plan_b, &backup_dir.to_string_lossy()).unwrap();
    let outcome = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan_b,
            backup_dir: &backup_dir,
            expected_hash: &fp.content_hash,
            expected_rendered_hash: &fp.rendered_hash,
        },
    )
    .unwrap();

    assert_eq!(outcome.changed, vec![target.to_string_lossy().to_string()]);
    assert!(outcome.backup.content_hash == sha256_hex(&original));
    let switched = fs::read_to_string(&target).unwrap();
    // The UI receives only a redacted candidate; the executor hashes and
    // writes its private original rendering.
    assert_ne!(switched, fp.content);
    assert!(fp.content.contains("••••••••"));
    assert!(!fp.content.contains("CODEX_RELAY_B_KEY"));
    assert!(switched.contains("experimental_bearer_token = \"CODEX_RELAY_B_KEY\""));
    assert!(switched.contains("model_provider = \"openai\""));
    assert!(switched.contains("openai_base_url = \"https://relay-b.internal/v1\""));
    assert!(switched.contains("disable_response_storage = true"));
    assert!(!switched.contains("[model_providers"));
    assert!(switched.contains("https://relay-b.internal/v1"));
    for host in ["threads = 8", "history_persistence", "trusted = true"] {
        assert!(switched.contains(host), "host field lost: {host}");
    }

    let restored = restore(&io, &outcome.backup, &target).unwrap();
    assert_eq!(restored.restored_hash, sha256_hex(&original));
    assert_eq!(fs::read_to_string(&target).unwrap(), original);
}

#[test]
fn claude_switch_round_trips_with_a_redacted_preview() {
    let (_dir, target, backup_dir) = setup(AppKind::Claude, CLAUDE_JSON);
    let io = FsIo;
    let plan = claude_plan("Relay C", "https://relay-c.internal", "claude-opus-4");

    let fp = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();
    let token_change = fp
        .preview
        .changes
        .iter()
        .find(|change| change.key == "env.ANTHROPIC_AUTH_TOKEN")
        .expect("profile token change");
    assert_eq!(token_change.after.as_deref(), Some("••••••••"));
    assert!(!fp.content.contains("test-api-key"));
    execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &fp.content_hash,
            expected_rendered_hash: &fp.rendered_hash,
        },
    )
    .unwrap();

    let text = fs::read_to_string(&target).unwrap();
    assert!(text.contains("https://relay-c.internal"));
    assert!(text.contains("claude-opus-4"));
    assert!(text.contains("Bash(npm run test:*)"));
    assert!(text.contains("\"ANTHROPIC_AUTH_TOKEN\": \"test-api-key\""));
    assert!(text.contains("\"alwaysThinkingEnabled\": true"));
}

#[test]
fn provider_projection_restores_the_client_file_when_state_commit_fails() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let io = FsIo;
    let plan = codex_plan(
        "Relay B",
        "https://relay-b.internal/v1",
        "gpt-5.2",
        "CODEX_RELAY_B_KEY",
    );
    let preview = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();
    let lock_path = lockfile::lock_path_for(&target);

    let error = asb_switch::execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &preview.content_hash,
            expected_rendered_hash: &preview.rendered_hash,
        },
        |_| {
            assert!(
                lock_path.exists(),
                "state commit occurs before lock release"
            );
            Err("application state unavailable".to_string())
        },
    )
    .unwrap_err();

    assert!(matches!(
        error,
        SwitchError::CommitFailed {
            stage: "state-save",
            recovery: RecoveryOutcome::Restored { .. },
            ..
        }
    ));
    assert_eq!(fs::read_to_string(&target).unwrap(), CODEX_TOML);
    assert!(!lock_path.exists());
}

#[test]
fn restore_restores_the_client_file_when_state_commit_fails() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let io = FsIo;
    let plan = codex_plan(
        "Relay B",
        "https://relay-b.internal/v1",
        "gpt-5.2",
        "CODEX_RELAY_B_KEY",
    );
    let preview = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();
    let switched = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &preview.content_hash,
            expected_rendered_hash: &preview.rendered_hash,
        },
    )
    .unwrap();
    let switched_content = fs::read_to_string(&target).unwrap();
    let lock_path = lockfile::lock_path_for(&target);

    let error = asb_switch::restore(&io, &switched.backup, &target, |_| {
        assert!(
            lock_path.exists(),
            "state commit occurs before lock release"
        );
        Err("application state unavailable".to_string())
    })
    .unwrap_err();

    assert!(matches!(
        error,
        SwitchError::CommitFailed {
            stage: "state-save",
            recovery: RecoveryOutcome::Restored { .. },
            ..
        }
    ));
    assert_eq!(fs::read_to_string(&target).unwrap(), switched_content);
    assert!(!lock_path.exists());
}

#[test]
fn executor_rejects_a_backup_that_fails_its_readback_verification() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let io = FailingIo {
        corrupt_backup_read: true,
        ..FailingIo::new()
    };
    let plan = codex_plan("Relay B", "https://relay-b.internal/v1", "gpt-5.2", "KEY");
    let preview = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();

    let error = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &preview.content_hash,
            expected_rendered_hash: &preview.rendered_hash,
        },
    )
    .unwrap_err();

    assert!(matches!(
        error,
        SwitchError::CommitFailed {
            stage: "backup-verify",
            recovery: RecoveryOutcome::NotNeeded,
            ..
        }
    ));
    assert_eq!(fs::read_to_string(&target).unwrap(), CODEX_TOML);
    assert!(!lockfile::lock_path_for(&target).exists());
}

#[test]
fn executor_rejects_a_temporary_file_that_fails_its_readback_verification() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let io = FailingIo {
        corrupt_temp_read: true,
        ..FailingIo::new()
    };
    let plan = codex_plan("Relay B", "https://relay-b.internal/v1", "gpt-5.2", "KEY");
    let preview = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();

    let error = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &preview.content_hash,
            expected_rendered_hash: &preview.rendered_hash,
        },
    )
    .unwrap_err();

    assert!(matches!(
        error,
        SwitchError::CommitFailed {
            stage: "temp-verify",
            recovery: RecoveryOutcome::NotNeeded,
            ..
        }
    ));
    assert_eq!(fs::read_to_string(&target).unwrap(), CODEX_TOML);
    assert!(!lockfile::lock_path_for(&target).exists());
}

#[test]
fn executor_preserves_a_host_edit_made_while_preparing_the_temporary_file() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let io = FailingIo {
        mutate_live_after_temp_write: true,
        ..FailingIo::new()
    };
    let plan = codex_plan("Relay B", "https://relay-b.internal/v1", "gpt-5.2", "KEY");
    let preview = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();

    let error = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &preview.content_hash,
            expected_rendered_hash: &preview.rendered_hash,
        },
    )
    .unwrap_err();

    assert!(matches!(error, SwitchError::ExternalChange { .. }));
    assert!(fs::read_to_string(&target)
        .unwrap()
        .contains("host_changed_during_switch = true"));
    assert!(!lockfile::lock_path_for(&target).exists());
}

#[test]
fn lock_release_failure_is_returned_as_an_observable_success_warning() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let io = FailingIo::new();
    io.fail_stage.set(Some("lock-release"));
    let plan = codex_plan("Relay B", "https://relay-b.internal/v1", "gpt-5.2", "KEY");
    let preview = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();

    let outcome = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &preview.content_hash,
            expected_rendered_hash: &preview.rendered_hash,
        },
    )
    .unwrap();

    assert!(outcome
        .warnings
        .iter()
        .any(|warning| warning.contains("无法释放写入锁")));
    assert!(lockfile::lock_path_for(&target).exists());
}

#[test]
fn claude_one_m_switch_writes_the_wire_suffix_from_semantic_profile_state() {
    let (_dir, target, backup_dir) = setup(AppKind::Claude, CLAUDE_JSON);
    let io = FsIo;
    let mut plan = claude_plan("Relay 1M", "https://relay-c.internal", "claude-opus-4-7");
    plan.profile.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
        primary_one_m: true,
        haiku_model: Some("claude-haiku-4".into()),
        sonnet_model: Some("claude-sonnet-4-6".into()),
        sonnet_one_m: true,
        opus_model: Some("claude-opus-4-7".into()),
        opus_one_m: true,
        available_models: None,
    }));

    let preview = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();
    execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &preview.content_hash,
            expected_rendered_hash: &preview.rendered_hash,
        },
    )
    .unwrap();

    let rendered: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    assert_eq!(rendered["model"], "claude-opus-4-7[1m]");
    assert_eq!(
        rendered["env"]["ANTHROPIC_DEFAULT_SONNET_MODEL"],
        "claude-sonnet-4-6[1m]"
    );
    assert_eq!(
        rendered["env"]["ANTHROPIC_DEFAULT_OPUS_MODEL"],
        "claude-opus-4-7[1m]"
    );
    assert_eq!(
        rendered["env"]["ANTHROPIC_DEFAULT_HAIKU_MODEL"],
        "claude-haiku-4"
    );
}

#[test]
fn switching_to_claude_profile_without_mappings_clears_the_previous_profile_mappings() {
    let (_dir, target, backup_dir) = setup(AppKind::Claude, CLAUDE_JSON);
    let io = FsIo;
    let mut plan_a = claude_plan("Aihub", "https://aihub.internal", "claude-opus-4-7");
    plan_a.profile.model_options = Some(ModelOptions::Claude(ClaudeModelSettings {
        primary_one_m: true,
        haiku_model: Some("claude-haiku-4".into()),
        sonnet_model: Some("claude-sonnet-4-6".into()),
        sonnet_one_m: true,
        opus_model: Some("claude-opus-4-7".into()),
        opus_one_m: true,
        available_models: Some(vec!["claude-opus-4-7".into()]),
    }));
    let preview_a = read_preview(&io, &target, &plan_a, &backup_dir.to_string_lossy()).unwrap();
    execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan_a,
            backup_dir: &backup_dir,
            expected_hash: &preview_a.content_hash,
            expected_rendered_hash: &preview_a.rendered_hash,
        },
    )
    .unwrap();

    let plan_b = claude_plan(
        "AnyRouter",
        "https://anyrouter.internal",
        "claude-sonnet-4-6",
    );
    let preview_b = read_preview(&io, &target, &plan_b, &backup_dir.to_string_lossy()).unwrap();
    execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan_b,
            backup_dir: &backup_dir,
            expected_hash: &preview_b.content_hash,
            expected_rendered_hash: &preview_b.rendered_hash,
        },
    )
    .unwrap();

    let rendered: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    assert_eq!(rendered["model"], "claude-sonnet-4-6");
    for key in [
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
    ] {
        assert!(
            rendered["env"].get(key).is_none(),
            "{key} leaked from Aihub"
        );
    }
    assert!(rendered.get("availableModels").is_none());
}

#[test]
fn switching_to_codex_profile_without_one_m_clears_the_previous_profile_window() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let io = FsIo;
    let mut plan_a = codex_plan("Aihub", "https://aihub.internal/v1", "gpt-5.2", "AIHUB_KEY");
    plan_a.profile.model_options = Some(ModelOptions::Codex(asb_core::CodexModelSettings {
        context_window: Some(1_000_000),
    }));
    let preview_a = read_preview(&io, &target, &plan_a, &backup_dir.to_string_lossy()).unwrap();
    execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan_a,
            backup_dir: &backup_dir,
            expected_hash: &preview_a.content_hash,
            expected_rendered_hash: &preview_a.rendered_hash,
        },
    )
    .unwrap();

    let plan_b = codex_plan(
        "AnyRouter",
        "https://anyrouter.internal/v1",
        "gpt-5.3",
        "ANYROUTER_KEY",
    );
    let preview_b = read_preview(&io, &target, &plan_b, &backup_dir.to_string_lossy()).unwrap();
    execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan_b,
            backup_dir: &backup_dir,
            expected_hash: &preview_b.content_hash,
            expected_rendered_hash: &preview_b.rendered_hash,
        },
    )
    .unwrap();

    let rendered = fs::read_to_string(&target).unwrap();
    assert!(!rendered.contains("model_context_window"));
    assert!(rendered.contains("disable_response_storage = true"));
}

#[test]
fn invalid_temporary_write_keeps_preceding_content() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let original = fs::read_to_string(&target).unwrap();
    let io = FailingIo::new();
    io.fail_stage.set(Some("temp-write"));

    let plan = codex_plan(
        "Relay B",
        "https://relay-b.internal/v1",
        "gpt-5.2",
        "CODEX_RELAY_B_KEY",
    );
    let fp = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();
    let err = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &fp.content_hash,
            expected_rendered_hash: &fp.rendered_hash,
        },
    )
    .unwrap_err();

    match &err {
        SwitchError::CommitFailed {
            stage, recovery, ..
        } => {
            assert_eq!(*stage, "temp-write");
            assert!(matches!(recovery, RecoveryOutcome::NotNeeded));
        }
        other => panic!("expected CommitFailed, got {other:?}"),
    }
    assert_eq!(fs::read_to_string(&target).unwrap(), original);
    // The lock was released even though the write failed.
    assert!(!lock_path_exists(&target));
}

#[test]
fn failed_post_write_verification_restores_the_backup() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let original = fs::read_to_string(&target).unwrap();
    let mut io = FailingIo::new();
    io.corrupt_after_rename = true;
    let plan = codex_plan(
        "Relay B",
        "https://relay-b.internal/v1",
        "gpt-5.2",
        "CODEX_RELAY_B_KEY",
    );
    let fp = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();
    let err = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &fp.content_hash,
            expected_rendered_hash: &fp.rendered_hash,
        },
    )
    .unwrap_err();

    match &err {
        SwitchError::CommitFailed {
            stage, recovery, ..
        } => {
            assert_eq!(*stage, "post-verify");
            assert!(
                matches!(recovery, RecoveryOutcome::Restored { .. }),
                "recovery: {recovery:?}"
            );
        }
        other => panic!("expected CommitFailed, got {other:?}"),
    }
    // The immediately preceding content is back, byte for byte.
    assert_eq!(fs::read_to_string(&target).unwrap(), original);
}

fn lock_path_exists(target: &Path) -> bool {
    lockfile::lock_path_for(target).exists()
}

#[test]
fn external_edit_blocks_the_switch_with_a_diagnostic() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let io = FsIo;
    let plan = codex_plan(
        "Relay B",
        "https://relay-b.internal/v1",
        "gpt-5.2",
        "CODEX_RELAY_B_KEY",
    );

    let fp = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();
    // The host edits the file between preview and switch.
    let edited = format!("{CODEX_TOML}\nthreads = 16\n");
    fs::write(&target, &edited).unwrap();

    let err = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &fp.content_hash,
            expected_rendered_hash: &fp.rendered_hash,
        },
    )
    .unwrap_err();
    assert!(matches!(err, SwitchError::ExternalChange { .. }));
    assert_eq!(fs::read_to_string(&target).unwrap(), edited);
}

#[test]
fn changed_candidate_after_preview_is_blocked_without_writing() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let original = fs::read_to_string(&target).unwrap();
    let io = FsIo;
    let previewed = codex_plan(
        "Relay A",
        "https://relay-a.internal/v1",
        "gpt-5.1",
        "RELAY_A_KEY",
    );
    let changed = codex_plan(
        "Relay B",
        "https://relay-b.internal/v1",
        "gpt-5.2",
        "RELAY_B_KEY",
    );
    let fp = read_preview(&io, &target, &previewed, &backup_dir.to_string_lossy()).unwrap();

    let err = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &changed,
            backup_dir: &backup_dir,
            expected_hash: &fp.content_hash,
            expected_rendered_hash: &fp.rendered_hash,
        },
    )
    .unwrap_err();

    assert!(matches!(err, SwitchError::PlanChanged));
    assert_eq!(fs::read_to_string(&target).unwrap(), original);
    assert!(!backup_dir.exists());
}

#[test]
fn first_switch_creates_a_file_and_undo_restores_its_absence() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("live").join("config.toml");
    let backup_dir = dir.path().join("backups");
    let io = FsIo;
    let plan = codex_plan(
        "Relay B",
        "https://relay-b.internal/v1",
        "gpt-5.2",
        "CODEX_RELAY_B_KEY",
    );

    let fp = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();
    assert!(fp
        .preview
        .warnings
        .iter()
        .any(|warning| warning.contains("尚不存在")));
    let switched = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &fp.content_hash,
            expected_rendered_hash: &fp.rendered_hash,
        },
    )
    .unwrap();
    assert!(!switched.backup.target_existed);
    let switched_content = fs::read_to_string(&target).expect("created target");
    assert!(switched_content.contains("relay-b.internal"));

    let restored = restore(&io, &switched.backup, &target).expect("restore original absence");
    assert!(!target.exists());
    let records = asb_switch::list_backups(&io, &backup_dir);
    assert!(records
        .iter()
        .any(|record| record.id == restored.pre_restore_backup.id));

    restore(&io, &restored.pre_restore_backup, &target).expect("undo restore");
    assert_eq!(fs::read_to_string(&target).unwrap(), switched_content);
}

#[test]
fn failed_restore_verification_recovers_the_pre_restore_snapshot() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let io = FsIo;
    let plan = codex_plan(
        "Relay B",
        "https://relay-b.internal/v1",
        "gpt-5.2",
        "CODEX_RELAY_B_KEY",
    );
    let fp = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();
    let switched = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &fp.content_hash,
            expected_rendered_hash: &fp.rendered_hash,
        },
    )
    .unwrap();
    let switched_content = fs::read_to_string(&target).unwrap();

    let mut failing = FailingIo::new();
    failing.corrupt_after_rename = true;
    let err = restore(&failing, &switched.backup, &target).unwrap_err();

    match err {
        SwitchError::CommitFailed {
            stage, recovery, ..
        } => {
            assert_eq!(stage, "restore-verify");
            assert!(matches!(recovery, RecoveryOutcome::Restored { .. }));
        }
        other => panic!("expected restore verification failure, got {other:?}"),
    }
    assert_eq!(fs::read_to_string(&target).unwrap(), switched_content);
}

#[test]
fn restore_rejects_a_temporary_file_that_fails_its_readback_verification() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let io = FsIo;
    let plan = codex_plan(
        "Relay B",
        "https://relay-b.internal/v1",
        "gpt-5.2",
        "CODEX_RELAY_B_KEY",
    );
    let preview = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();
    let switched = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &preview.content_hash,
            expected_rendered_hash: &preview.rendered_hash,
        },
    )
    .unwrap();
    let switched_content = fs::read_to_string(&target).unwrap();
    let failing = FailingIo {
        corrupt_temp_read: true,
        ..FailingIo::new()
    };

    let error = restore(&failing, &switched.backup, &target).unwrap_err();

    assert!(matches!(
        error,
        SwitchError::CommitFailed {
            stage: "restore-temp-verify",
            recovery: RecoveryOutcome::NotNeeded,
            ..
        }
    ));
    assert_eq!(fs::read_to_string(&target).unwrap(), switched_content);
    assert!(!lockfile::lock_path_for(&target).exists());
}

#[test]
fn restore_rejects_a_hash_matching_backup_with_invalid_syntax() {
    let (dir, target, _backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let invalid = "model = [\n";
    let backup_path = dir.path().join("invalid-config.toml.bak");
    fs::write(&backup_path, invalid).unwrap();
    let record = asb_core::BackupRecord {
        id: "invalid".to_string(),
        app: AppKind::Codex,
        target_path: target.to_string_lossy().to_string(),
        backup_path: backup_path.to_string_lossy().to_string(),
        created_at: "2026-09-01T00:00:00Z".to_string(),
        content_hash: sha256_hex(invalid),
        target_existed: true,
        reason: "test".to_string(),
    };

    let error = restore(&FsIo, &record, &target).unwrap_err();

    assert!(matches!(
        error,
        SwitchError::CommitFailed {
            stage: "restore-backup-verify",
            recovery: RecoveryOutcome::NotNeeded,
            ..
        }
    ));
    assert_eq!(fs::read_to_string(&target).unwrap(), CODEX_TOML);
    assert!(!lockfile::lock_path_for(&target).exists());
}

#[test]
fn held_lock_blocks_and_stale_lock_recovery_is_explicit() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let io = FsIo;
    let plan = codex_plan(
        "Relay B",
        "https://relay-b.internal/v1",
        "gpt-5.2",
        "CODEX_RELAY_B_KEY",
    );
    let fp = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();

    // A live foreign holder: this test process itself.
    let lock_path = lockfile::lock_path_for(&target);
    let holder = serde_json::json!({
        "pid": std::process::id(),
        "process_name": "someone-else",
        "acquired_at": "2026-08-26T00:00:00Z",
    });
    fs::write(&lock_path, holder.to_string()).unwrap();

    let err = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &fp.content_hash,
            expected_rendered_hash: &fp.rendered_hash,
        },
    )
    .unwrap_err();
    assert!(matches!(
        err,
        SwitchError::BlockedByLock {
            status: asb_core::LockStatus::Held(_)
        }
    ));
    // The held lock was not deleted.
    assert!(lock_path.exists());

    // Use a Windows-invalid PID instead of a just-reaped one: the OS can
    // immediately reuse an exited PID and make this assertion flaky.
    let dead_pid = u32::MAX;
    let stale = serde_json::json!({
        "pid": dead_pid,
        "process_name": "gone",
        "acquired_at": "2026-08-01T00:00:00Z",
    });
    fs::write(&lock_path, stale.to_string()).unwrap();
    let err = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &fp.content_hash,
            expected_rendered_hash: &fp.rendered_hash,
        },
    )
    .unwrap_err();
    assert!(matches!(
        err,
        SwitchError::BlockedByLock {
            status: asb_core::LockStatus::Stale(_)
        }
    ));
    assert!(
        lock_path.exists(),
        "stale lock must not be removed implicitly"
    );

    // Explicit recovery removes it and is reported.
    let entry = lockfile::recover_stale(&io, &target).unwrap();
    assert_eq!(entry.removed_holder_pid, Some(dead_pid));
    assert!(!lock_path.exists());

    // Now the switch succeeds.
    execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &fp.content_hash,
            expected_rendered_hash: &fp.rendered_hash,
        },
    )
    .unwrap();
    assert!(fs::read_to_string(&target)
        .unwrap()
        .contains("relay-b.internal"));
}

#[test]
fn indeterminate_lock_blocks_without_recovery() {
    let (_dir, target, _backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let io = FsIo;
    let lock_path = lockfile::lock_path_for(&target);
    fs::write(&lock_path, "not json at all").unwrap();

    let status = lockfile::probe_lock(&io, &target);
    assert!(matches!(status, asb_core::LockStatus::Indeterminate { .. }));
    // Recovery refuses anything that is not classified stale.
    let refused = lockfile::recover_stale(&io, &target).unwrap_err();
    assert!(matches!(
        refused,
        asb_core::LockStatus::Indeterminate { .. }
    ));
    assert!(lock_path.exists());
}

#[test]
fn missing_lock_is_reported_as_free() {
    let (_dir, target, _backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    assert!(matches!(
        lockfile::probe_lock(&FsIo, &target),
        asb_core::LockStatus::Free
    ));
}

#[test]
fn outcome_reports_lock_changes_warnings_backup_and_recovery() {
    let (_dir, target, backup_dir) = setup(AppKind::Claude, CLAUDE_JSON);
    let io = FsIo;
    let plan = claude_plan("Relay C", "https://relay-c.internal", "claude-opus-4");
    let fp = read_preview(&io, &target, &plan, &backup_dir.to_string_lossy()).unwrap();

    let outcome = execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan,
            backup_dir: &backup_dir,
            expected_hash: &fp.content_hash,
            expected_rendered_hash: &fp.rendered_hash,
        },
    )
    .unwrap();

    assert!(matches!(outcome.lock, asb_core::LockStatus::Free));
    assert!(!outcome.acquired_at.is_empty());
    assert_eq!(outcome.changed.len(), 1);
    // The sample carries env.ANTHROPIC_MODEL, which overrides `model`; the
    // adapter removes it and warns.
    assert!(outcome
        .warnings
        .iter()
        .any(|w| w.contains("ANTHROPIC_MODEL")));
    assert!(Path::new(&outcome.backup.backup_path).exists());
    assert!(matches!(outcome.recovery, RecoveryOutcome::NotNeeded));
    // Preview in the outcome matches what the user confirmed.
    assert_eq!(outcome.preview, fp.preview);
    // The final hash describes the content that is now live.
    assert_eq!(
        outcome.final_hash,
        sha256_hex(&fs::read_to_string(&target).unwrap())
    );
}

#[test]
fn list_backups_returns_records_with_hashes() {
    let (_dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let io = FsIo;
    let plan_a = codex_plan("Relay A2", "https://a2.internal/v1", "m-a", "GATEWAY_A_KEY");
    let plan_b = codex_plan(
        "Relay B",
        "https://relay-b.internal/v1",
        "gpt-5.2",
        "CODEX_RELAY_B_KEY",
    );
    let fp = read_preview(&io, &target, &plan_a, &backup_dir.to_string_lossy()).unwrap();
    execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan_a,
            backup_dir: &backup_dir,
            expected_hash: &fp.content_hash,
            expected_rendered_hash: &fp.rendered_hash,
        },
    )
    .unwrap();
    let fp2 = read_preview(&io, &target, &plan_b, &backup_dir.to_string_lossy()).unwrap();
    execute(
        &io,
        &asb_switch::SwitchRequest {
            target: &target,
            plan: &plan_b,
            backup_dir: &backup_dir,
            expected_hash: &fp2.content_hash,
            expected_rendered_hash: &fp2.rendered_hash,
        },
    )
    .unwrap();

    let records = asb_switch::list_backups(&io, &backup_dir);
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|r| r.content_hash.len() == 64));
}

#[test]
fn backup_listing_ignores_metadata_that_points_outside_its_sidecar() {
    let (dir, target, backup_dir) = setup(AppKind::Codex, CODEX_TOML);
    let io = FsIo;
    fs::create_dir_all(&backup_dir).unwrap();
    let outside = dir.path().join("outside.bak");
    fs::write(&outside, "not a backup").unwrap();
    let forged = asb_core::BackupRecord {
        id: "forged".to_string(),
        app: AppKind::Codex,
        target_path: target.to_string_lossy().to_string(),
        backup_path: outside.to_string_lossy().to_string(),
        created_at: "2026-08-26T00:00:00Z".to_string(),
        content_hash: sha256_hex("not a backup"),
        target_existed: true,
        reason: "switch".to_string(),
    };
    fs::write(
        backup_dir.join("forged.meta.json"),
        serde_json::to_string(&forged).unwrap(),
    )
    .unwrap();

    assert!(asb_switch::list_backups(&io, &backup_dir).is_empty());
}
