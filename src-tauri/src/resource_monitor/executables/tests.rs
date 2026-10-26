use super::*;
use crate::resource_monitor::observation::ProcessKey;
use std::{path::PathBuf, time::Instant};

fn catalog() -> Executables { Executables { paths: HashMap::new(), installations: Installations::new() } }

fn observation(path: PathBuf) -> Observation {
    Observation { key: ProcessKey { pid: 1, birth: 1 }, parent_pid: 0,
        name: path.file_name().unwrap().to_string_lossy().into_owned(), executable: Some(path),
        cpu_ns: Ok(0), resident_bytes: Ok(1024), at: Instant::now() }
}

fn package(manifest: &str, entry: &str) -> (tempfile::TempDir, Observation) {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("package.json"), manifest).unwrap();
    let path = directory.path().join(entry);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"MZ\0\0").unwrap();
    (directory, observation(path))
}

#[test]
fn installed_editor_engines_are_recognized_in_custom_extension_directories_without_path() {
    let catalog = catalog();
    for (manifest, entry, group) in [
        (r#"{"name":"chatgpt","publisher":"openai"}"#, "bin/windows-x86_64/codex.exe", Group::Codex),
        (r#"{"name":"chatgpt","publisher":"OpenAI"}"#, "bin/macos-aarch64/codex", Group::Codex),
        (r#"{"name":"chatgpt","publisher":"openai"}"#, "bin/linux-x86_64/codex", Group::Codex),
        (r#"{"name":"claude-code","publisher":"Anthropic"}"#, "resources/native-binary/claude.exe", Group::Claude),
        (r#"{"name":"claude-code","publisher":"anthropic"}"#, "resources/native-binary/claude", Group::Claude),
    ] {
        let (_directory, process) = package(manifest, entry);
        assert_eq!(catalog.classify(&process), Ok(Some(group)), "{entry}");
    }
}

#[test]
fn npm_native_payloads_use_their_own_manifest_and_current_entry_layout() {
    let catalog = catalog();
    for (manifest, entry, group) in [
        (r#"{"name":"@openai/codex"}"#, "vendor/x86_64-pc-windows-msvc/bin/codex.exe", Group::Codex),
        (r#"{"name":"@openai/codex"}"#, "vendor/aarch64-apple-darwin/bin/codex", Group::Codex),
        (r#"{"name":"@anthropic-ai/claude-code"}"#, "bin/claude.exe", Group::Claude),
        (r#"{"name":"@anthropic-ai/claude-code-win32-x64"}"#, "claude.exe", Group::Claude),
        (r#"{"name":"@anthropic-ai/claude-code-linux-arm64-musl"}"#, "claude", Group::Claude),
    ] {
        let (_directory, process) = package(manifest, entry);
        assert_eq!(catalog.classify(&process), Ok(Some(group)), "{entry}");
    }
}

#[test]
fn native_release_manifests_identify_custom_prefixes_without_path_or_npm() {
    let catalog = catalog();
    let (directory, process) = package(
        r#"{"layoutVersion":1,"variant":"codex","entrypoint":"bin/codex"}"#, "bin/codex");
    fs::rename(directory.path().join("package.json"), directory.path().join("codex-package.json")).unwrap();
    assert_eq!(catalog.classify(&process), Ok(Some(Group::Codex)));
    for manifest in [r#"{"layoutVersion":2,"variant":"codex","entrypoint":"bin/codex"}"#,
        r#"{"layoutVersion":1,"variant":"other","entrypoint":"bin/codex"}"#,
        r#"{"layoutVersion":1,"variant":"codex","entrypoint":"other/codex"}"#] {
        fs::write(directory.path().join("codex-package.json"), manifest).unwrap();
        assert_eq!(catalog.classify(&process), Err(Issue::UnrecognizedExecutable));
    }
}

#[test]
fn flattened_editor_payload_uses_the_editors_layout_instead_of_the_copied_release_layout() {
    let catalog = catalog();
    let (_directory, process) = package(r#"{"name":"chatgpt","publisher":"openai"}"#, "bin/windows-x86_64/codex.exe");
    fs::write(process.executable.as_ref().unwrap().parent().unwrap().join("codex-package.json"),
        r#"{"layoutVersion":1,"variant":"codex","entrypoint":"bin/codex.exe"}"#).unwrap();
    assert_eq!(catalog.classify(&process), Ok(Some(Group::Codex)));
}

#[test]
fn package_names_publishers_and_entry_layouts_must_agree() {
    let catalog = catalog();
    for (manifest, entry) in [
        (r#"{"name":"chatgpt","publisher":"unrelated"}"#, "bin/windows-x86_64/codex.exe"),
        (r#"{"name":"claude-code","publisher":"unrelated"}"#, "resources/native-binary/claude"),
        (r#"{"name":"@openai/codex"}"#, "unrelated/codex"),
        (r#"{"name":"@anthropic-ai/claude-code"}"#, "arbitrary/nested/claude"),
        (r#"{"name":"@anthropic-ai/claude-code-fake"}"#, "claude"),
        (r#"{"name":"unrelated"}"#, "@openai/codex-fake/bin/codex"),
    ] {
        let (_directory, process) = package(manifest, entry);
        assert_eq!(catalog.classify(&process), Err(Issue::UnrecognizedExecutable), "{entry}");
    }
}

#[test]
fn scripts_and_unrelated_versioned_processes_never_become_native_engines() {
    let catalog = catalog();
    let (_directory, script) = package(r#"{"name":"@anthropic-ai/claude-code"}"#, "bin/claude");
    fs::write(script.executable.as_ref().unwrap(), b"#!/bin/sh\n").unwrap();
    assert_eq!(catalog.classify(&script), Err(Issue::UnrecognizedExecutable));
    let (_directory, versioned) = package(r#"{"name":"unrelated"}"#, "2.1.275");
    assert!(candidate(&versioned.name));
    assert_eq!(named_group(&versioned.name), None);
    assert_eq!(catalog.classify(&versioned), Ok(None));
}

#[test]
fn helpers_services_and_generic_runtimes_are_not_candidate_engine_roots() {
    for name in ["codex-code-mode-host.exe", "codex-windows-sandbox-service.exe", "codex-command-runner",
        "claude-code-helper", "node.exe", "rg", "ChatGPT.exe"] { assert!(!candidate(name), "{name}"); }
    for name in ["codex", "Codex.exe", "claude", "CLAUDE.EXE", "claude-code", "2.1.275.EXE"] {
        assert!(candidate(name), "{name}");
    }
}

#[test]
fn unreadable_or_oversized_package_metadata_keeps_the_identity_unknown() {
    let catalog = catalog();
    let (directory, process) = package(r#"{"name":"chatgpt","publisher":"openai"}"#, "bin/windows-x86_64/codex");
    fs::write(directory.path().join("package.json"), vec![b' '; 262_145]).unwrap();
    assert_eq!(catalog.classify(&process), Err(Issue::IdentityUnavailable));
    fs::write(directory.path().join("package.json"), b"{").unwrap();
    assert_eq!(catalog.classify(&process), Err(Issue::IdentityUnavailable));
}
