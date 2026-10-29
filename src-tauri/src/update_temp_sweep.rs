//! Removes updater payloads left in the OS temp directory. The updater
//! plugin launches the installer and then exits the process, so its own
//! temp cleanup never runs; this startup sweep owns the residue instead.
use crate::runtime_log::RuntimeLogAction;
use std::path::Path;
use std::time::{Duration, SystemTime};

/// Update installs are minutes-long events; a week of residue retention
/// keeps any just-downloaded payload safe while staying effectively bounded.
const MIN_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub(crate) fn spawn(app: tauri::AppHandle) {
    let spawned = std::thread::Builder::new().name("update-temp-sweep".into()).spawn(move || {
        // The updater plugin builds its temp prefix from the same package
        // name, so only this application's payloads ever match.
        let prefix = format!("{}-", app.package_info().name);
        if sweep_directory(&std::env::temp_dir(), &prefix, SystemTime::now()) > 0 {
            crate::runtime_log::record_success(RuntimeLogAction::UpdateTempSwept);
        }
    });
    if let Err(error) = spawned { log::warn!("无法启动更新包残留清扫线程：{error}"); }
}

/// Deletes `{prefix}…-updater-…` directories older than the retention
/// window. Single-entry failures are ignored on purpose: an installer that
/// is mid-run is locked by Windows and vanishes once the update finishes.
pub(crate) fn sweep_directory(dir: &Path, prefix: &str, now: SystemTime) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    let mut removed = 0_u64;
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else { continue };
        if !metadata.is_dir() { continue; }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.starts_with(prefix) && name.contains("-updater-")) { continue; }
        let stale = metadata.modified().ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= MIN_AGE);
        if !stale { continue; }
        if std::fs::remove_dir_all(entry.path()).is_ok() { removed += 1; }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweeps_only_stale_directories_matching_the_updater_prefix() {
        let temp = tempfile::tempdir().unwrap();
        let stale = temp.path().join("Agent Switchboard-0.3.11-updater-abc");
        let fresh = temp.path().join("Agent Switchboard-0.3.12-updater-def");
        let other = temp.path().join("other-app-0.1-updater-xyz");
        let file = temp.path().join("Agent Switchboard-0.3.10-updater-file");
        for path in [&stale, &fresh, &other] { std::fs::create_dir_all(path).unwrap(); }
        std::fs::write(&file, b"payload").unwrap();

        // Just-created payloads are inside the retention window and stay.
        assert_eq!(sweep_directory(temp.path(), "Agent Switchboard-", SystemTime::now()), 0);
        assert!(stale.exists());
        assert!(fresh.exists());

        // A clock past the retention window reclaims only matching directories.
        let future = SystemTime::now() + MIN_AGE + Duration::from_secs(1);
        assert_eq!(sweep_directory(temp.path(), "Agent Switchboard-", future), 2);
        assert!(!stale.exists());
        assert!(!fresh.exists());
        assert!(other.exists());
        assert!(file.exists());
    }

    #[test]
    fn a_missing_temp_directory_sweeps_nothing() {
        assert_eq!(sweep_directory(Path::new("Z:/definitely-missing-asb"), "x-", SystemTime::now()), 0);
    }
}
