use std::{io::Write, process::{Child, Command, Stdio}};

/// EOF before the recorded handoff prevents starting an untracked installer.
/// After handoff, ASB never terminates the installer on its own exit.
pub fn recorded(command: &mut Command, record: impl FnOnce(u32, String) -> Result<(), String>) -> Result<Child, String> {
    let mut wrapper = waiting(command)?;
    for (key, value) in command.get_envs() {
        if let Some(value) = value { wrapper.env(key, value); } else { wrapper.env_remove(key); }
    }
    if let Some(directory) = command.get_current_dir() { wrapper.current_dir(directory); }
    wrapper.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::process_control::suppress_window(&mut wrapper);
    let mut child = wrapper.spawn().map_err(|e| e.to_string())?;
    let result = (|| {
        let birth = crate::resource_monitor::client_processes::birth(child.id())?
            .ok_or("Updater process exited before its identity was recorded")?;
        record(child.id(), birth)?;
        child.stdin.take().ok_or("Updater start pipe unavailable")?.write_all(b"start\n").map_err(|e| e.to_string())
    })();
    if let Err(error) = result {
        // The installer has not received permission to begin yet.
        let _ = child.kill(); let _ = child.wait();
        return Err(error);
    }
    Ok(child)
}

#[cfg(windows)]
fn waiting(command: &Command) -> Result<Command, String> {
    let system = std::env::var_os("SystemRoot").ok_or("Windows system directory unavailable")?;
    let mut wrapper = Command::new(std::path::PathBuf::from(system).join("System32/WindowsPowerShell/v1.0/powershell.exe"));
    // Only this fixed program is interpreted. Paths and arguments remain data.
    wrapper.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", include_str!("start-updater.ps1")]);
    let arguments: Result<Vec<_>, _> = command.get_args().map(|a| a.to_str().ok_or("Updater argument is not Unicode")).collect();
    wrapper.env("ASB_UPDATER_PROGRAM", command.get_program().to_str().ok_or("Updater path is not Unicode")?)
        .env("ASB_UPDATER_ARGUMENTS", serde_json::to_string(&arguments?).map_err(|e| e.to_string())?);
    Ok(wrapper)
}

#[cfg(unix)]
fn waiting(command: &Command) -> Result<Command, String> {
    let mut wrapper = Command::new("/bin/sh");
    wrapper.args(["-c", "IFS= read -r ready || exit 1; [ \"$ready\" = start ] || exit 1; exec \"$@\"", "asb-updater"])
        .arg(command.get_program()).args(command.get_args());
    Ok(wrapper)
}
