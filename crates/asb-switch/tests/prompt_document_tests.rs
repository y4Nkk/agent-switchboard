//! Global prompt documents are always exercised in temporary directories; no
//! test reaches a real Codex or Claude Code user directory.

use asb_core::AppKind;
use asb_switch::io::{FsIo, SwitchIo};
use asb_switch::{
    read_global_prompt_document, write_global_prompt_document, GlobalPromptDocumentRequest,
    RecoveryOutcome, SwitchError,
};
use std::cell::Cell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().expect("test target has parent")).expect("create test parent");
    fs::write(path, content).expect("write test document");
}

fn request<'a>(
    target: &'a Path,
    backup_dir: &'a Path,
    content: &'a str,
    expected_hash: &'a str,
) -> GlobalPromptDocumentRequest<'a> {
    GlobalPromptDocumentRequest {
        target,
        app: AppKind::Codex,
        content,
        backup_dir,
        expected_hash,
    }
}

#[test]
fn saves_a_global_agents_document_with_an_isolated_backup() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let target = directory.path().join("codex").join("AGENTS.md");
    let backup_dir = directory.path().join("backups").join("prompts");
    write(&target, "# Previous instructions\n");
    let io = FsIo;
    let before = read_global_prompt_document(&io, &target, AppKind::Codex).expect("read document");

    let outcome = write_global_prompt_document(
        &io,
        &request(
            &target,
            &backup_dir,
            "# Current instructions\n- Run targeted tests.\n",
            &before.content_hash,
        ),
    )
    .expect("save document");

    assert_eq!(outcome.document.app, AppKind::Codex);
    assert_eq!(outcome.document.file_name, "AGENTS.md");
    assert!(outcome.document.exists);
    assert_eq!(
        fs::read_to_string(&target).expect("read saved document"),
        "# Current instructions\n- Run targeted tests.\n"
    );
    assert_eq!(
        fs::read_to_string(&outcome.backup.backup_path).expect("read backup"),
        "# Previous instructions\n"
    );
    assert_eq!(outcome.backup.reason, "prompt-management");
    assert!(Path::new(&format!("{}.meta.json", outcome.backup.backup_path)).exists());
}

#[test]
fn refuses_to_overwrite_a_prompt_document_changed_after_read() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let target = directory.path().join("codex").join("AGENTS.md");
    let backup_dir = directory.path().join("backups").join("prompts");
    write(&target, "# First version\n");
    let io = FsIo;
    let before = read_global_prompt_document(&io, &target, AppKind::Codex).expect("read document");
    write(&target, "# External version\n");

    let error = write_global_prompt_document(
        &io,
        &request(
            &target,
            &backup_dir,
            "# Local draft\n",
            &before.content_hash,
        ),
    )
    .expect_err("changed file must be rejected");

    assert!(matches!(error, SwitchError::ExternalChange { .. }));
    assert_eq!(
        fs::read_to_string(&target).expect("read unchanged external content"),
        "# External version\n"
    );
    assert!(!backup_dir.exists());
}

#[test]
fn creates_a_missing_document_and_marks_its_backup_as_absent_target() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let target = directory.path().join("codex").join("AGENTS.md");
    let backup_dir = directory.path().join("backups").join("prompts");
    let io = FsIo;
    let before =
        read_global_prompt_document(&io, &target, AppKind::Codex).expect("read missing document");

    assert!(!before.exists);
    let outcome = write_global_prompt_document(
        &io,
        &request(
            &target,
            &backup_dir,
            "# New global prompt\n",
            &before.content_hash,
        ),
    )
    .expect("create document");

    assert!(!outcome.backup.target_existed);
    assert_eq!(
        fs::read_to_string(&outcome.backup.backup_path).expect("read empty creation backup"),
        ""
    );
    assert_eq!(
        fs::read_to_string(&target).expect("read created document"),
        "# New global prompt\n"
    );
}

/// Injects one bad post-replace read so the executor must restore its backup.
struct CorruptOnceIo {
    replaced: Cell<bool>,
    corrupted: Cell<bool>,
}

impl CorruptOnceIo {
    fn new() -> Self {
        Self {
            replaced: Cell::new(false),
            corrupted: Cell::new(false),
        }
    }

    fn is_target(path: &Path) -> bool {
        path.file_name().is_some_and(|name| name == "AGENTS.md")
    }
}

impl SwitchIo for CorruptOnceIo {
    fn read_file(&self, path: &Path) -> io::Result<String> {
        let content = fs::read_to_string(path)?;
        if self.replaced.get() && !self.corrupted.replace(true) && Self::is_target(path) {
            return Ok(format!("{content}\ncorrupt"));
        }
        Ok(content)
    }

    fn write_new_file(&self, path: &Path, content: &str) -> io::Result<()> {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(content.as_bytes())
    }

    fn write_file_replace(&self, path: &Path, content: &str) -> io::Result<()> {
        fs::write(path, content)
    }

    fn rename_replace(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)?;
        if Self::is_target(to) {
            self.replaced.set(true);
        }
        Ok(())
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn ensure_dir(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    fn list_dir(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
        fs::read_dir(path)?
            .map(|entry| entry.map(|item| item.path()))
            .collect()
    }

    fn now_rfc3339(&self) -> String {
        FsIo.now_rfc3339()
    }
}

#[test]
fn restores_the_prewrite_backup_when_post_write_verification_fails() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let target = directory.path().join("codex").join("AGENTS.md");
    let backup_dir = directory.path().join("backups").join("prompts");
    write(&target, "# Previous instructions\n");
    let io = CorruptOnceIo::new();
    let before = read_global_prompt_document(&io, &target, AppKind::Codex).expect("read document");

    let error = write_global_prompt_document(
        &io,
        &request(
            &target,
            &backup_dir,
            "# Candidate instructions\n",
            &before.content_hash,
        ),
    )
    .expect_err("verification failure must restore");

    assert!(matches!(
        error,
        SwitchError::CommitFailed {
            stage: "post-verify",
            recovery: RecoveryOutcome::Restored { .. },
            ..
        }
    ));
    assert_eq!(
        fs::read_to_string(&target).expect("read restored document"),
        "# Previous instructions\n"
    );
}
