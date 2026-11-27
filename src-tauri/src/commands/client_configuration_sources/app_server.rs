use crate::client_software::{discovery::launch, types::Installation};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, ChildStdin, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

const RESPONSE_LIMIT: u64 = 4 * 1024 * 1024;
const TIME_LIMIT: Duration = Duration::from_secs(10);

pub(super) struct AppServer {
    child: Child,
    input: ChildStdin,
    responses: Receiver<Result<Value, String>>,
    deadline: Instant,
    id: u64,
}

impl AppServer {
    pub(super) fn start(installation: &Installation, safe_cwd: &Path) -> Result<Self, String> {
        let mut command = launch::command(installation)?;
        crate::process_control::suppress_window(&mut command);
        let mut child = command.arg("app-server").current_dir(safe_cwd)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
            .spawn().map_err(|error| format!("Codex App Server: {error}"))?;
        let input = child.stdin.take().expect("piped stdin");
        let output = child.stdout.take().expect("piped stdout");
        let (sender, responses) = mpsc::sync_channel(4);
        std::thread::spawn(move || read_responses(output, sender));
        let mut server = Self { child, input, responses, deadline: Instant::now() + TIME_LIMIT, id: 0 };
        server.call("initialize", json!({"clientInfo": {
            "name": "agent-switchboard", "title": "Agent Switchboard", "version": env!("CARGO_PKG_VERSION")
        }}))?;
        server.write(&json!({"method": "initialized"}))?;
        Ok(server)
    }

    pub(super) fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.id += 1;
        let id = self.id;
        let request = if params.is_null() { json!({"id": id, "method": method}) }
        else { json!({"id": id, "method": method, "params": params}) };
        self.write(&request)?;
        loop {
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            let message = self.responses.recv_timeout(remaining).map_err(|error| format!("Codex {method}: {error}"))??;
            if message.get("id").and_then(Value::as_u64) != Some(id) { continue; }
            if let Some(error) = message.get("error") {
                let code = error.get("code").and_then(Value::as_i64).unwrap_or_default();
                return Err(format!("Codex {method}: RPC {code}"));
            }
            return message.get("result").cloned().ok_or_else(|| format!("Codex {method}: missing result"));
        }
    }

    fn write(&mut self, message: &Value) -> Result<(), String> {
        writeln!(self.input, "{message}").map_err(|error| format!("Codex App Server: {error}"))
    }
}

impl Drop for AppServer {
    fn drop(&mut self) {
        crate::process_control::terminate_process_tree(&mut self.child);
    }
}

fn read_responses(output: impl Read, sender: mpsc::SyncSender<Result<Value, String>>) {
    let mut reader = BufReader::new(output);
    loop {
        let mut line = Vec::new();
        let read = (&mut reader).take(RESPONSE_LIMIT + 1).read_until(b'\n', &mut line);
        let reply = match read {
            Ok(0) => break,
            Ok(size) if size as u64 > RESPONSE_LIMIT => Err("Codex App Server response exceeded the size limit".into()),
            Ok(_) => serde_json::from_slice(&line).map_err(|_| "Codex App Server returned invalid JSON".into()),
            Err(error) => Err(format!("Codex App Server: {error}")),
        };
        let failed = reply.is_err();
        if sender.send(reply).is_err() || failed { break; }
    }
}
