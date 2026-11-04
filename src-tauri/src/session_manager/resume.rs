use asb_core::contracts::AppKind;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};

pub(super) fn launch_client(app: AppKind, session_id: &str, project_dir: Option<&str>) -> Result<String, String> {
    let lease = crate::client_software::gate::launch(app)?;
    let installation = crate::client_software::discovery::resolve(app)?;
    let mut arguments = crate::client_software::discovery::launch::arguments(&installation)?;
    arguments.extend(resume_arguments(app, session_id).into_iter().skip(1).map(str::to_string));
    let borrowed: Vec<_> = arguments.iter().map(String::as_str).collect();
    let mut terminal = launch_terminal(&borrowed, project_dir)?;
    std::thread::spawn(move || { let _lease = lease; let _ = terminal.wait(); });
    Ok(borrowed.iter().map(|arg| command_quote(arg)).collect::<Vec<_>>().join(" "))
}

pub(super) fn resume_command(app: AppKind, session_id: &str) -> String {
    resume_arguments(app, session_id).iter().map(|argument| command_quote(argument)).collect::<Vec<_>>().join(" ")
}

pub(super) fn resume_arguments(app: AppKind, session_id: &str) -> Vec<&str> {
    match app {
        AppKind::Codex => vec!["codex", "resume", session_id],
        AppKind::Claude => vec!["claude", "--resume", session_id],
    }
}

#[cfg(windows)]
fn launch_terminal(arguments: &[&str], project_dir: Option<&str>) -> Result<Child, String> {
    use base64::Engine;
    let (program, arguments) = arguments.split_first().ok_or("缺少恢复客户端")?;
    let script = format!("& {} {}",
        command_quote(program), arguments.iter().map(|argument| command_quote(argument)).collect::<Vec<_>>().join(" "));
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let mut terminal = Command::new("powershell.exe");
    terminal
        .args(["-NoLogo", "-NoProfile", "-NoExit", "-EncodedCommand", &encoded])
        .creation_flags(CREATE_NEW_CONSOLE);
    if let Some(project_dir) = project_dir {
        terminal.current_dir(project_dir);
    }
    terminal
        .spawn()
        .map_err(|error| format!("无法启动恢复终端：{error}"))
}

#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

#[cfg(all(unix, not(target_os = "macos")))]
fn launch_terminal(arguments: &[&str], project_dir: Option<&str>) -> Result<Child, String> {
    let script = shell_command(arguments);

    let spawn = |program: &str, flags: &[&str]| -> Result<Child, String> {
        let mut terminal = Command::new(program);
        terminal.args(flags).args(["sh", "-lc", &script]);
        if let Some(project_dir) = project_dir {
            terminal.current_dir(project_dir);
        }
        terminal
            .spawn()
            .map_err(|error| format!("无法启动终端：{error}"))
    };

    if let Some(configured) = std::env::var_os("TERMINAL")
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
    {
        if let Ok(child) = spawn(&configured.to_string_lossy(), &["-e"]) {
            return Ok(child);
        }
    }
    let candidates: &[(&str, &[&str])] = &[
        ("gnome-terminal", &["--wait", "--"]),
        ("konsole", &["-e"]),
        ("xfce4-terminal", &["-x"]),
        ("x-terminal-emulator", &["-e"]),
        ("alacritty", &["-e"]),
        ("kitty", &[]),
    ];
    for (program, flags) in candidates {
        if let Ok(child) = spawn(program, flags) {
            return Ok(child);
        }
    }
    Err("无法启动终端：没有可用的终端程序".to_string())
}

#[cfg(target_os = "macos")]
fn launch_terminal(arguments: &[&str], project_dir: Option<&str>) -> Result<Child, String> {
    let command = shell_command(arguments);
    let script = match project_dir {
        Some(project_dir) => format!("cd -- {} && {command}", shell_quote(project_dir)),
        None => command,
    };
    let escaped = script.replace('\\', "\\\\").replace('"', "\\\"");
    Command::new("osascript")
        .args([
            "-e",
            &format!("tell application \"Terminal\"\nset launchedTab to do script \"{escaped}\"\ndelay 1\nrepeat while busy of launchedTab\ndelay 1\nend repeat\nend tell"),
        ])
        .spawn()
        .map_err(|error| format!("无法启动终端：{error}"))
}

#[cfg(any(unix, test))]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

fn command_quote(value: &str) -> String {
    if value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte)) { return value.into(); }
    #[cfg(windows)]
    { format!("'{}'", value.replace('\'', "''")) }
    #[cfg(not(windows))]
    { shell_quote(value) }
}

#[cfg(any(unix, test))]
pub(super) fn shell_command(arguments: &[&str]) -> String {
    arguments
        .iter()
        .map(|argument| shell_quote(argument))
        .collect::<Vec<_>>()
        .join(" ")
}
