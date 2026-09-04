use asb_core::contracts::AppKind;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::Command;

pub(super) fn resume_command(app: AppKind, session_id: &str) -> String {
    resume_arguments(app, session_id).join(" ")
}

pub(super) fn resume_arguments(app: AppKind, session_id: &str) -> Vec<&str> {
    match app {
        AppKind::Codex => vec!["codex", "resume", session_id],
        AppKind::Claude => vec!["claude", "--resume", session_id],
    }
}

#[cfg(windows)]
pub(super) fn launch_terminal(arguments: &[&str], project_dir: Option<&str>) -> Result<(), String> {
    let mut terminal = Command::new("cmd.exe");
    terminal
        .args(["/d", "/k"])
        .args(arguments)
        .creation_flags(CREATE_NEW_CONSOLE);
    if let Some(project_dir) = project_dir {
        terminal.current_dir(project_dir);
    }
    terminal
        .spawn()
        .map_err(|error| format!("无法启动命令提示符：{error}"))?;
    Ok(())
}

#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

#[cfg(all(unix, not(target_os = "macos")))]
pub(super) fn launch_terminal(arguments: &[&str], project_dir: Option<&str>) -> Result<(), String> {
    let script = shell_command(arguments);

    let spawn = |program: &str, flags: &[&str]| -> Result<(), String> {
        let mut terminal = Command::new(program);
        terminal.args(flags).args(["sh", "-lc", &script]);
        if let Some(project_dir) = project_dir {
            terminal.current_dir(project_dir);
        }
        terminal
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("无法启动终端：{error}"))
    };

    if let Some(configured) = std::env::var_os("TERMINAL")
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
    {
        if spawn(&configured.to_string_lossy(), &["-e"]).is_ok() {
            return Ok(());
        }
    }
    let candidates: &[(&str, &[&str])] = &[
        ("gnome-terminal", &["--"]),
        ("konsole", &["-e"]),
        ("xfce4-terminal", &["-x"]),
        ("x-terminal-emulator", &["-e"]),
        ("alacritty", &["-e"]),
        ("kitty", &[]),
    ];
    for (program, flags) in candidates {
        if spawn(program, flags).is_ok() {
            return Ok(());
        }
    }
    Err("无法启动终端：没有可用的终端程序".to_string())
}

#[cfg(target_os = "macos")]
pub(super) fn launch_terminal(arguments: &[&str], project_dir: Option<&str>) -> Result<(), String> {
    let command = shell_command(arguments);
    let script = match project_dir {
        Some(project_dir) => format!("cd -- {} && {command}", shell_quote(project_dir)),
        None => command,
    };
    let escaped = script.replace('\\', "\\\\").replace('"', "\\\"");
    Command::new("osascript")
        .args([
            "-e",
            &format!("tell application \"Terminal\" to do script \"{escaped}\""),
        ])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法启动终端：{error}"))?;
    Ok(())
}

#[cfg(any(unix, test))]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

#[cfg(any(unix, test))]
pub(super) fn shell_command(arguments: &[&str]) -> String {
    arguments
        .iter()
        .map(|argument| shell_quote(argument))
        .collect::<Vec<_>>()
        .join(" ")
}
