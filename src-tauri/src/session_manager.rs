//! Read-only local session discovery for the two clients Agent Switchboard owns.
//!
//! The scanner never indexes, modifies, deletes, or launches a client session.
//! It reads only the JSONL records that Codex and Claude Code already maintain.

use asb_core::contracts::AppKind;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const TITLE_LIMIT: usize = 120;
const SUMMARY_LIMIT: usize = 180;
/// Every metadata field below is first-match-wins, so once all five are
/// collected the remaining lines cannot change the result. This cap bounds
/// the per-file scan cost when some fields never appear; records past the cap
/// are ignored and the title falls back as if they were absent.
const METADATA_LINE_LIMIT: usize = 32;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub app: AppKind,
    pub session_id: String,
    pub title: String,
    pub summary: String,
    pub project_dir: Option<String>,
    pub created_at: Option<String>,
    pub last_active_at: Option<String>,
    pub resume_command: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    pub at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIssue {
    pub app: AppKind,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionScan {
    pub sessions: Vec<SessionMeta>,
    pub issues: Vec<SessionIssue>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResume {
    pub command: String,
    pub used_project_dir: bool,
}

#[derive(Debug, Clone)]
struct SessionSource {
    meta: SessionMeta,
    path: PathBuf,
}

pub fn scan_sessions() -> SessionScan {
    let home = match std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()) {
        Some(home) => PathBuf::from(home),
        None => {
            return SessionScan {
                sessions: Vec::new(),
                issues: vec![SessionIssue {
                    app: AppKind::Codex,
                    message: "无法确定 Windows 用户目录".to_string(),
                }],
            }
        }
    };
    scan_session_roots(&[
        (AppKind::Codex, home.join(".codex").join("sessions")),
        (
            AppKind::Codex,
            home.join(".codex").join("archived_sessions"),
        ),
        (AppKind::Claude, home.join(".claude").join("projects")),
    ])
}

pub fn load_messages(app: AppKind, session_id: &str) -> Result<Vec<SessionMessage>, String> {
    let source = resolve_session(app, session_id)?;
    read_messages(&source.path)
}

/// Opens the selected local session in a new Windows Command Prompt window.
/// The renderer supplies only the supported client plus a validated session id;
/// the source path and working directory stay resolved inside this module.
pub fn resume_session(app: AppKind, session_id: &str) -> Result<SessionResume, String> {
    let source = resolve_session(app, session_id)?;
    let project_dir = source
        .meta
        .project_dir
        .as_deref()
        .filter(|path| Path::new(path).is_dir());
    let mut terminal = Command::new("cmd.exe");
    terminal
        .args(["/d", "/k"])
        .args(resume_arguments(app, session_id))
        .creation_flags(CREATE_NEW_CONSOLE);
    if let Some(project_dir) = project_dir {
        terminal.current_dir(project_dir);
    }
    terminal
        .spawn()
        .map_err(|error| format!("无法启动命令提示符：{error}"))?;

    Ok(SessionResume {
        command: resume_command(app, session_id),
        used_project_dir: project_dir.is_some(),
    })
}

const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

fn resolve_session(app: AppKind, session_id: &str) -> Result<SessionSource, String> {
    if !valid_session_id(session_id) {
        return Err("会话 ID 无效".to_string());
    }
    let (sources, issues) = scan_sources()?;
    sources
        .into_iter()
        .find(|source| source.meta.app == app && source.meta.session_id == session_id)
        .ok_or_else(|| {
            issues
                .first()
                .map(|issue| issue.message.clone())
                .unwrap_or_else(|| "找不到指定会话；请刷新会话列表后重试".to_string())
        })
}

fn scan_sources() -> Result<(Vec<SessionSource>, Vec<SessionIssue>), String> {
    let home = std::env::var_os("USERPROFILE")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "无法确定 Windows 用户目录".to_string())?;
    Ok(scan_session_source_roots(&[
        (AppKind::Codex, home.join(".codex").join("sessions")),
        (
            AppKind::Codex,
            home.join(".codex").join("archived_sessions"),
        ),
        (AppKind::Claude, home.join(".claude").join("projects")),
    ]))
}

fn scan_session_roots(roots: &[(AppKind, PathBuf)]) -> SessionScan {
    let (mut sources, issues) = scan_session_source_roots(roots);
    sources.sort_by(|left, right| {
        right
            .meta
            .last_active_at
            .as_deref()
            .unwrap_or("")
            .cmp(left.meta.last_active_at.as_deref().unwrap_or(""))
    });
    SessionScan {
        sessions: sources.into_iter().map(|source| source.meta).collect(),
        issues,
    }
}

fn scan_session_source_roots(
    roots: &[(AppKind, PathBuf)],
) -> (Vec<SessionSource>, Vec<SessionIssue>) {
    let mut sources = Vec::new();
    let mut issues = Vec::new();

    for (app, root) in roots {
        match collect_jsonl_files(root) {
            Ok(paths) => {
                for path in paths {
                    if let Ok(meta) = parse_session(*app, &path) {
                        sources.push(SessionSource { meta, path });
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => issues.push(SessionIssue {
                app: *app,
                message: format!("无法读取{}会话目录", client_label(*app)),
            }),
        }
    }

    (sources, issues)
}

fn collect_jsonl_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut pending = vec![root.to_path_buf()];

    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
            {
                paths.push(entry.path());
            }
        }
    }

    Ok(paths)
}

fn parse_session(app: AppKind, path: &Path) -> Result<SessionMeta, io::Error> {
    let mut session_id = None;
    let mut project_dir = None;
    let mut created_at = None;
    let mut custom_title = None;
    let mut first_user_message = None;

    let reader = BufReader::new(File::open(path)?);
    for value in reader
        .lines()
        .filter_map(Result::ok)
        .take(METADATA_LINE_LIMIT)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
    {
        session_id = session_id.or_else(|| session_id_from(&value));
        project_dir = project_dir.or_else(|| project_dir_from(&value));
        created_at = created_at.or_else(|| timestamp_from(&value));
        custom_title = custom_title.or_else(|| custom_title_from(&value));
        if first_user_message.is_none() {
            first_user_message = message_from(&value)
                .filter(|message| message.role == "user")
                .map(|message| message.content)
                .filter(|content| useful_user_content(content));
        }
        if session_id.is_some()
            && project_dir.is_some()
            && created_at.is_some()
            && custom_title.is_some()
            && first_user_message.is_some()
        {
            break;
        }
    }

    let session_id = session_id
        .filter(|id| valid_session_id(id))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing session id"))?;
    let title = custom_title
        .or_else(|| first_user_message.clone())
        .or_else(|| project_dir.as_deref().and_then(project_name))
        .unwrap_or_else(|| session_id.clone());
    let last_active_at = fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .map(system_time_iso);

    Ok(SessionMeta {
        app,
        resume_command: resume_command(app, &session_id),
        session_id,
        title: clamp_text(&title, TITLE_LIMIT),
        summary: clamp_text(
            first_user_message.as_deref().unwrap_or(&title),
            SUMMARY_LIMIT,
        ),
        project_dir,
        created_at,
        last_active_at,
    })
}

fn read_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let file = File::open(path).map_err(|_| "无法读取会话记录".to_string())?;
    let reader = BufReader::new(file);
    Ok(reader
        .lines()
        .filter_map(Result::ok)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
        .filter_map(|value| message_from(&value))
        .collect())
}

fn message_from(value: &Value) -> Option<SessionMessage> {
    let envelope = if value.get("type").and_then(Value::as_str) == Some("response_item") {
        value.get("payload").unwrap_or(value)
    } else {
        value
    };
    let message = envelope.get("message").unwrap_or(envelope);
    let role = message
        .get("role")
        .or_else(|| envelope.get("role"))
        .and_then(Value::as_str)
        .or_else(|| match envelope.get("type").and_then(Value::as_str) {
            Some("user") => Some("user"),
            Some("assistant") => Some("assistant"),
            Some("system") => Some("system"),
            _ => None,
        })?;
    let content = message
        .get("content")
        .or_else(|| envelope.get("content"))
        .and_then(content_text)?;
    if content.trim().is_empty() {
        return None;
    }
    Some(SessionMessage {
        role: role.to_string(),
        content,
        at: timestamp_from(value),
    })
}

fn content_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(content_text)
                .collect::<Vec<_>>()
                .join("\n");
            (!text.is_empty()).then_some(text)
        }
        Value::Object(object) => object
            .get("text")
            .or_else(|| object.get("content"))
            .or_else(|| object.get("value"))
            .and_then(content_text),
        _ => None,
    }
}

fn session_id_from(value: &Value) -> Option<String> {
    let payload = value.get("payload").unwrap_or(value);
    value
        .get("sessionId")
        .or_else(|| value.get("session_id"))
        .or_else(|| payload.get("id"))
        .or_else(|| payload.get("sessionId"))
        .or_else(|| payload.get("session_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn project_dir_from(value: &Value) -> Option<String> {
    let payload = value.get("payload").unwrap_or(value);
    value
        .get("cwd")
        .or_else(|| payload.get("cwd"))
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
        .map(str::to_string)
}

fn timestamp_from(value: &Value) -> Option<String> {
    let payload = value.get("payload").unwrap_or(value);
    value
        .get("timestamp")
        .or_else(|| payload.get("timestamp"))
        .and_then(Value::as_str)
        .filter(|timestamp| !timestamp.is_empty())
        .map(str::to_string)
}

fn custom_title_from(value: &Value) -> Option<String> {
    value
        .get("customTitle")
        .or_else(|| value.get("custom_title"))
        .and_then(Value::as_str)
        .filter(|title| !title.trim().is_empty())
        .map(str::to_string)
}

fn valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn resume_command(app: AppKind, session_id: &str) -> String {
    resume_arguments(app, session_id).join(" ")
}

fn resume_arguments(app: AppKind, session_id: &str) -> Vec<&str> {
    match app {
        AppKind::Codex => vec!["codex", "resume", session_id],
        AppKind::Claude => vec!["claude", "--resume", session_id],
    }
}

fn client_label(app: AppKind) -> &'static str {
    match app {
        AppKind::Codex => "Codex",
        AppKind::Claude => "Claude Code",
    }
}

fn clamp_text(text: &str, limit: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= limit {
        return text;
    }
    let mut shortened = text.chars().take(limit).collect::<String>();
    shortened.push('…');
    shortened
}

fn useful_user_content(content: &str) -> bool {
    !content.starts_with("<local-command-caveat>") && !content.starts_with("<command-name>")
}

fn project_name(path: &str) -> Option<String> {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

fn system_time_iso(time: std::time::SystemTime) -> String {
    DateTime::<Utc>::from(time).to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, content).expect("write session");
    }

    #[test]
    fn scans_supported_sources_without_exposing_paths() {
        let temp = tempdir().expect("temp");
        let codex = temp.path().join("codex").join("a.jsonl");
        let claude = temp.path().join("claude").join("b.jsonl");
        write(
            &codex,
            concat!(
                "{\"type\":\"session_meta\",\"payload\":{\"id\":\"codex-123\",\"cwd\":\"C:/work/relay\",\"timestamp\":\"2026-08-01T10:00:00Z\"}}\n",
                "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":\"修复会话页面\"}}\n"
            ),
        );
        write(
            &claude,
            concat!(
                "{\"type\":\"user\",\"sessionId\":\"claude-456\",\"cwd\":\"C:/work/claude\",\"timestamp\":\"2026-08-02T10:00:00Z\",\"message\":{\"role\":\"user\",\"content\":\"整理历史\"}}\n",
                "{\"type\":\"custom-title\",\"customTitle\":\"归档整理\"}\n"
            ),
        );

        let scan = scan_session_roots(&[
            (AppKind::Codex, temp.path().join("codex")),
            (AppKind::Claude, temp.path().join("claude")),
        ]);

        assert!(scan.issues.is_empty());
        assert_eq!(scan.sessions.len(), 2);
        let claude = scan
            .sessions
            .iter()
            .find(|session| session.app == AppKind::Claude)
            .expect("Claude session");
        let codex = scan
            .sessions
            .iter()
            .find(|session| session.app == AppKind::Codex)
            .expect("Codex session");
        assert_eq!(claude.title, "归档整理");
        assert_eq!(claude.resume_command, "claude --resume claude-456");
        assert_eq!(codex.resume_command, "codex resume codex-123");
    }

    #[test]
    fn extracts_messages_only_when_a_record_has_a_role_and_content() {
        let temp = tempdir().expect("temp");
        let path = temp.path().join("session.jsonl");
        write(
            &path,
            concat!(
                "{\"type\":\"session_meta\",\"payload\":{\"id\":\"codex-123\"}}\n",
                "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"你好\"}]}}\n",
                "{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"已完成\"}]}}\n",
                "{\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\",\"name\":\"shell\"}}\n"
            ),
        );

        let messages = read_messages(&path).expect("read messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "你好");
        assert_eq!(messages[1].role, "assistant");
    }

    #[test]
    fn invalid_session_ids_are_not_made_resumable() {
        assert!(!valid_session_id("session; shutdown"));
        assert!(!valid_session_id(""));
        assert!(valid_session_id("019f4b74-c859-7e72-bb0c-9f83347954fb"));
    }

    #[test]
    fn metadata_records_beyond_the_line_cap_are_ignored() {
        let temp = tempdir().expect("temp");
        let mut content = String::new();
        for _ in 0..(METADATA_LINE_LIMIT + 1) {
            content.push_str(
                "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"噪声\"}}\n",
            );
        }
        content.push_str(
            "{\"sessionId\":\"late-1\",\"cwd\":\"C:/work/late\",\"message\":{\"role\":\"user\",\"content\":\"很晚才出现\"}}\n",
        );
        write(&temp.path().join("late.jsonl"), &content);

        let scan = scan_session_roots(&[(AppKind::Claude, temp.path().to_path_buf())]);
        assert!(
            scan.sessions.is_empty(),
            "会话 ID 在限界之后才出现时不应被收录"
        );
    }

    #[test]
    fn resume_arguments_keep_the_client_command_and_id_separate() {
        assert_eq!(
            resume_arguments(AppKind::Codex, "codex-123"),
            ["codex", "resume", "codex-123"]
        );
        assert_eq!(
            resume_arguments(AppKind::Claude, "claude-456"),
            ["claude", "--resume", "claude-456"]
        );
    }
}
