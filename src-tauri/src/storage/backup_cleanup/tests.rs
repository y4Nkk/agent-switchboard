use super::*;
use asb_core::BackupRecord;
use asb_switch::{config_journal_path, PendingConfigWrite};
use std::io::Read;

fn write_backup(
    local: &LocalState, app: AppKind, name: &str, created: &str, parent: Option<&str>,
) -> PathBuf {
    let root = local.backup_dir();
    fs::create_dir_all(&root).unwrap();
    let path = root.join(name);
    let content = format!("content of {name}");
    fs::write(&path, &content).unwrap();
    let target = local.target(app).unwrap();
    let record = BackupRecord {
        id: format!("id-{name}"), app,
        target_path: if parent.is_some() { target.with_file_name("auth.json") } else { target }
            .to_string_lossy().into_owned(),
        backup_path: path.to_string_lossy().into_owned(), created_at: created.into(),
        content_hash: asb_switch::sha256_hex(&content), target_existed: true,
        linked_backup_id: parent.map(str::to_string),
        reason: if parent.is_some() { "codex-auth-projection" } else { "provider-projection" }.into(),
    };
    fs::write(format!("{}.meta.json", path.display()), serde_json::to_vec(&record).unwrap()).unwrap();
    path
}

fn snapshot(local: &LocalState, backup: &Path) -> PathBuf {
    let record: BackupRecord = serde_json::from_slice(
        &fs::read(format!("{}.meta.json", backup.display())).unwrap(),
    ).unwrap();
    let path = local.client_settings_backup_path(&record).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "{}").unwrap();
    age(&path);
    path
}

fn age(path: &Path) {
    let old = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_577_836_800);
    fs::OpenOptions::new().write(true).open(path).unwrap()
        .set_times(fs::FileTimes::new().set_modified(old)).unwrap();
}

fn tar_contents(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let input = flate2::read::GzDecoder::new(fs::File::open(path).unwrap());
    let mut archive = tar::Archive::new(input);
    archive.entries().unwrap().map(|entry| {
        let mut entry = entry.unwrap();
        let name = entry.path().unwrap().to_string_lossy().replace('\\', "/");
        let mut data = Vec::new();
        entry.read_to_end(&mut data).unwrap();
        (name, data)
    }).collect()
}

#[test]
fn exports_complete_families_and_retains_every_member_of_the_newest_stale_group() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let temp = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temp.path().join("state"));
    let old = write_backup(&local, AppKind::Codex, "config.toml.old.bak", "2020-01-01T00:00:00Z", None);
    let auth = write_backup(&local, AppKind::Codex, "auth.json.old.bak", "2020-01-01T00:00:00Z", Some("id-config.toml.old.bak"));
    let settings = snapshot(&local, &old);
    assert_eq!(settings, local.backup_dir().join("client-settings/config.toml.old.json"));
    let newest = write_backup(&local, AppKind::Codex, "config.toml.latest.bak", "2020-12-01T00:00:00Z", None);
    let newest_auth = write_backup(&local, AppKind::Codex, "auth.json.latest.bak", "2020-12-01T00:00:00Z", Some("id-config.toml.latest.bak"));
    let newest_settings = snapshot(&local, &newest);
    let claude = write_backup(&local, AppKind::Claude, "settings.json.old.bak", "2020-01-01T00:00:00Z", None);
    let claude_latest = write_backup(&local, AppKind::Claude, "settings.json.latest.bak", "2020-12-01T00:00:00Z", None);

    let plan = preview(&local, "2021-01-01").unwrap();
    assert_eq!(plan.digest.records, 2);
    let destination = temp.path().join("backups.tar.gz");
    let result = execute(&local, &plan, &destination).unwrap();
    assert_eq!(result.removed, 2);
    assert!(result.warnings.is_empty());
    for removed in [&old, &auth, &settings, &claude] { assert!(!removed.exists()); }
    for retained in [&newest, &newest_auth, &newest_settings, &claude_latest] { assert!(retained.exists()); }
    let archive = tar_contents(&destination);
    let manifest: serde_json::Value = serde_json::from_slice(&archive["manifest.json"]).unwrap();
    assert_eq!(manifest["plan"], serde_json::to_value(plan).unwrap());
    for name in ["config.toml.old.bak", "auth.json.old.bak", "client-settings/config.toml.old.json"] {
        assert!(archive.contains_key(&format!("agent-switchboard-backups/{name}")));
    }
    assert!(!archive.keys().any(|name| name.contains("latest")));
}

#[test]
fn refuses_pending_configuration_transactions() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let temp = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temp.path().join("state"));
    let bak = write_backup(&local, AppKind::Codex, "config.toml.old.bak", "2020-01-01T00:00:00Z", None);
    let backup: BackupRecord = serde_json::from_slice(&fs::read(format!("{}.meta.json", bak.display())).unwrap()).unwrap();
    let pending = PendingConfigWrite {
        version: 1, app: AppKind::Codex, profile_id: None, backup,
        after_hash: "b".repeat(64), after_existed: true, auth: None,
    };
    fs::write(config_journal_path(&local.backup_dir(), AppKind::Codex), serde_json::to_vec(&pending).unwrap()).unwrap();
    assert!(preview(&local, "2021-01-01").is_err());
    assert!(bak.exists());
}

#[test]
fn same_length_content_changes_invalidate_the_preview_without_deleting_materials() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let temp = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temp.path().join("state"));
    let bak = write_backup(&local, AppKind::Codex, "config.toml.old.bak", "2020-01-01T00:00:00Z", None);
    write_backup(&local, AppKind::Codex, "config.toml.latest.bak", "2020-12-01T00:00:00Z", None);
    fs::write(&bak, "aaaa").unwrap();
    let plan = preview(&local, "2021-01-01").unwrap();
    fs::write(&bak, "bbbb").unwrap();
    let destination = temp.path().join("backups.tar.gz");
    assert!(execute(&local, &plan, &destination).is_err());
    assert_eq!(fs::read(&bak).unwrap(), b"bbbb");
    assert!(!destination.exists());
}

#[test]
fn damaged_archive_readback_fails_while_originals_remain() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let temp = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temp.path().join("state"));
    let bak = write_backup(&local, AppKind::Codex, "config.toml.old.bak", "2020-01-01T00:00:00Z", None);
    write_backup(&local, AppKind::Codex, "config.toml.latest.bak", "2020-12-01T00:00:00Z", None);
    let selected = select(&local, cutoff_time("2021-01-01").unwrap()).unwrap();
    let destination = temp.path().join("backups.tar.gz");
    archive::write(&destination, &selected.entries, &selected.plan).unwrap();
    archive::verify(&destination, &selected.entries, &selected.plan).unwrap();
    let data = fs::read(&destination).unwrap();
    fs::write(&destination, &data[..data.len() / 2]).unwrap();
    assert!(archive::verify(&destination, &selected.entries, &selected.plan).is_err());
    assert!(bak.exists());
}

#[test]
fn directory_artifacts_archive_nested_and_empty_directories_as_standard_tar_entries() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let temp = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temp.path().join("state"));
    let directory = local.backup_dir().join("extension.bak");
    fs::create_dir_all(directory.join("empty")).unwrap();
    fs::write(directory.join("data.txt"), b"preserved").unwrap();
    let families = vec![Family {
        paths: vec![directory], stamp: cutoff_time("2020-01-01").unwrap(), app: None, config: false,
    }];
    let entries = archive::inventory(&local.backup_dir(), &families).unwrap();
    let plan = BackupCleanupPlan {
        cutoff: "2021-01-01".into(), files: 1, bytes: 9,
        digest: ArchiveDigest { records: 1, revision: "fixture".into() },
    };
    let destination = temp.path().join("backups.tar.gz");
    archive::write(&destination, &entries, &plan).unwrap();
    archive::verify(&destination, &entries, &plan).unwrap();
    let archive = tar_contents(&destination);
    assert!(archive.keys().any(|name| name.trim_end_matches('/') == "agent-switchboard-backups/extension.bak/empty"));
    assert_eq!(archive["agent-switchboard-backups/extension.bak/data.txt"], b"preserved");
}

#[test]
fn unrecognized_backup_pairs_stay_on_disk_and_invalid_cutoffs_are_rejected() {
    let _paths = crate::test_client_paths::redirect_client_paths();
    let temp = tempfile::tempdir().unwrap();
    let local = LocalState::from_root(temp.path().join("state"));
    fs::create_dir_all(local.backup_dir()).unwrap();
    let bak = local.backup_dir().join("unrecognized.bak");
    let meta = local.backup_dir().join("unrecognized.bak.meta.json");
    fs::write(&bak, b"keep").unwrap();
    fs::write(&meta, b"invalid metadata").unwrap();
    age(&bak);
    age(&meta);
    assert_eq!(preview(&local, "2021-01-01").unwrap().digest.records, 0);
    assert!(bak.exists() && meta.exists());
    assert!(preview(&local, "2099-01-01").is_err());
    assert!(preview(&local, "not-a-date").is_err());
}
