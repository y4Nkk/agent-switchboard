use super::*;
use std::fs;

fn item(app: AppKind, launcher: PathBuf, executable: PathBuf) -> Installation {
    Installation { id: String::new(), app, form: Form::Cli, manager: Manager::Manual,
        root: launcher.parent().unwrap().into(), launcher, executable,
        version: None, version_text: None, active: false, channel: "latest".into(), detail: None, owner: None }
}

#[test]
fn npm_shim_uses_its_canonical_package_and_original_runtime_without_executing_them() {
    let directory = tempfile::tempdir().unwrap();
    let prefix = directory.path();
    let modules = prefix.join(if cfg!(windows) { "node_modules" } else { "lib/node_modules" });
    let root = modules.join("@openai/codex");
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(root.join("package.json"), br#"{"name":"@openai/codex","bin":{"codex":"bin/codex.js"},"optionalDependencies":{"@openai/codex-win32-x64":"fixture","../../unrelated":"fixture"}}"#).unwrap();
    fs::write(root.join("bin/codex.js"), "fixture, never executed").unwrap();
    let node_dir = if cfg!(windows) { prefix.to_path_buf() } else { prefix.join("bin") };
    fs::create_dir_all(&node_dir).unwrap();
    let node = node_dir.join(if cfg!(windows) { "node.exe" } else { "node" });
    fs::write(&node, b"fixture").unwrap();
    fs::create_dir_all(modules.join("npm/bin")).unwrap();
    fs::write(modules.join("npm/bin/npm-cli.js"), b"fixture").unwrap();
    let launcher = node_dir.join(if cfg!(windows) { "codex.cmd" } else { "codex" });
    fs::write(&launcher, b"fixture").unwrap();
    let executable = if cfg!(windows) { fs::canonicalize(&launcher).unwrap() } else { fs::canonicalize(root.join("bin/codex.js")).unwrap() };
    let mut installation = item(AppKind::Codex, launcher, executable);
    assert!(npm(&mut installation).unwrap());
    assert_eq!(installation.root, fs::canonicalize(&root).unwrap());
    let Some(Owner::Npm { node: resolved, prefix: actual, registry, .. }) = &installation.owner else { panic!("npm owner missing"); };
    assert_eq!(resolved, &fs::canonicalize(node).unwrap());
    assert_eq!(actual, &fs::canonicalize(prefix).unwrap());
    assert!(registry.is_none());
    let payload = modules.join("@openai/codex-win32-x64");
    fs::create_dir_all(&payload).unwrap();
    let roots = crate::client_software::discovery::process_roots(&installation).unwrap();
    assert_eq!(roots.len(), 2);
    assert!(roots.contains(&fs::canonicalize(payload).unwrap()));
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
    installed.launcher = installed.executable.clone(); installed.owner = None;
    assert!(native(&mut installed).unwrap());
    assert!(installed.owner.is_none());
}
