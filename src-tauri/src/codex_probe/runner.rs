use super::output::{diagnostic, parse_events, read_usage};
use super::questions::answer_matches;
use super::progress::RunProgress;
use super::{ProbeGrading, ProbeRunOutcome, ProbeRunResult, ResolvedQuestion};
use crate::process_control::terminate_process_tree;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub(super) enum ProbeAbort {
    Cancelled,
    ConfigChanged,
    Spawn(String),
}

// The guard owns only the UUID directory it created, on every exit path.
struct Workdir(PathBuf);
impl Workdir {
    fn create() -> Result<Self, ProbeAbort> {
        let path = std::env::temp_dir().join(format!("asb-codex-probe-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path)
            .map_err(|_| ProbeAbort::Spawn("无法创建临时工作目录".to_string()))?;
        Ok(Self(path))
    }
}
impl Drop for Workdir {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            log::warn!("无法清理检测临时目录 {}: {error}", self.0.display());
        }
    }
}

/// Runs one probe call. `on_session_id` fires the moment the CLI reports its
/// session identity on stdout — before the process exits — so the executor
/// can persist it immediately.
pub(super) fn run_once(
    question: &ResolvedQuestion,
    cancel: &AtomicBool,
    on_session_id: &Arc<dyn Fn(&str) + Send + Sync>,
    check_config: &dyn Fn() -> Result<(), ProbeAbort>,
    progress: &RunProgress,
) -> Result<ProbeRunResult, ProbeAbort> {
    if cancel.load(Ordering::SeqCst) { return Err(ProbeAbort::Cancelled); }
    check_config()?;
    let started = progress.started();
    let workdir = Workdir::create()?;
    let answer_path = workdir.0.join("answer.txt");
    let mut command = Command::new(codex_executable());
    // Keep the active client configuration; only output transport and answer
    // formatting belong to the probe. Never include the expected answer.
    command.args(["exec", "--json", "--skip-git-repo-check", "--output-last-message"])
        .arg(&answer_path).arg("-")
        .current_dir(&workdir.0).stdin(Stdio::piped())
        .stdout(Stdio::piped()).stderr(Stdio::piped());
    let (mut child, lifetime) = super::process::spawn(&mut command)
        .map_err(|error| ProbeAbort::Spawn(format!("无法安全启动 Codex CLI：{error}")))?;
    let noop: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(|_| {});
    let stdout = drain_pipe(child.stdout.take().expect("piped stdout"), Arc::clone(on_session_id));
    let stderr = drain_pipe(child.stderr.take().expect("piped stderr"), noop);
    let mut stdin = child.stdin.take().expect("piped stdin");
    let prompt = question.execution_prompt();
    // Stdin preserves multiline custom questions through Windows .cmd launchers.
    let input = std::thread::spawn(move || stdin.write_all(prompt.as_bytes()));
    let outcome = wait_for_exit(&mut child, cancel, progress.deadline(), check_config);
    progress.collecting();
    drop(lifetime);
    let stdout = collect_pipe(stdout);
    let stderr = collect_pipe(stderr);
    let input = input.join();
    let (exit, timed_out) = outcome?;
    check_config()?;
    if cancel.load(Ordering::SeqCst) { return Err(ProbeAbort::Cancelled); }
    if exit.success() && !timed_out && !matches!(input, Ok(Ok(()))) {
        return Err(ProbeAbort::Spawn("无法向 Codex 写入检测题目".to_string()));
    }
    let stdout = stdout.map_err(ProbeAbort::Spawn)?;
    let stderr = stderr.map_err(ProbeAbort::Spawn)?;
    let early_session = early_session_ids(&stdout).into_iter().next();
    let result = collect_result(question, &answer_path, &stdout, &stderr, exit, timed_out, started, early_session.as_deref());
    check_config()?;
    Ok(result)
}

/// Best-effort CLI version for the batch snapshot; a hanging or failing
/// `codex --version` never blocks a probe.
pub(super) fn cli_version() -> Option<String> {
    let mut command = Command::new(codex_executable());
    command.arg("--version").stdin(Stdio::null())
        .stdout(Stdio::piped()).stderr(Stdio::null());
    let (mut child, _lifetime) = super::process::spawn(&mut command).ok()?;
    let deadline = Instant::now() + VERSION_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            _ => {
                terminate_process_tree(&mut child);
                return None;
            }
        }
    };
    if !status.success() { return None; }
    let mut text = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_string(&mut text);
    }
    text.lines().map(str::trim).find(|line| !line.is_empty())
        .map(|line| line.chars().take(120).collect())
}

fn wait_for_exit(child: &mut Child, cancel: &AtomicBool, deadline: Instant,
    check_config: &dyn Fn() -> Result<(), ProbeAbort>) -> Result<(ExitStatus, bool), ProbeAbort> {
    loop {
        if let Err(error) = check_config() { terminate_process_tree(child); return Err(error); }
        if cancel.load(Ordering::SeqCst) {
            terminate_process_tree(child);
            return Err(ProbeAbort::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(exit)) => return Ok((exit, false)),
            Ok(None) => {}
            Err(_) => {
                terminate_process_tree(child);
                return Err(ProbeAbort::Spawn("无法监测 Codex 进程".to_string()));
            }
        }
        if Instant::now() >= deadline {
            terminate_process_tree(child);
            return child.wait().map(|exit| (exit, true))
                .map_err(|_| ProbeAbort::Spawn("无法收集 Codex 退出状态".to_string()));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn collect_result(
    question: &ResolvedQuestion, answer_path: &Path, stdout: &str, stderr: &str,
    exit: ExitStatus, timed_out: bool, started: Instant, early_session: Option<&str>,
) -> ProbeRunResult {
    let events = parse_events(stdout);
    let mut error = if timed_out {
        Some(format!("单次运行超时（上限 {} 分钟）",
            question.timeout_seconds / 60))
    } else if !exit.success() {
        Some(format!("Codex 进程退出码 {}：{}", exit.code().unwrap_or(-1), diagnostic(stderr)))
    } else {
        match &events {
            Err(message) => Some(message.clone()),
            Ok(events) if events.failure.is_some() => events.failure.clone(),
            Ok(events) if !events.completed => Some("Codex 未完成本次回答".to_string()),
            Ok(_) => None,
        }
    };
    let answer = std::fs::read_to_string(answer_path);
    if error.is_none() && answer.as_ref().map_or(true, |answer| answer.trim().is_empty()) {
        error = Some("Codex 未写入最终回答".to_string());
    }
    // Visual answers and execution failures never receive an automatic verdict.
    let outcome = if error.is_some() || question.grading != ProbeGrading::Integer {
        ProbeRunOutcome::Undetermined
    } else if answer.as_ref()
        .is_ok_and(|answer| answer_matches(answer, &question.expected_answer))
    {
        ProbeRunOutcome::Passed
    } else {
        ProbeRunOutcome::Failed
    };
    let final_answer = answer.ok().filter(|text| !text.trim().is_empty())
        .map(|text| match question.grading {
            ProbeGrading::Integer => text.trim().to_string(),
            ProbeGrading::SvgVisual | ProbeGrading::HtmlVisual => text,
        });
    let session_id = events.as_ref().ok().map(|events| events.session_id.clone())
        .or_else(|| early_session.map(str::to_string));
    let usage = events.as_ref().map_err(Clone::clone)
        .and_then(|events| read_usage(&events.session_id));
    let usage_error = match &usage {
        Err(message) => Some(message.clone()),
        Ok(summary) if summary.total.is_none() => Some("检测会话未提供总 token 消耗".to_string()),
        Ok(_) => None,
    };
    ProbeRunResult {
        outcome,
        final_answer,
        session_id,
        reasoning_tokens: usage.as_ref().ok().and_then(|summary| summary.reasoning_output),
        total_tokens: usage.as_ref().ok().and_then(|summary| summary.total),
        model: usage.as_ref().ok().and_then(|summary| summary.model.clone()),
        usage_error,
        duration_ms: started.elapsed().as_millis() as u64,
        error,
    }
}

pub(crate) fn codex_executable() -> PathBuf {
    if let Some(explicit) = std::env::var_os("ASB_TEST_CODEX_BIN") {
        return PathBuf::from(explicit);
    }
    if let Some(path) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&path) {
            for name in ["codex.exe", "codex.cmd", "codex.bat"] {
                let candidate = directory.join(name);
                if candidate.is_file() { return candidate; }
            }
        }
    }
    "codex".into()
}

fn drain_pipe<R: Read + Send + 'static>(
    mut pipe: R,
    on_session_id: Arc<dyn Fn(&str) + Send + Sync>,
) -> JoinHandle<std::io::Result<String>> {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut scanned = 0usize;
        let mut buffer = [0u8; 8192];
        loop {
            let read = pipe.read(&mut buffer)?;
            if read == 0 { break; }
            bytes.extend_from_slice(&buffer[..read]);
            // Report each session id as soon as its line is complete; strict
            // single-session validation still runs on the whole output.
            while let Some(newline) = bytes[scanned..].iter().position(|byte| *byte == b'\n') {
                let line = String::from_utf8_lossy(&bytes[scanned..scanned + newline]).into_owned();
                scanned += newline + 1;
                if let Some(id) = session_id_in_line(&line) {
                    on_session_id(&id);
                }
            }
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    })
}

/// The `thread.started` identity of one stdout line, when the line carries a
/// valid one. Best-effort only: it must never fail the run.
fn session_id_in_line(line: &str) -> Option<String> {
    let event: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if event.get("type").and_then(serde_json::Value::as_str) != Some("thread.started") {
        return None;
    }
    let id = event.get("thread_id").and_then(serde_json::Value::as_str)?;
    uuid::Uuid::parse_str(id).ok().map(|id| id.hyphenated().to_string())
}

fn early_session_ids(stdout: &str) -> Vec<String> {
    stdout.lines().filter_map(session_id_in_line).collect()
}

fn collect_pipe(reader: JoinHandle<std::io::Result<String>>) -> Result<String, String> {
    reader.join().map_err(|_| "Codex 输出读取线程已中断".to_string())?
        .map_err(|_| "无法读取 Codex 进程输出".to_string())
}

#[cfg(test)]
mod tests;
