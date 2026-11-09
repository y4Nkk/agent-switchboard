use super::*;
use std::{cell::Cell, fs, io::{BufRead, BufReader, Read, Write}};

struct Fixture { _directory: tempfile::TempDir, state: PathBuf, scope: Scope, db: rusqlite::Connection }

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let scope = Scope { home: root.join("codex"), sqlite: root.join("native-db") };
        fs::create_dir_all(scope.home.join("sessions")).unwrap();
        fs::create_dir_all(&scope.sqlite).unwrap();
        let db = rusqlite::Connection::open(scope.database()).unwrap();
        db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE threads(id TEXT PRIMARY KEY,model_provider TEXT NOT NULL,rollout_path TEXT NOT NULL,archived INTEGER NOT NULL,title TEXT NOT NULL,updated_at INTEGER NOT NULL);").unwrap();
        Self { _directory: directory, state: root.join("asb"), scope, db }
    }

    fn file(&self, id: &str, provider: Option<&str>, archived: bool, compressed: bool) -> PathBuf {
        let directory = self.scope.home.join(if archived { "archived_sessions" } else { "sessions" });
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!("{id}.jsonl{}", if compressed { ".zst" } else { "" }));
        let mut meta = serde_json::json!({"type":"session_meta","payload":{"id":id,"cwd":"/keep/project","model":"keep-model","custom":{"keep":true}}});
        if let Some(provider) = provider { meta["payload"]["model_provider"] = serde_json::json!(provider); }
        let bytes = format!("{}\r\n{{\"type\":\"response_item\",\"payload\":{{\"text\":\"model_provider=custom must stay\"}}}}\n", meta).into_bytes();
        let bytes = if compressed { zstd::stream::encode_all(&bytes[..], 1).unwrap() } else { bytes };
        fs::write(&path, bytes).unwrap();
        path
    }

    fn register(&self, id: &str, provider: &str, path: &Path, archived: bool) {
        self.db.execute("INSERT INTO threads VALUES(?1,?2,?3,?4,'keep-title',123)",
            rusqlite::params![id, provider, path.display().to_string(), archived]).unwrap();
    }

    fn preview(&self) -> Preview { preview_at(&self.state, &self.scope, &|| Ok(())).unwrap() }
    fn apply(&self, id: &str) -> Operation { transaction::execute(&self.state, &self.scope, id, &|| Ok(())).unwrap() }
}

fn decoded(path: &Path) -> (serde_json::Value, Vec<u8>) {
    let mut reader = BufReader::new(super::super::record_file::decode(path, fs::File::open(path).unwrap()).unwrap());
    let mut first = Vec::new(); reader.read_until(b'\n', &mut first).unwrap();
    let mut body = Vec::new(); reader.read_to_end(&mut body).unwrap();
    (serde_json::from_slice(&first).unwrap(), body)
}

#[test]
fn mixed_native_and_compressed_history_transfers_and_restores_only_provider_fields() {
    let fixture = Fixture::new();
    let plain = fixture.file("thread-a", Some("custom"), false, false);
    let archive = fixture.file("thread-b", Some("OpenAI"), true, true);
    let official = fixture.file("thread-c", Some(TARGET), false, false);
    let orphan = fixture.file("thread-d", None, false, false);
    fixture.register("thread-a", "custom", &plain, false);
    fixture.register("thread-b", "OpenAI", &archive, true);
    fixture.register("thread-c", TARGET, &official, false);
    fixture.register("thread-e", "cc-switch", &fixture.scope.home.join("missing.jsonl"), true);
    let original: Vec<_> = [&plain, &archive, &official, &orphan].iter().map(|p| fs::read(p).unwrap()).collect();
    let bodies: Vec<_> = [&plain, &archive, &official, &orphan].iter().map(|p| decoded(p).1).collect();
    let preview = fixture.preview();
    assert_eq!((preview.sessions, preview.database_rows, preview.files, preview.missing_originals), (4, 3, 3, 1));
    fixture.apply(&preview.id);
    let rows = database::read(&fixture.db).unwrap();
    assert!(rows.iter().all(|row| row.provider == TARGET));
    assert!(rows.iter().find(|row| row.id == "thread-b").unwrap().archived);
    for (index, path) in [&plain, &archive, &official, &orphan].iter().enumerate() {
        let (header, body) = decoded(path);
        assert_eq!(header["payload"]["model_provider"], TARGET);
        assert_eq!(header["payload"]["custom"]["keep"], true);
        assert_eq!(body, bodies[index]);
    }
    assert_eq!(fs::read(&official).unwrap(), original[2]);
    let title: (String, i64) = fixture.db.query_row("SELECT title,updated_at FROM threads WHERE id='thread-b'", [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
    assert_eq!(title, ("keep-title".into(), 123));
    fixture.db.execute("UPDATE threads SET title='later-title' WHERE id='thread-a'", []).unwrap();
    transaction::restore(&fixture.state, &fixture.scope, &preview.id, &|| Ok(())).unwrap();
    for (index, path) in [&plain, &archive, &official, &orphan].iter().enumerate() { assert_eq!(fs::read(path).unwrap(), original[index]); }
    let restored = database::read(&fixture.db).unwrap();
    assert_eq!(restored[0].provider, "custom");
    assert_eq!(restored[1].provider, "OpenAI");
    assert_eq!(restored[3].provider, "cc-switch");
    let title: String = fixture.db.query_row("SELECT title FROM threads WHERE id='thread-a'", [], |r| r.get(0)).unwrap();
    assert_eq!(title, "later-title");
    assert_eq!(journal::latest(&fixture.state).unwrap().unwrap().phase, Phase::Restored);
}

#[test]
fn changed_sources_and_expired_previews_never_start_a_native_write() {
    let fixture = Fixture::new();
    let path = fixture.file("thread-a", Some("custom"), false, false);
    fixture.register("thread-a", "custom", &path, false);
    let preview = fixture.preview();
    fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"{\"type\":\"response_item\",\"payload\":{\"text\":\"new event\"}}\n").unwrap();
    assert!(matches!(transaction::execute(&fixture.state, &fixture.scope, &preview.id, &|| Ok(())), Err(Error::Changed)));
    assert!(journal::latest(&fixture.state).unwrap().is_none());
    assert_eq!(database::read(&fixture.db).unwrap()[0].provider, "custom");
    let preview = fixture.preview();
    let path = journal::plan_path(&fixture.state);
    let mut plan: journal::Plan = journal::load(&path).unwrap();
    plan.created_at -= 3600;
    journal::save(&path, &plan).unwrap();
    assert!(matches!(transaction::execute(&fixture.state, &fixture.scope, &preview.id, &|| Ok(())), Err(Error::Changed)));
    plan.created_at = chrono::Utc::now().timestamp() + 3600;
    journal::save(&path, &plan).unwrap();
    assert!(matches!(transaction::execute(&fixture.state, &fixture.scope, &preview.id, &|| Ok(())), Err(Error::Changed)));
    assert!(journal::load_plan(&fixture.state, "../../config.toml").is_err());
}

#[test]
fn partial_file_publication_is_automatically_restored() {
    let fixture = Fixture::new();
    let first = fixture.file("a", Some("custom"), false, false);
    fixture.file("b", Some("other"), false, true);
    fixture.register("a", "custom", &first, false);
    let preview = fixture.preview();
    let failed = Cell::new(false);
    let check = || {
        if !failed.get() && transcript::inspect(&first)?.provider.as_deref() == Some(TARGET) {
            failed.set(true); return Err(Error::Failure("injected publication failure".into()));
        }
        Ok(())
    };
    assert!(transaction::execute(&fixture.state, &fixture.scope, &preview.id, &check).is_err());
    assert!(failed.get());
    assert_eq!(transcript::inspect(&first).unwrap().provider.as_deref(), Some("custom"));
    assert_eq!(database::read(&fixture.db).unwrap()[0].provider, "custom");
    assert_eq!(journal::latest(&fixture.state).unwrap().unwrap().phase, Phase::Restored);
}

#[test]
fn interrupted_recovery_is_visible_after_reload_and_can_be_completed() {
    let fixture = Fixture::new();
    let first = fixture.file("a", Some("custom"), false, false);
    let second = fixture.file("b", Some("other"), false, false);
    let preview = fixture.preview();
    let permit_recovery = Cell::new(false);
    let check = || {
        if !permit_recovery.get() && transcript::inspect(&first)?.provider.as_deref() == Some(TARGET) {
            return Err(Error::Busy(Some(42)));
        }
        Ok(())
    };
    assert!(matches!(transaction::execute(&fixture.state, &fixture.scope, &preview.id, &check), Err(Error::Recovery(_))));
    assert_eq!(latest(&fixture.state).unwrap().unwrap().phase, Phase::Applying);
    assert!(preview_at(&fixture.state, &fixture.scope, &|| Ok(())).is_err());
    assert!(matches!(ensure_settled(&fixture.state), Err(Error::Recovery(_))));
    let temporary = transcript::temporary(&second, &preview.id);
    fs::write(&temporary, b"interrupted partial publication").unwrap();
    permit_recovery.set(true);
    transaction::restore(&fixture.state, &fixture.scope, &preview.id, &check).unwrap();
    assert_eq!(transcript::inspect(&first).unwrap().provider.as_deref(), Some("custom"));
    assert_eq!(latest(&fixture.state).unwrap().unwrap().phase, Phase::Restored);
    assert!(!temporary.exists());
    ensure_settled(&fixture.state).unwrap();
}

#[test]
fn earlier_backups_cannot_override_the_latest_operation() {
    let fixture = Fixture::new();
    let path = fixture.file("a", Some("custom"), false, false);
    let first = fixture.preview(); fixture.apply(&first.id);
    transaction::restore(&fixture.state, &fixture.scope, &first.id, &|| Ok(())).unwrap();
    let second = fixture.preview(); fixture.apply(&second.id);
    let empty = fixture.preview();
    assert_eq!(empty.sessions, 0);
    assert!(transaction::execute(&fixture.state, &fixture.scope, &empty.id, &|| Ok(())).is_err());
    assert!(transaction::restore(&fixture.state, &fixture.scope, &first.id, &|| Ok(())).is_err());
    assert_eq!(transcript::inspect(&path).unwrap().provider.as_deref(), Some(TARGET));
    assert_eq!(latest(&fixture.state).unwrap().unwrap().preview.id, second.id);
}

#[test]
fn restoration_rejects_later_message_or_provider_changes_without_overwriting_them() {
    let fixture = Fixture::new();
    let path = fixture.file("a", Some("custom"), false, false);
    fixture.register("a", "custom", &path, false);
    let preview = fixture.preview(); fixture.apply(&preview.id);
    fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"{\"type\":\"response_item\",\"payload\":{\"text\":\"later event\"}}\n").unwrap();
    let later = fs::read(&path).unwrap();
    assert!(transaction::restore(&fixture.state, &fixture.scope, &preview.id, &|| Ok(())).is_err());
    assert_eq!(fs::read(&path).unwrap(), later);
    assert_eq!(database::read(&fixture.db).unwrap()[0].provider, TARGET);
}

#[test]
fn inconsistent_registered_identities_stop_the_preview() {
    let fixture = Fixture::new();
    let path = fixture.file("actual-id", Some("custom"), false, false);
    fixture.register("different-id", "custom", &path, false);
    let original = fs::read(&path).unwrap();
    assert!(preview_at(&fixture.state, &fixture.scope, &|| Ok(())).is_err());
    assert_eq!(fs::read(path).unwrap(), original);
    assert_eq!(database::read(&fixture.db).unwrap()[0].provider, "custom");
}

#[test]
fn later_provider_changes_stop_restoration_before_any_transcript_is_replaced() {
    let fixture = Fixture::new();
    let path = fixture.file("a", Some("custom"), false, false);
    fixture.register("a", "custom", &path, false);
    let preview = fixture.preview(); fixture.apply(&preview.id);
    fixture.db.execute("UPDATE threads SET model_provider='later-provider' WHERE id='a'", []).unwrap();
    let current = fs::read(&path).unwrap();
    assert!(matches!(transaction::restore(&fixture.state, &fixture.scope, &preview.id, &|| Ok(())), Err(Error::Conflict(_))));
    assert_eq!(fs::read(path).unwrap(), current);
    assert_eq!(database::read(&fixture.db).unwrap()[0].provider, "later-provider");
}

#[test]
fn registered_external_transcripts_are_verified_and_included_in_the_preview() {
    let fixture = Fixture::new();
    let original = fixture.file("a", Some("custom"), false, false);
    let external = fixture.scope.sqlite.join("registered-external.jsonl");
    fs::rename(original, &external).unwrap();
    fixture.register("a", "custom", &external, false);
    let preview = fixture.preview();
    assert_eq!(preview.external_files, vec![external.display().to_string()]);
    fixture.apply(&preview.id);
    assert_eq!(transcript::inspect(&external).unwrap().provider.as_deref(), Some(TARGET));
    transaction::restore(&fixture.state, &fixture.scope, &preview.id, &|| Ok(())).unwrap();
    assert_eq!(transcript::inspect(&external).unwrap().provider.as_deref(), Some("custom"));
}

#[test]
fn file_only_history_does_not_create_a_native_database() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let scope = Scope { home: root.join("codex"), sqlite: root.join("metadata") };
    let state = root.join("asb");
    fs::create_dir_all(scope.home.join("sessions")).unwrap();
    let path = scope.home.join("sessions/a.jsonl");
    fs::write(&path, b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"a\",\"model_provider\":\"custom\"}}\n").unwrap();
    let preview = preview_at(&state, &scope, &|| Ok(())).unwrap();
    assert!(preview.database_path.is_none());
    transaction::execute(&state, &scope, &preview.id, &|| Ok(())).unwrap();
    assert!(!scope.database().exists());
    transaction::restore(&state, &scope, &preview.id, &|| Ok(())).unwrap();
    assert!(!scope.database().exists());
    assert_eq!(transcript::inspect(&path).unwrap().provider.as_deref(), Some("custom"));
}

#[test]
fn a_new_preview_invalidates_the_old_id_without_creating_a_backup() {
    let fixture = Fixture::new();
    let path = fixture.file("a", Some("custom"), false, false);
    let original = fs::read(&path).unwrap();
    let first = fixture.preview();
    let second = fixture.preview();
    assert!(matches!(transaction::execute(&fixture.state, &fixture.scope, &first.id, &|| Ok(())), Err(Error::Changed)));
    assert!(!journal::directory(&fixture.state, &first.id).unwrap().exists());
    assert_eq!(journal::load_plan(&fixture.state, &second.id).unwrap().view.id, second.id);
    assert_eq!(fs::read(path).unwrap(), original);
    assert!(latest(&fixture.state).unwrap().is_none());
}

#[test]
fn failed_preparation_discards_only_its_staging_without_touching_native_history() {
    let fixture = Fixture::new();
    let path = fixture.file("a", Some("custom"), false, false);
    fixture.register("a", "custom", &path, false);
    let original = fs::read(&path).unwrap();
    let preview = fixture.preview();
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() == 2 { return Err(Error::Failure("injected pre-publication failure".into())); }
        Ok(())
    };
    assert!(transaction::execute(&fixture.state, &fixture.scope, &preview.id, &check).is_err());
    assert!(!journal::directory(&fixture.state, &preview.id).unwrap().exists());
    assert!(latest(&fixture.state).unwrap().is_none());
    assert_eq!(fs::read(path).unwrap(), original);
    assert_eq!(database::read(&fixture.db).unwrap()[0].provider, "custom");
}

#[test]
fn a_corrupted_publication_material_never_replaces_the_transcript() {
    let fixture = Fixture::new();
    let path = fixture.file("a", Some("custom"), false, false);
    let original = fs::read(&path).unwrap();
    let material = fixture.scope.sqlite.join("candidate.jsonl");
    transcript::candidate(&path, &material, "a", Some("custom")).unwrap();
    let after = transcript::digest(&material).unwrap();
    fs::write(&material, b"corrupt publication material").unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let before = transcript::digest(&path).unwrap();
    assert!(transcript::replace(&material, &path, &before, &after, &id).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(!transcript::temporary(&path, &id).exists());
}

#[test]
fn candidate_generation_rejects_provider_or_identity_drift() {
    let fixture = Fixture::new();
    let path = fixture.file("a", Some("custom"), false, true);
    let material = fixture.scope.sqlite.join("candidate.jsonl.zst");
    for (id, provider) in [("a", Some("different")), ("different-id", Some("custom"))] {
        assert!(matches!(transcript::candidate(&path, &material, id, provider), Err(Error::Changed)));
        assert!(!material.exists());
    }
}

#[test]
fn repeating_a_finished_restore_preserves_later_changes() {
    let fixture = Fixture::new();
    let path = fixture.file("a", Some("custom"), false, false);
    fixture.register("a", "custom", &path, false);
    let preview = fixture.preview(); fixture.apply(&preview.id);
    transaction::restore(&fixture.state, &fixture.scope, &preview.id, &|| Ok(())).unwrap();
    fixture.db.execute("UPDATE threads SET model_provider='later-provider' WHERE id='a'", []).unwrap();
    fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"{\"type\":\"response_item\",\"payload\":{\"text\":\"later event\"}}\n").unwrap();
    let later = fs::read(&path).unwrap();
    let repeated = transaction::restore(&fixture.state, &fixture.scope, &preview.id, &|| Ok(())).unwrap();
    assert_eq!(repeated.phase, Phase::Restored);
    assert_eq!(fs::read(path).unwrap(), later);
    assert_eq!(database::read(&fixture.db).unwrap()[0].provider, "later-provider");
}

#[test]
fn changed_registration_is_reported_before_restoration_writes_any_file() {
    let fixture = Fixture::new();
    let path = fixture.file("a", Some("custom"), false, false);
    fixture.register("a", "custom", &path, false);
    let preview = fixture.preview(); fixture.apply(&preview.id);
    let applied = fs::read(&path).unwrap();
    let replacement = path.with_file_name("new-registration.jsonl");
    fs::write(&replacement, &applied).unwrap();
    fixture.db.execute("UPDATE threads SET rollout_path=?1 WHERE id='a'", [replacement.display().to_string()]).unwrap();
    let error = transaction::restore(&fixture.state, &fixture.scope, &preview.id, &|| Ok(())).unwrap_err();
    assert!(matches!(&error, Error::Conflict(_)));
    assert!(error.to_string().contains("Thread a"));
    assert_eq!(fs::read(path).unwrap(), applied);
    assert_eq!(database::read(&fixture.db).unwrap()[0].provider, TARGET);
}

#[test]
fn an_unreadable_backup_reports_its_path_and_preserves_the_applied_history() {
    let fixture = Fixture::new();
    let path = fixture.file("a", Some("custom"), false, false);
    let preview = fixture.preview(); fixture.apply(&preview.id);
    let record = journal::latest(&fixture.state).unwrap().unwrap();
    let backup = journal::material(&journal::directory(&fixture.state, &preview.id).unwrap(), 0, &path, false);
    fs::remove_file(&backup).unwrap();
    let applied = fs::read(&path).unwrap();
    let error = transaction::restore(&fixture.state, &fixture.scope, &record.plan.view.id, &|| Ok(())).unwrap_err();
    assert!(error.to_string().contains(&backup.display().to_string()));
    assert_eq!(fs::read(path).unwrap(), applied);
    assert_eq!(journal::latest(&fixture.state).unwrap().unwrap().phase, Phase::Applied);
}
