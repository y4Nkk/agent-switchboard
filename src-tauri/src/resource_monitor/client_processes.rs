//! Read-only process guards. An update replaces the installation's
//! files, so only a candidate-named process verifiably running one of those
//! images blocks it. An unavailable image remains unknown and does not
//! establish occupancy.
// Note: 占用契约见 .agents/notes/implemented/bug-fix/2026-11-09-update-occupancy-guard.md
use super::{platform, types::Issue};
#[cfg(not(windows))]
use super::platform::Reader;
use crate::client_software::{discovery, identity, SoftwareError, types::Installation};
use std::path::PathBuf;

pub fn ensure_idle(installation: &Installation) -> Result<(), SoftwareError> {
    check_images(installation, platform::process_images, platform::image_path)
}

fn check_images(
    installation: &Installation,
    images: impl FnOnce() -> Result<Vec<(u32, String)>, String>,
    mut image_path: impl FnMut(u32) -> Option<PathBuf>,
) -> Result<(), SoftwareError> {
    let target = identity::canonical_root(&installation.executable);
    let roots: Vec<_> = discovery::process_roots(installation)?.iter()
        .map(|root| format!("{}/", identity::canonical_root(root))).collect();
    let launcher = installation.executable.file_name().map(|name| name.to_string_lossy().into_owned());
    for (pid, image) in images()? {
        if !is_candidate(&image, launcher.as_deref()) { continue; }
        let Some(path) = image_path(pid) else { continue };
        let normalized = identity::canonical_root(&path);
        if occupies(&normalized, &target, &roots) && !identity::installations::desktop_shell(&normalized) {
            return Err(SoftwareError::ProcessInUse { app: installation.app, pid });
        }
    }
    Ok(())
}

/// Deliberately wider than the monitor's engine identification: platform
/// payload images (codex-x86_64-pc-windows-msvc) carry neither the exact
/// engine name nor a version shape, so the client name prefixes and the
/// installed product's own file name carry them into the check. Claude's
/// native version filenames are covered by the shared numeric-name rule.
fn is_candidate(name: &str, launcher: Option<&str>) -> bool {
    let name = name.to_ascii_lowercase();
    identity::candidate(&name) || name.contains("codex") || name.contains("claude")
        || launcher.is_some_and(|launcher| launcher.eq_ignore_ascii_case(&name))
}

/// Occupancy is a file fact, not a user fact: any process running the
/// installation's image would hold the replaced files open.
fn occupies(normalized: &str, target: &str, roots: &[String]) -> bool {
    normalized == target || roots.iter().any(|root| normalized.starts_with(root))
}

#[derive(Debug, PartialEq, Eq)]
pub enum HistoryProcessCheck { Idle, Running(u32), Uncertain }

pub fn codex_history_occupant() -> Result<HistoryProcessCheck, String> {
    let (entries, issues) = platform::Reader::new()?.entries()?;
    if !issues.is_empty() { return Ok(HistoryProcessCheck::Uncertain); }
    Ok(history_occupant(&entries))
}

fn history_occupant(entries: &[super::observation::Entry]) -> HistoryProcessCheck {
    use super::observation::ProcessUser;
    for entry in entries {
        let name = entry.name.to_ascii_lowercase();
        let name = name.strip_suffix(".exe").unwrap_or(&name);
        // Current-user workers must exit too; unverifiable SYSTEM helpers
        // do not establish that this user's Codex history is still open.
        if (entry.user == ProcessUser::Current && (name == "codex" || name.starts_with("codex-")))
            || (entry.user == ProcessUser::Unverified && (name == "codex"
                || (name.starts_with("codex-") && name != "codex-windows-sandbox-service"))) {
            return HistoryProcessCheck::Running(entry.pid);
        }
    }
    HistoryProcessCheck::Idle
}

#[cfg(windows)]
pub fn birth(pid: u32) -> Result<Option<String>, String> {
    match platform::process_key(pid) {
        Ok(key) => Ok(Some(key.birth.to_string())),
        Err(Issue::ProcessExited) => Ok(None),
        Err(issue) => Err(format!("Process identity unavailable: {issue:?}")),
    }
}

#[cfg(not(windows))]
pub fn birth(pid: u32) -> Result<Option<String>, String> {
    let reader = Reader::new()?;
    let (entries, _) = reader.entries()?;
    let Some(entry) = entries.iter().find(|entry| entry.pid == pid) else { return Ok(None); };
    match reader.read(entry) {
        Ok(Some(process)) => Ok(Some(process.key.birth.to_string())),
        Ok(None) | Err(Issue::ProcessExited) => Ok(None),
        Err(issue) => Err(format!("Process identity unavailable: {issue:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{check_images, ensure_idle, is_candidate, occupies, SoftwareError};
    use crate::client_software::types::{Form, Installation, Manager};
    use asb_core::contracts::AppKind;
    use std::{fs, path::{Path, PathBuf}, process::{Child, Command, Stdio}, thread, time::{Duration, Instant}};

    const FIXTURE_TEST: &str = "resource_monitor::client_processes::tests::occupancy_fixture_process";

    #[test]
    fn history_checks_block_user_engines_without_claiming_the_system_sandbox_service() {
        use super::{history_occupant, HistoryProcessCheck};
        use crate::resource_monitor::observation::{Entry, ProcessUser};
        let entry = |pid, name: &str, user| Entry { pid, parent_pid: 0, name: name.into(), user };
        let mut entries = vec![entry(1, "codex-windows-sandbox-service.exe", ProcessUser::Unverified),
            entry(2, "codex.exe", ProcessUser::Other)];
        assert_eq!(history_occupant(&entries), HistoryProcessCheck::Idle);
        entries.push(entry(3, "codex-x86_64-pc-windows-msvc.exe", ProcessUser::Current));
        assert_eq!(history_occupant(&entries), HistoryProcessCheck::Running(3));
        assert_eq!(history_occupant(&[entry(4, "Codex.exe", ProcessUser::Unverified)]), HistoryProcessCheck::Running(4));
        assert_eq!(history_occupant(&[entry(5, "codex-x86_64-pc-windows-msvc.exe", ProcessUser::Unverified)]), HistoryProcessCheck::Running(5));
    }

    #[test]
    fn candidate_net_covers_engine_payloads_prefixes_and_launcher_names() {
        assert!(is_candidate("codex.exe", None));
        assert!(is_candidate("claude.exe", None));
        assert!(is_candidate("claude-code.exe", None));
        assert!(is_candidate("codex-x86_64-pc-windows-msvc.exe", None));
        assert!(is_candidate("codex-windows-sandbox-service.exe", None));
        assert!(is_candidate("custom-engine", Some("CUSTOM-ENGINE")));
        assert!(!is_candidate("custom-engine", None));
        assert!(is_candidate("0.162.1.exe", None));
        assert!(is_candidate("2.1.310", None));
        assert!(is_candidate("2.1.310.exe", None));
        assert!(!is_candidate("node.exe", None));
        assert!(!is_candidate("explorer.exe", None));
    }

    #[test]
    fn occupancy_requires_the_exact_image_or_an_installation_root_prefix() {
        let root = "c:/users/u/appdata/roaming/npm/node_modules/@openai/codex-win32-x64/";
        assert!(occupies(&format!("{root}codex.exe"), "t", &[root.into()]));
        assert!(occupies("c:/npm/codex.exe", "c:/npm/codex.exe", &[]));
        // A truncated prefix must not claim sibling directories.
        assert!(!occupies("c:/npm/codex-extra/x.exe", "t", &["c:/npm/codex/".into()]));
        assert!(!occupies("c:/windows/system32/codex.exe", "t", &[root.into()]));
    }

    #[test]
    fn unreadable_candidate_images_do_not_block_or_probe_unrelated_processes() {
        for app in [AppKind::Codex, AppKind::Claude] {
            let directory = tempfile::tempdir().unwrap();
            let root = fs::canonicalize(directory.path()).unwrap();
            let item = installation(app, root.join("client/engine"));
            let images = vec![(1, "codex-windows-sandbox-service.exe".into()),
                (2, "claude.exe".into()), (3, "explorer.exe".into())];
            let mut probed = Vec::new();
            assert_eq!(check_images(&item, || Ok(images), |pid| { probed.push(pid); None }), Ok(()));
            assert_eq!(probed, vec![1, 2]);
        }
    }

    #[test]
    fn skipping_an_unreadable_image_still_blocks_a_proven_occupant() {
        for (app, engine, service) in [(AppKind::Codex, "codex", "codex-windows-sandbox-service.exe"),
            (AppKind::Claude, "2.1.310.exe", "claude-background-service.exe")] {
            let directory = tempfile::tempdir().unwrap();
            let root = fs::canonicalize(directory.path()).unwrap();
            let item = installation(app, root.join("client").join(engine));
            let images = vec![(1, service.into()), (2, engine.into())];
            let result = check_images(&item, || Ok(images), |pid|
                (pid == 2).then(|| item.executable.clone()));
            assert_eq!(result, Err(SoftwareError::ProcessInUse { app, pid: 2 }));
        }
    }

    #[test]
    fn a_proven_foreign_image_does_not_block() {
        for (app, engine) in [(AppKind::Codex, "codex"), (AppKind::Claude, "claude")] {
            let directory = tempfile::tempdir().unwrap();
            let root = fs::canonicalize(directory.path()).unwrap();
            let item = installation(app, root.join("client").join(engine));
            assert_eq!(check_images(&item, || Ok(vec![(1, engine.into())]),
                |_| Some(root.join("foreign").join(engine))), Ok(()));
        }
    }

    #[test]
    fn claude_native_versions_block_even_when_the_stable_entry_is_outside_the_versions_root() {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        let mut item = installation(AppKind::Claude, root.join("bin/claude.exe"));
        item.manager = Manager::Native;
        item.root = root.join("versions");
        fs::create_dir_all(&item.root).unwrap();
        for engine in ["2.1.309", "2.1.309.exe"] {
            let image = item.root.join(engine);
            fs::write(&image, b"MZ\0\0").unwrap();
            assert_eq!(check_images(&item, || Ok(vec![(42, engine.into())]), |_| Some(image.clone())),
                Err(SoftwareError::ProcessInUse { app: AppKind::Claude, pid: 42 }));
        }
    }

    #[test]
    fn declared_npm_platform_images_block_without_claiming_other_packages() {
        for (app, package, payload, engine) in [
            (AppKind::Codex, "@openai/codex", "@openai/codex-win32-x64", "codex.exe"),
            (AppKind::Claude, "@anthropic-ai/claude-code", "@anthropic-ai/claude-code-win32-x64", "claude.exe"),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let modules = fs::canonicalize(directory.path()).unwrap().join("node_modules");
            let root = modules.join(package);
            fs::create_dir_all(root.join("bin")).unwrap();
            let mut item = installation(app, root.join("bin").join(engine));
            item.manager = Manager::Npm;
            item.root = root.clone();
            let manifest = serde_json::json!({ "name": package, "optionalDependencies": { (payload): "fixture" } });
            fs::write(root.join("package.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
            for payload in [root.join("node_modules").join(payload), modules.join(payload)] {
                fs::create_dir_all(&payload).unwrap();
                let image = payload.join(engine);
                fs::write(&image, b"MZ\0\0").unwrap();
                assert_eq!(check_images(&item, || Ok(vec![(42, engine.into())]), |_| Some(image.clone())),
                    Err(SoftwareError::ProcessInUse { app, pid: 42 }));
            }
            assert_eq!(check_images(&item, || Ok(vec![(42, engine.into())]),
                |_| Some(modules.join("unrelated").join(engine))), Ok(()));
        }
    }

    #[cfg(unix)]
    #[test]
    fn image_aliases_use_the_same_canonical_path_as_the_installation() {
        for (app, engine) in [(AppKind::Codex, "codex"), (AppKind::Claude, "2.1.310")] {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().join("installation");
            let alias = directory.path().join("image-alias");
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join(engine), b"fixture").unwrap();
            std::os::unix::fs::symlink(&root, &alias).unwrap();
            let item = installation(app, root.join(engine));
            assert_eq!(check_images(&item, || Ok(vec![(42, engine.into())]), |_| Some(alias.join(engine))),
                Err(SoftwareError::ProcessInUse { app, pid: 42 }));
        }
    }

    #[test]
    fn enumeration_failure_is_reported_without_probing_individual_images() {
        let directory = tempfile::tempdir().unwrap();
        let item = installation(AppKind::Codex, directory.path().join("codex"));
        assert_eq!(check_images(&item, || Err("fixture enumeration failure".into()),
            |_| panic!("no image can be queried after enumeration failed")),
            Err(SoftwareError::Operation("fixture enumeration failure".into())));
    }

    fn installation(app: AppKind, executable: PathBuf) -> Installation {
        Installation { id: "fixture".into(), app, form: Form::Cli, manager: Manager::Manual,
            launcher: executable.clone(), root: executable.parent().unwrap().into(),
            executable, version: None, version_text: None, active: true, channel: "latest".into(),
            detail: None, owner: None }
    }

    #[test]
    fn a_running_installation_image_blocks_and_a_foreign_candidate_does_not() {
        for (app, engine) in [(AppKind::Codex, "codex-occupancy-fixture"),
            (AppKind::Claude, "claude-occupancy-fixture"), (AppKind::Claude, "2.1.310")] {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path();
            let fixture = fixture_executable(&std::env::current_exe().unwrap(), &root.join(engine));
            let mut children = Children { stop: root.join("stop"), processes: Vec::new() };
            children.processes.push(spawn_fixture(&fixture, root));
            let pid = wait_pid(root);
            let deadline = Instant::now() + Duration::from_secs(30);
            let blocked = loop {
                match ensure_idle(&installation(app, fixture.clone())) {
                    Err(error) => break error,
                    Ok(()) => {
                        assert!(Instant::now() < deadline, "running fixture image never blocked the update");
                        thread::sleep(Duration::from_millis(20));
                    }
                }
            };
            assert_eq!(blocked, SoftwareError::ProcessInUse { app, pid });
            let foreign = root.join("unrelated/other-client");
            fs::create_dir_all(foreign.parent().unwrap()).unwrap();
            assert_eq!(ensure_idle(&installation(app, foreign)), Ok(()));
        }
    }

    fn fixture_executable(source: &Path, destination: &Path) -> PathBuf {
        let mut filename = destination.as_os_str().to_os_string();
        if cfg!(windows) { filename.push(".exe"); }
        let destination = PathBuf::from(filename);
        fs::copy(source, &destination).unwrap();
        destination
    }

    fn spawn_fixture(executable: &Path, directory: &Path) -> Child {
        let mut command = Command::new(executable);
        command.args(["--ignored", "--exact", FIXTURE_TEST, "--test-threads=1"])
            .env("ASB_OCCUPANCY_FIXTURE_DIRECTORY", directory).stdin(Stdio::null())
            .stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)] {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        command.spawn().unwrap()
    }

    fn wait_pid(directory: &Path) -> u32 {
        let path = directory.join("occupancy-fixture.pid");
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Ok(value) = fs::read_to_string(&path) {
                if let Ok(pid) = value.parse() { return pid; }
            }
            assert!(Instant::now() < deadline, "fixture did not become ready: {}", path.display());
            thread::sleep(Duration::from_millis(10));
        }
    }

    struct Children { stop: PathBuf, processes: Vec<Child> }

    impl Drop for Children {
        fn drop(&mut self) {
            let _ = fs::write(&self.stop, b"stop");
            for child in &mut self.processes {
                let deadline = Instant::now() + Duration::from_secs(5);
                while child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(10));
                }
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    #[test]
    #[ignore = "native process fixture; launched only by the occupancy test"]
    fn occupancy_fixture_process() {
        let root = PathBuf::from(std::env::var_os("ASB_OCCUPANCY_FIXTURE_DIRECTORY").expect("isolated fixture directory"));
        fs::write(root.join("occupancy-fixture.pid"), std::process::id().to_string()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        while !root.join("stop").exists() && Instant::now() < deadline { thread::sleep(Duration::from_millis(10)); }
    }
}
