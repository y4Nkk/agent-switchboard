use super::*;
use asb_core::BackupRecord;
use std::fs;

const BEFORE: &str = "model = 'before'\n";
const PLANNED: &str = "model = 'planned'\n";
const CHANGED: &str = "model = 'changed-outside-switchboard'\n";
const AUTH_BEFORE: &str = r#"{"fixture":"auth-before"}"#;
const AUTH_CURRENT: &str = r#"{"fixture":"auth-current"}"#;
const BACKUP_ID: &str = "transaction-fixture";

struct Fixture {
    state: LocalState,
    gateway: GatewayController,
    config: PathBuf,
    backup: PathBuf,
    pending: PathBuf,
    _directory: tempfile::TempDir,
    _client_paths: crate::test_client_paths::ClientPathGuard,
}

impl Fixture {
    fn new() -> Self {
        let client_paths = crate::test_client_paths::redirect_client_paths();
        let directory = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(directory.path().join("state"));
        let gateway = GatewayController::start(&state);
        let config = state.target(AppKind::Codex).unwrap();
        fs::write(&config, BEFORE).unwrap();
        let after_hash = asb_switch::sha256_hex(PLANNED);
        begin(&state, &gateway, AppKind::Codex, None, &after_hash, true, None).unwrap();

        fs::create_dir_all(state.backup_dir()).unwrap();
        let backup = state.backup_dir().join("transaction-fixture.toml");
        fs::write(&backup, BEFORE).unwrap();
        let record = BackupRecord {
            id: BACKUP_ID.into(),
            app: AppKind::Codex,
            target_path: config.to_string_lossy().into_owned(),
            backup_path: backup.to_string_lossy().into_owned(),
            created_at: "2026-11-23T00:00:00Z".into(),
            content_hash: asb_switch::sha256_hex(BEFORE),
            target_existed: true,
            linked_backup_id: None,
            reason: "provider-projection".into(),
        };
        fs::write(backup.with_extension("toml.meta.json"), serde_json::to_vec(&record).unwrap()).unwrap();
        let pending = asb_switch::config_journal_path(&state.backup_dir(), AppKind::Codex);
        fs::write(&pending, serde_json::to_vec(&PendingConfigWrite {
            version: 1, app: AppKind::Codex, profile_id: None, backup: record, after_hash,
            after_existed: true, auth: None,
        }).unwrap()).unwrap();
        fs::write(&config, CHANGED).unwrap();

        Self { state, gateway, config, backup, pending, _directory: directory, _client_paths: client_paths }
    }

    fn restore(&self, backup_id: &str) -> Result<Option<RestoreOutcome>, CommandError> {
        restore_pending_backup(&self.state, &self.gateway, backup_id, true)
    }

    fn add_paired_auth(&self) -> (PathBuf, PathBuf) {
        let target = self.config.with_file_name("auth.json");
        let backup = self.state.backup_dir().join("fixture-auth.json.bak");
        fs::write(&target, AUTH_CURRENT).unwrap();
        fs::write(&backup, AUTH_BEFORE).unwrap();
        let hash = asb_switch::sha256_hex(AUTH_BEFORE);
        let after_hash = asb_switch::sha256_hex(AUTH_CURRENT);
        let mut intent = load(&self.state).unwrap().unwrap();
        intent.auth = Some(AuthIntent {
            before_hash: hash.clone(), before_existed: true, after_hash: after_hash.clone(), after_existed: true,
        });
        journal::save(&self.state, &intent).unwrap();
        let mut pending: PendingConfigWrite = serde_json::from_slice(&fs::read(&self.pending).unwrap()).unwrap();
        pending.auth = Some(asb_switch::PendingAuthWrite {
            backup: BackupRecord {
                id: "auth-fixture".into(),
                target_path: target.to_string_lossy().into_owned(),
                backup_path: backup.to_string_lossy().into_owned(),
                content_hash: hash,
                linked_backup_id: Some(BACKUP_ID.into()),
                reason: "codex-auth-projection".into(),
                ..pending.backup.clone()
            },
            after_hash, after_existed: true,
        });
        fs::write(&self.pending, serde_json::to_vec(&pending).unwrap()).unwrap();
        (target, backup)
    }

    fn assert_preserved(&self) {
        assert_eq!(fs::read_to_string(&self.config).unwrap(), CHANGED);
        assert!(self.pending.is_file());
        assert!(load(&self.state).unwrap().is_some());
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.gateway.shutdown();
    }
}

#[test]
fn auto_recovery_conflict_can_be_resolved_with_the_matching_backup() {
    let f = Fixture::new();
    assert!(recover(&f.state, &f.gateway).unwrap_err().contains(BACKUP_ID));
    f.assert_preserved();
    assert_eq!(recovery_backup_id(&f.state).as_deref(), Some(BACKUP_ID));
    let visible = super::super::super::backups::local_backups(&f.state).unwrap();
    assert!(visible.iter().any(|backup| backup.id == BACKUP_ID));

    let unexpected = f.restore("unrelated-history-backup").unwrap_err();
    assert_eq!(unexpected.code, "transaction-backup-mismatch");
    f.assert_preserved();
    let result = f.restore(BACKUP_ID).unwrap().unwrap();
    assert_eq!(fs::read_to_string(&f.config).unwrap(), BEFORE);
    assert_eq!(fs::read_to_string(result.pre_restore_backup.backup_path).unwrap(), CHANGED);
    assert_eq!(result.restored_hash, asb_switch::sha256_hex(BEFORE));
    assert!(!f.pending.exists());
    assert!(load(&f.state).unwrap().is_none());
    assert_eq!(recovery_backup_id(&f.state), None);
    recover(&f.state, &f.gateway).unwrap();

    // A stale confirmation must not restore the same file as a historical backup.
    assert_eq!(f.restore(BACKUP_ID).unwrap_err().code, "transaction-backup-mismatch");
    assert!(restore_pending_backup(&f.state, &f.gateway, BACKUP_ID, false).unwrap().is_none());
}

#[test]
fn corrupt_transaction_backup_reports_failure_and_allows_retry() {
    let f = Fixture::new();
    fs::write(&f.backup, "tampered fixture").unwrap();
    let failure = f.restore(BACKUP_ID).unwrap_err();
    assert!(failure.message.contains("备份内容与记录哈希不符"), "{failure:?}");
    f.assert_preserved();

    fs::write(&f.backup, BEFORE).unwrap();
    assert!(f.restore(BACKUP_ID).unwrap().is_some());
    assert_eq!(fs::read_to_string(&f.config).unwrap(), BEFORE);
}

#[test]
fn corrupt_paired_auth_backup_is_reported_before_either_file_changes() {
    let f = Fixture::new();
    let (auth, backup) = f.add_paired_auth();
    fs::write(&backup, "tampered auth fixture").unwrap();
    let failure = f.restore(BACKUP_ID).unwrap_err();
    assert!(failure.message.contains("认证备份校验失败"), "{failure:?}");
    f.assert_preserved();
    assert_eq!(fs::read_to_string(&auth).unwrap(), AUTH_CURRENT);

    fs::write(backup, AUTH_BEFORE).unwrap();
    assert!(f.restore(BACKUP_ID).unwrap().is_some());
    assert_eq!(fs::read_to_string(&f.config).unwrap(), BEFORE);
    assert_eq!(fs::read_to_string(&auth).unwrap(), AUTH_BEFORE);
}

#[test]
fn mismatched_journal_is_not_offered_or_recovered() {
    let f = Fixture::new();
    let mut intent = load(&f.state).unwrap().unwrap();
    intent.version = 1;
    journal::save(&f.state, &intent).unwrap();
    assert_eq!(recovery_backup_id(&f.state), None);
    assert_eq!(f.restore(BACKUP_ID).unwrap_err().code, "config-recovery-required");
    f.assert_preserved();
}
