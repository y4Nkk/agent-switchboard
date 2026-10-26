use super::*;

fn installations() -> Installations {
    Installations { codex_bins: vec!["/user/local/OpenAI/Codex/bin".into()],
        claude_downloads: vec!["/user/data/Claude/claude-code".into(), "/user/local/Claude-3p/claude-code".into()],
        claude_versions: Some("/user/.local/share/claude/versions".into()) }
}

#[test]
fn desktop_downloads_are_discovered_without_the_launchers_path() {
    let catalog = installations();
    assert_eq!(catalog.classify("/user/local/OpenAI/Codex/bin/2993767bd1dfb009/codex.exe"), Some(Group::Codex));
    assert_eq!(catalog.classify("/user/data/Claude/claude-code/2.1.310/claude"), Some(Group::Claude));
    assert_eq!(catalog.classify("/user/local/Claude-3p/claude-code/2.1.142/claude.exe"), Some(Group::Claude));
    assert_eq!(catalog.classify("/user/data/Claude/claude-code/2.1.310/0123456789abcdef/claude.exe"), Some(Group::Claude));
    assert_eq!(catalog.classify("/user/.local/share/claude/versions/2.1.310"), Some(Group::Claude));
    assert_eq!(catalog.classify("/user/.local/share/claude/versions/2.1.310.exe"), Some(Group::Claude));
}

#[test]
fn similarly_named_directories_and_helpers_are_not_engine_installations() {
    let catalog = installations();
    for path in ["/other/user/local/OpenAI/Codex/bin/2993767bd1dfb009/codex.exe",
        "/user/local/OpenAI/Codex/bin-extra/2993767bd1dfb009/codex.exe",
        "/user/local/OpenAI/Codex/bin/2993767bd1dfb009/codex-code-mode-host.exe",
        "/user/local/OpenAI/Codex/bin/2993767bd1dfb009/nested/codex.exe",
        "/user/data/Claude/claude-code-vm/2.1.310/claude",
        "/user/data/Claude/claude-code/not-a-version/claude",
        "/user/.local/share/claude/versions/2.1.310/claude",
        "/other/2.1.310"] { assert_eq!(catalog.classify(path), None, "{path}"); }
}

#[test]
fn application_shells_and_bundled_engines_have_different_scope() {
    let catalog = installations();
    for (app, group, engine) in [("Codex", Group::Codex, "codex"), ("Claude", Group::Claude, "claude")] {
        let shell = format!("/Applications/{app}.app/Contents/MacOS/{app}");
        let engine = format!("/Applications/{app}.app/Contents/Resources/{engine}");
        assert!(desktop_shell(&shell));
        assert_eq!(catalog.classify(&crate::resource_monitor::executables::normalize(std::path::Path::new(&engine))), Some(group));
    }
    assert!(desktop_shell("c:/program files/windowsapps/claude_2.1_x64__pzs8sxrjxfjjc/app/claude.exe"));
    assert!(desktop_shell("c:/program files/windowsapps/openai.codex_26_x64__package/app/codex.exe"));
    assert!(!desktop_shell("/user/data/Claude/claude-code/2.1.310/claude.exe"));
}
