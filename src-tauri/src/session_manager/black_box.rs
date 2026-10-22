use super::*;
use base64::Engine;
use serde_json::json;
use std::process::Command;

const A: &str = "a1111111-2222-4333-8444-555555555555";
const B: &str = "b1111111-2222-4333-8444-555555555555";
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aGN8AAAAASUVORK5CYII=";
const CREDENTIAL: &str = "sandbox-credential-sentinel-not-a-real-token";

fn write_account(root: &Path, account: &str, id: &str) {
    let directory = root.join(account);
    fs::create_dir_all(directory.join("projects/project")).unwrap();
    fs::write(directory.join(".credentials.json"), CREDENTIAL).unwrap();
    let image = directory.join("picture.png");
    fs::write(&image, base64::engine::general_purpose::STANDARD.decode(PNG).unwrap()).unwrap();
    let value = json!({"type":"user","sessionId":id,"uuid":id,"cwd":root.join("project"),
        "timestamp":"2026-10-22T01:00:00Z","message":{"role":"user","content":[
            {"type":"text","text":format!("沙箱跨账号完整正文 {account}")},{"type":"image","path":image}]}});
    fs::write(directory.join("projects/project").join(format!("{id}.jsonl")), format!("{value}\n")).unwrap();
}

fn subprocess(root: &Path, phase: &str) {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", "session_manager::black_box::process_driver", "--ignored", "--test-threads=1", "--nocapture"])
        .env("ASB_RETENTION_TEST_ROOT", root).env("ASB_RETENTION_TEST_PHASE", phase);
    for (name, relative) in [("HOME","home"),("USERPROFILE","home"),("APPDATA","appdata"),
        ("LOCALAPPDATA","localappdata"),("CODEX_HOME","codex"),("CODEX_SQLITE_HOME","codex"),
        ("CLAUDE_CONFIG_DIR","account A"),("TEMP","temp"),("TMP","temp"),("TMPDIR","temp"),
        ("XDG_CONFIG_HOME","home/.config"),("XDG_DATA_HOME","home/.local/share"),("XDG_CACHE_HOME","home/.cache")] {
        fs::create_dir_all(root.join(relative)).unwrap();
        command.env(name, root.join(relative));
    }
    #[cfg(windows)]
    { use std::os::windows::process::CommandExt; command.creation_flags(0x0800_0000); }
    let output = command.output().unwrap();
    assert!(output.status.success(), "{phase}:\n{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}

#[test]
fn public_api_survives_account_switch_original_loss_restart_and_offline_reappearance() {
    let sandbox = tempfile::tempdir().unwrap();
    let root = sandbox.path();
    write_account(root, "account A", A);
    write_account(root, "account B", B);
    subprocess(root, "capture");
    assert_credentials(root);
    for account in ["account A", "account B"] {
        let source = root.join(account).join("projects");
        assert!(source.canonicalize().unwrap().starts_with(root.canonicalize().unwrap()));
        fs::remove_dir_all(source).unwrap();
    }
    subprocess(root, "read-delete");
    assert_credentials(root);
    write_account(root, "account A", A);
    write_account(root, "account B", B);
    subprocess(root, "deleted-restart");
    assert_credentials(root);
}

fn assert_credentials(root: &Path) {
    for account in ["account A", "account B"] {
        assert_eq!(fs::read_to_string(root.join(account).join(".credentials.json")).unwrap(), CREDENTIAL);
    }
}

#[test]
fn public_api_reports_corrupt_storage_and_rejects_invalid_inputs() {
    let sandbox = tempfile::tempdir().unwrap();
    subprocess(sandbox.path(), "corrupt");
}

fn search(root: &Path, query: &str) -> SessionSearchPage {
    search_sessions(root, SessionSearchRequest { app: Some(AppKind::Claude), query: query.into(), ..Default::default() }).unwrap()
}

fn page(root: &Path, id: &str) -> MessagePage {
    load_page(root, MessagePageRequest { app: AppKind::Claude, session_id: id.into(), offset: 0, anchor_id: None }).unwrap()
}

fn capture(root: &Path, state: &Path) {
    assert_eq!(search(state, "").total, 1);
    assert!(history::preserve_before_switch(state).unwrap().is_empty());
    std::env::set_var("CLAUDE_CONFIG_DIR", root.join("account B"));
    let both = search(state, "沙箱跨账号完整正文");
    assert_eq!(both.total, 2);
    assert!(both.issues.is_empty());
    let page = page(state, A);
    assert_eq!(page.total, 1);
    assert!(page.session.preservation.as_ref().unwrap().source_available);
    let bookmark = save_bookmark(state, AppKind::Claude, A, &page.items[0].id).unwrap();
    fs::write(root.join("bookmark-id"), bookmark.id).unwrap();
    let output = root.join("before-switch.md");
    export_markdown(state, AppKind::Claude, A, &output).unwrap();
    let markdown = fs::read_to_string(output).unwrap();
    assert!(markdown.contains("沙箱跨账号完整正文 account A"));
    assert!(markdown.contains(PNG));
}

fn read_and_delete(root: &Path, state: &Path) {
    assert_eq!(search(state, "沙箱跨账号完整正文").total, 2);
    let page = page(state, A);
    assert!(!page.session.preservation.as_ref().unwrap().source_available);
    let id = &page.items[0].id;
    let content = load_content(state, MessageContentRequest { app: AppKind::Claude,
        session_id: A.into(), message_id: id.clone(), offset: 0 }).unwrap();
    assert!(content.content.contains("account A"));
    assert_eq!(content.images, vec![format!("data:image/png;base64,{PNG}")]);
    let copied = load_copy_text(state, SessionCopySource::Session { app: AppKind::Claude,
        session_id: A.into(), message_id: id.clone() }).unwrap();
    assert!(copied.contains("account A") && copied.contains(PNG));
    export_markdown(state, AppKind::Claude, A, &root.join("after-loss.md")).unwrap();
    assert!(export_markdown(state, AppKind::Claude, A, &state.join("sessions/claude.sqlite3")).is_err());
    delete_session(state, AppKind::Claude, A).unwrap();
    let requests = [SessionDeleteRequest { app: AppKind::Claude, session_id: B.into() },
        SessionDeleteRequest { app: AppKind::Claude, session_id: B.into() }];
    let outcomes = delete_sessions(state, &requests).unwrap();
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].deleted && outcomes[0].error.is_none());
    assert_eq!(search(state, "").total, 0);
    assert_bookmark(root, state);
}

fn assert_bookmark(root: &Path, state: &Path) {
    let id = fs::read_to_string(root.join("bookmark-id")).unwrap();
    assert_eq!(search_bookmarks(state, BookmarkSearchRequest::default()).unwrap().total, 1);
    let bookmark = load_bookmark_content(state, &id, 0).unwrap();
    assert!(bookmark.content.contains("account A"));
    assert_eq!(bookmark.images, vec![format!("data:image/png;base64,{PNG}")]);
}

fn corrupt(root: &Path, state: &Path) {
    fs::create_dir_all(state.join("sessions")).unwrap();
    let path = state.join("sessions/claude.sqlite3");
    fs::write(&path, b"corrupt sandbox database").unwrap();
    let result = search(state, "");
    assert!(result.issues.iter().any(|issue| issue.message.contains("保留库")));
    assert_eq!(fs::read(path).unwrap(), b"corrupt sandbox database");
    assert!(load_metadata(state, AppKind::Claude, "../../outside").is_err());
    assert!(search_sessions(state, SessionSearchRequest { query: "x".repeat(4001), ..Default::default() }).is_err());
    assert!(load_page(state, MessagePageRequest { app: AppKind::Claude, session_id: A.into(), offset: 1, anchor_id: None }).is_err());
    assert!(history::add_root(state, root).is_err());
}

#[test]
#[ignore = "Subprocess driver; requires the owning black-box fixture"]
fn process_driver() {
    let root = PathBuf::from(std::env::var_os("ASB_RETENTION_TEST_ROOT").expect("isolated fixture root"));
    assert!(root.is_absolute());
    for key in ["USERPROFILE", "APPDATA", "LOCALAPPDATA", "CODEX_HOME", "CODEX_SQLITE_HOME", "CLAUDE_CONFIG_DIR", "TEMP"] {
        assert!(PathBuf::from(std::env::var_os(key).unwrap()).starts_with(&root), "unisolated {key}");
    }
    let state = root.join("state");
    match std::env::var("ASB_RETENTION_TEST_PHASE").unwrap().as_str() {
        "capture" => capture(&root, &state),
        "read-delete" => read_and_delete(&root, &state),
        "deleted-restart" => { assert_eq!(search(&state, "").total, 0); assert_bookmark(&root, &state); }
        "corrupt" => corrupt(&root, &state),
        phase => panic!("unsupported sandbox phase {phase}"),
    }
}
