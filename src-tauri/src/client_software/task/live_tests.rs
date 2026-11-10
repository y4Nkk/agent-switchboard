use super::*;
use crate::client_software::{discovery, npm, releases, types::{Comparison, Manager, Owner, Release, Snapshot, Software}};
use asb_core::contracts::AppKind;
use std::{fs, path::{Path, PathBuf}, time::Instant};

#[test]
#[ignore = "Installs official packages; requires an explicitly isolated client-update environment"]
fn npm_codex_upgrade() {
    upgrade(AppKind::Codex, "@openai/codex", "0.162.1", "0.167.0");
}

#[test]
#[ignore = "Installs official packages; requires an explicitly isolated client-update environment"]
fn npm_claude_upgrade() {
    upgrade(AppKind::Claude, "@anthropic-ai/claude-code", "2.1.275", "latest");
}

fn isolated_root() -> PathBuf {
    let root = fs::canonicalize(std::env::var_os("ASB_CLIENT_UPDATE_LIVE_ROOT")
        .expect("Set ASB_CLIENT_UPDATE_LIVE_ROOT to the isolated verification directory")).unwrap();
    for key in ["USERPROFILE", "HOME", "APPDATA", "LOCALAPPDATA", "CODEX_HOME", "CLAUDE_CONFIG_DIR",
        "npm_config_cache", "npm_config_userconfig", "npm_config_globalconfig"] {
        let path = fs::canonicalize(std::env::var_os(key).unwrap_or_else(|| panic!("Missing {key}"))).unwrap();
        assert!(path.starts_with(&root), "{key} must stay under {}", root.display());
    }
    root
}

fn seed(root: &Path, app: AppKind, package: &str, version: &str) -> PathBuf {
    let prefix = root.join(format!("{} npm prefix", discovery::name(app)));
    fs::create_dir_all(&prefix).unwrap();
    let owner = Owner::Npm {
        node: fs::canonicalize(std::env::var_os("ASB_CLIENT_UPDATE_NODE").unwrap()).unwrap(),
        npm: fs::canonicalize(std::env::var_os("ASB_CLIENT_UPDATE_NPM").unwrap()).unwrap(),
        prefix: fs::canonicalize(&prefix).unwrap(), package: package.into(),
        registry: Some("https://registry.npmjs.org/".into()),
    };
    let mut command = npm::command(&owner, &Default::default()).unwrap();
    command.args(["install", "--engine-strict", "--no-audit", "--no-fund"]).arg(format!("{package}@{version}"));
    let output = process::output(&mut command, Duration::from_secs(240)).expect("Isolated seed installation failed");
    fs::write(root.join(format!("{}-seed.log", discovery::name(app))), output).unwrap();
    prefix.join(format!("{}.cmd", discovery::name(app)))
}

fn upgrade(app: AppKind, package: &str, from: &str, candidate: &str) {
    let root = isolated_root();
    let launcher = seed(&root, app, package, from);
    let mut installation = discovery::inspect(app, &launcher).unwrap();
    discovery::check_source(&mut installation).unwrap();
    assert_eq!(installation.manager, Manager::Npm);
    assert_eq!(installation.version.as_deref(), Some(from));
    let owner = installation.owner.as_ref().expect("Production discovery must identify the npm owner");
    let metadata = npm::metadata(owner, candidate, &Default::default()).unwrap();
    let target = metadata["version"].as_str().unwrap().to_string();
    assert_eq!(metadata["name"].as_str(), Some(package));
    assert_eq!(releases::compare(Some(from), Some(&target)), Comparison::UpdateAvailable);
    let state = LocalState::from_root(root.join(format!("{}-state", discovery::name(app))));
    let policy = crate::commands::client_settings::software_policy::read(&state, app);
    assert!(policy.issue.is_none(), "Unexpected policy: {:?}", policy.issue);
    let software = Software { installation: installation.clone(), official: None,
        available: Some(Release { version: target.clone(), source: npm::source(owner),
            checked_at: chrono::Utc::now().to_rfc3339(), stale: false }),
        comparison: Comparison::UpdateAvailable, policy, update_allowed: true, reason: None,
        guide_url: crate::client_software::types::guide(app).into(), release_error: None };
    let revision = uuid::Uuid::new_v4().to_string();
    let manager = SoftwareManager::default();
    manager.lock().unwrap().snapshot = Some(Snapshot { revision: revision.clone(), installations: vec![software],
        issues: Vec::new(), task: None });
    let plan = manager.prepare(&state, &installation.id, &revision).unwrap();
    assert_eq!(plan.from_version, from);
    assert_eq!(plan.target_version, target);
    assert_eq!(plan.launcher, launcher);
    manager.start(&state, &plan.id).unwrap();
    let finished = wait_for_completion(&manager);
    let saved = journal::read(&state).unwrap().unwrap();
    fs::write(root.join(format!("{}-update-result.json", discovery::name(app))),
        serde_json::to_vec_pretty(&serde_json::json!({ "metadata": metadata, "operation": finished,
            "journal": saved, "installationBefore": installation })).unwrap()).unwrap();
    assert_eq!(finished.phase, Phase::Completed, "Update failed: {:?}; output: {:?}", finished.error, finished.log);
    assert_eq!(finished.actual_version.as_deref(), Some(target.as_str()));
    assert!(finished.error.is_none());
    assert!(!finished.log.is_empty(), "The real npm updater must produce captured output");
    assert_eq!(saved.phase, Phase::Completed);
    assert_eq!(saved.actual_version, finished.actual_version);
    let actual = discovery::inspect(app, &launcher).unwrap();
    assert_eq!(actual.id, installation.id);
    assert_eq!(actual.manager, Manager::Npm);
    assert_eq!(actual.version.as_deref(), Some(target.as_str()));
    assert_eq!(actual.root, installation.root);
    let manifest = discovery::json(&actual.root.join("package.json")).unwrap().unwrap();
    assert_eq!(manifest["version"].as_str(), Some(target.as_str()));
    println!("VERIFIED {}: {from} -> {target}; launcher: {}", app.label(), launcher.display());
}

fn wait_for_completion(manager: &SoftwareManager) -> UpdateTask {
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut phase = None;
    loop {
        let task = manager.task().unwrap().unwrap();
        if phase != Some(task.phase) {
            println!("{} update phase: {:?}", task.plan.app.label(), task.phase);
            phase = Some(task.phase);
        }
        if !task.phase.active() { return task; }
        assert!(Instant::now() < deadline, "Isolated update did not finish: {:?}", task.log);
        std::thread::sleep(Duration::from_millis(100));
    }
}
