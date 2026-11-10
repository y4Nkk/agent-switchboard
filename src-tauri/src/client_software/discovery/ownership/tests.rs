use super::*;
use std::fs;

fn item(app: AppKind, launcher: PathBuf, executable: PathBuf) -> Installation {
    Installation { id: String::new(), app, form: Form::Cli, manager: Manager::Manual,
        root: launcher.parent().unwrap().into(), launcher, executable,
        version: None, version_text: None, active: false, channel: "latest".into(), detail: None, owner: None }
}

#[test]
fn npm_shim_uses_its_canonical_package_and_original_runtime_without_executing_them() {
    for (app, package, entry, platform_package) in [
        (AppKind::Codex, "@openai/codex", "bin/codex.js", "@openai/codex-win32-x64"),
        (AppKind::Claude, "@anthropic-ai/claude-code", "bin/claude.exe", "@anthropic-ai/claude-code-win32-x64"),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let prefix = directory.path();
        let modules = prefix.join(if cfg!(windows) { "node_modules" } else { "lib/node_modules" });
        let root = modules.join(package);
        fs::create_dir_all(root.join("bin")).unwrap();
        let manifest = serde_json::json!({ "name": package, "bin": { (name(app)): entry },
            "optionalDependencies": { (platform_package): "fixture", "../../unrelated": "fixture" } });
        fs::write(root.join("package.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
        fs::write(root.join(entry), "fixture, never executed").unwrap();
        let node_dir = if cfg!(windows) { prefix.to_path_buf() } else { prefix.join("bin") };
        fs::create_dir_all(&node_dir).unwrap();
        let node = node_dir.join(if cfg!(windows) { "node.exe" } else { "node" });
        fs::write(&node, b"fixture").unwrap();
        fs::create_dir_all(modules.join("npm/bin")).unwrap();
        fs::write(modules.join("npm/bin/npm-cli.js"), b"fixture").unwrap();
        let launcher = node_dir.join(if cfg!(windows) { format!("{}.cmd", name(app)) } else { name(app).into() });
        fs::write(&launcher, b"fixture").unwrap();
        let executable = if cfg!(windows) { fs::canonicalize(&launcher) } else { fs::canonicalize(root.join(entry)) }.unwrap();
        let mut installation = item(app, launcher, executable);
        assert!(npm(&mut installation).unwrap());
        assert_eq!(installation.root, fs::canonicalize(&root).unwrap());
        let Some(Owner::Npm { node: resolved, npm: resolved_npm, prefix: actual, package: owned, registry }) = &installation.owner else { panic!("npm owner missing"); };
        assert_eq!(resolved, &fs::canonicalize(node).unwrap());
        assert_eq!(actual, &fs::canonicalize(prefix).unwrap());
        assert_eq!(owned, package);
        assert!(registry.is_none());
        let owner = Owner::Npm { node: resolved.clone(), npm: resolved_npm.clone(), prefix: actual.clone(),
            package: owned.clone(), registry: Some("https://registry.invalid/".into()) };
        let command = crate::client_software::npm::command(&owner, &Default::default()).unwrap();
        let args: Vec<_> = command.get_args().collect();
        let npm_entry = crate::client_software::process::command_path(resolved_npm);
        let command_prefix = crate::client_software::process::command_path(actual);
        assert_eq!(command.get_program(), resolved.as_os_str());
        assert_eq!(Path::new(args[0]), npm_entry.as_path());
        assert_eq!(Path::new(args[3]), command_prefix.as_path());
        if app == AppKind::Codex {
            let command = crate::client_software::discovery::launch::command(&installation).unwrap();
            let entry = crate::client_software::process::command_path(&installation.executable);
            assert_eq!(command.get_program(), resolved.as_os_str());
            assert_eq!(Path::new(command.get_args().next().unwrap()), entry.as_path());
        }
        let payloads = [root.join("node_modules").join(platform_package), modules.join(platform_package)];
        for payload in &payloads { fs::create_dir_all(payload).unwrap(); }
        let roots = crate::client_software::discovery::process_roots(&installation).unwrap();
        assert_eq!(roots.len(), 3);
        for payload in payloads { assert!(roots.contains(&fs::canonicalize(payload).unwrap())); }
    }
}

#[test]
fn native_codex_requires_current_package_ownership_and_a_stable_launcher() {
    let directory = tempfile::tempdir().unwrap();
    let release = directory.path().join("custom-home/packages/standalone/releases/0.165.0-fixture");
    fs::create_dir_all(release.join("bin")).unwrap();
    let executable = release.join("bin/codex");
    fs::write(&executable, b"MZ\0\0").unwrap();
    fs::write(release.join("codex-package.json"), br#"{"layoutVersion":1,"variant":"codex","entrypoint":"bin/codex"}"#).unwrap();
    let launcher = directory.path().join("visible/codex");
    fs::create_dir_all(launcher.parent().unwrap()).unwrap(); fs::write(&launcher, b"fixture").unwrap();
    let mut installed = item(AppKind::Codex, launcher, fs::canonicalize(&executable).unwrap());
    assert!(native(&mut installed).unwrap());
    assert_eq!(installed.owner, Some(Owner::Native));
    installed.launcher = installed.executable.clone();
    assert!(native(&mut installed).unwrap());
    assert!(installed.owner.is_none());
    installed.launcher = directory.path().join("visible/../custom-home/packages/standalone/releases/0.165.0-fixture/bin/codex");
    assert!(native(&mut installed).unwrap());
    assert!(installed.owner.is_none());
}

#[test]
fn native_claude_recognizes_native_versions_and_requires_an_external_stable_launcher() {
    let directory = tempfile::tempdir().unwrap();
    let home = fs::canonicalize(directory.path()).unwrap();
    let versions = home.join(".local/share/claude/versions");
    let launcher = home.join(".local/bin").join(if cfg!(windows) { "claude.exe" } else { "claude" });
    fs::create_dir_all(&versions).unwrap();
    fs::create_dir_all(launcher.parent().unwrap()).unwrap();
    fs::write(&launcher, b"MZ\0\0").unwrap();
    let mut installed = item(AppKind::Claude, launcher.clone(), launcher.clone());
    assert!(native_claude(&mut installed, &home));
    assert_eq!(installed.root, versions);
    assert_eq!(installed.owner, Some(Owner::Native));
    for version in ["2.1.310", "2.1.310.exe"] {
        let executable = versions.join(version);
        fs::write(&executable, b"MZ\0\0").unwrap();
        let mut installed = item(AppKind::Claude, launcher.clone(), executable.clone());
        assert!(native_claude(&mut installed, &home));
        assert_eq!(installed.owner, Some(Owner::Native));
        installed.launcher = executable;
        assert!(native_claude(&mut installed, &home));
        assert!(installed.owner.is_none());
        assert!(installed.detail.is_some());
        installed.launcher = home.join(".local/bin/../share/claude/versions").join(version);
        assert!(native_claude(&mut installed, &home));
        assert!(installed.owner.is_none());
    }
}

#[test]
fn native_claude_rejects_arbitrary_nested_and_non_native_version_files() {
    let directory = tempfile::tempdir().unwrap();
    let home = fs::canonicalize(directory.path()).unwrap();
    let versions = home.join(".local/share/claude/versions");
    let launcher = home.join(".local/bin").join(if cfg!(windows) { "claude.exe" } else { "claude" });
    for (filename, native_file) in [("not-a-version", true), ("nested/2.1.310", true),
        ("2.1.310/claude", true), ("3.0.0", false)] {
        let executable = versions.join(filename);
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, if native_file { &b"MZ\0\0"[..] } else { &b"not a native executable"[..] }).unwrap();
        let mut installed = item(AppKind::Claude, launcher.clone(), executable);
        assert!(!native_claude(&mut installed, &home), "{filename}");
        assert!(installed.owner.is_none());
        assert_eq!(installed.manager, Manager::Manual);
    }
}

#[cfg(unix)]
#[test]
fn native_ownership_preserves_external_stable_file_links() {
    for (app, engine) in [(AppKind::Codex, "codex"), (AppKind::Claude, "claude")] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("versions");
        let launcher = directory.path().join("bin").join(engine);
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(launcher.parent().unwrap()).unwrap();
        let executable = root.join(engine);
        fs::write(&executable, b"MZ\0\0").unwrap();
        std::os::unix::fs::symlink(&executable, &launcher).unwrap();
        let mut installed = item(app, launcher, fs::canonicalize(executable).unwrap());
        installed.root = root;
        native_owner(&mut installed);
        assert_eq!(installed.owner, Some(Owner::Native));
    }
}
