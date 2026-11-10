use std::{io::Read, path::{Path, PathBuf}, process::{Child, Command, Stdio}, sync::mpsc, time::{Duration, Instant}};

/// Node/npm path arguments need ordinary paths; installation identity stays canonical.
pub fn command_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    if let Some(path) = path.to_str() {
        if let Some(path) = path.strip_prefix(r"\\?\UNC\") { return PathBuf::from(format!(r"\\{path}")); }
        if let Some(path) = path.strip_prefix(r"\\?\") { return PathBuf::from(path); }
    }
    path.to_path_buf()
}

pub fn quiet(command: &mut Command) -> &mut Command {
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

pub fn output(command: &mut Command, timeout: Duration) -> Result<String, String> {
    let mut child = quiet(command).spawn().map_err(|e| e.to_string())?;
    let stderr = child.stderr.take().map(|pipe| Box::new(pipe) as Box<dyn Read + Send>);
    let error_receiver = pipes(stderr.into_iter().collect());
    let receiver = drain(&mut child);
    let deadline = Instant::now() + timeout;
    let mut bytes = Vec::new();
    let mut errors = Vec::new();
    let status = loop {
        collect(&receiver, &mut bytes);
        collect(&error_receiver, &mut errors);
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? { break status; }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Command timed out".into());
        }
        std::thread::sleep(Duration::from_millis(40));
    };
    // Readers finish independently; a descendant retaining a pipe cannot block the UI.
    let finish = Instant::now() + Duration::from_millis(300);
    while Instant::now() < finish {
        if let Ok(chunk) = receiver.recv_timeout(Duration::from_millis(20)) { append(&mut bytes, &chunk.bytes); }
        collect(&error_receiver, &mut errors);
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if status.success() { Ok(text) } else {
        Err(command_error(command, &format!("{status}: {text} {}", String::from_utf8_lossy(&errors))))
    }
}

fn command_error(command: &Command, message: &str) -> String {
    let mut clean = message.to_string();
    for (key, value) in command.get_envs() {
        if key.to_string_lossy().to_ascii_lowercase().contains("proxy") {
            if let Some(value) = value { clean = redact_url(&clean, &value.to_string_lossy()); }
        }
    }
    let mut args = command.get_args();
    while let Some(arg) = args.next() {
        if arg == "--registry" { if let Some(address) = args.next() { clean = redact_url(&clean, &address.to_string_lossy()); } }
    }
    scrub(&clean)
}

pub struct Chunk { pub stderr: bool, pub bytes: Vec<u8> }

pub fn drain(child: &mut Child) -> mpsc::Receiver<Chunk> {
    let pipes: Vec<Box<dyn Read + Send>> = child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>)
        .into_iter().chain(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>)).collect();
    self::pipes(pipes)
}

fn pipes(pipes: Vec<Box<dyn Read + Send>>) -> mpsc::Receiver<Chunk> {
    let (sender, receiver) = mpsc::sync_channel(32);
    for (index, mut pipe) in pipes.into_iter().enumerate() {
        let sender = sender.clone();
        std::thread::spawn(move || {
            let mut buffer = [0; 2048];
            loop {
                match pipe.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => { if sender.send(Chunk { stderr: index == 1, bytes: buffer[..n].to_vec() }).is_err() { break; } }
                }
            }
        });
    }
    receiver
}

pub fn collect(receiver: &mpsc::Receiver<Chunk>, bytes: &mut Vec<u8>) {
    while let Ok(chunk) = receiver.try_recv() { append(bytes, &chunk.bytes); }
}

fn append(bytes: &mut Vec<u8>, chunk: &[u8]) {
    const LIMIT: usize = 262_144;
    let remaining = LIMIT.saturating_sub(bytes.len());
    bytes.extend_from_slice(&chunk[..remaining.min(chunk.len())]);
}

pub fn scrub(text: &str) -> String {
    let mut clean = text.to_string();
    for token in text.split_whitespace() {
        if let Some(start) = token.find("://") {
            let authority = token[start + 3..].split(['/', '?', '#', '"', '\'']).next().unwrap_or_default();
            if let Some(at) = authority.rfind('@') { clean = clean.replace(&authority[..at], "[redacted]"); }
        }
    }
    asb_core::adapter::scrub_message(clean).chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .take(4096).collect()
}

pub fn redact_url(text: &str, address: &str) -> String {
    if address.is_empty() { return text.into(); }
    let mut clean = text.replace(address, "[redacted-url]");
    if let Ok(url) = reqwest::Url::parse(address) {
        if let Some(password) = url.password().filter(|password| !password.is_empty()) {
            clean = clean.replace(password, "[redacted]");
            let decoded = decode_percent(password);
            if !decoded.is_empty() { clean = clean.replace(&decoded, "[redacted]"); }
        }
    }
    clean
}

fn decode_percent(value: &str) -> String {
    let mut bytes = Vec::new(); let mut input = value.as_bytes().iter().copied();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let pair = input.next().zip(input.next()).and_then(|(a, b)| Some(((a as char).to_digit(16)?, (b as char).to_digit(16)?)));
            if let Some((a, b)) = pair { bytes.push((a * 16 + b) as u8); } else { return String::new(); }
        } else { bytes.push(byte); }
    }
    String::from_utf8(bytes).unwrap_or_default()
}

pub fn version(text: &str) -> Option<String> {
    text.split_whitespace().find_map(|word| {
        let candidate = word.trim_start_matches('v');
        semver::Version::parse(candidate).ok().map(|v| v.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn node_paths_project_drives_and_unc_without_changing_regular_paths() {
        for (source, expected) in [
            (r"\\?\C:\安装 目录\npm-cli.js", r"C:\安装 目录\npm-cli.js"),
            (r"\\?\UNC\server\share\codex.js", r"\\server\share\codex.js"),
            (r"C:\node\npm-cli.js", r"C:\node\npm-cli.js"),
            (r"\\?\C:\npm prefix", r"C:\npm prefix"),
            (r"\\?\UNC\server\npm prefix", r"\\server\npm prefix"),
        ] { assert_eq!(command_path(Path::new(source)), PathBuf::from(expected)); }
    }

    #[cfg(unix)]
    #[test]
    fn command_paths_preserve_unix_path_bytes() {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
        let path = Path::new(OsStr::from_bytes(b"/node/\xff/npm-cli.js"));
        assert_eq!(command_path(path).as_os_str().as_bytes(), path.as_os_str().as_bytes());
    }
}
