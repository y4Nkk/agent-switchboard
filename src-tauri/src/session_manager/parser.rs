use super::{content, SessionMessage, SessionMeta, TITLE_LIMIT};
use asb_core::contracts::AppKind;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs::File, io::{self, BufRead, BufReader, Read}};
use std::ops::ControlFlow;
use std::path::Path;

const SUMMARY_LIMIT: usize = 180;

pub(super) fn parse_session(app: AppKind, path: &Path) -> Result<(SessionMeta, Vec<String>), io::Error> {
    parse_session_observed(app, path, &|| Ok(()))
}

pub(super) fn parse_session_observed(app: AppKind, path: &Path, checkpoint: &dyn Fn() -> Result<(), String>) -> Result<(SessionMeta, Vec<String>), io::Error> {
    checkpoint().map_err(io::Error::other)?;
    let file = File::open(path)?;
    let length = file.metadata()?.len();
    parse_session_reader(app, path, BufReader::new(file.take(length)), checkpoint)
}

pub(super) fn parse_session_reader(app: AppKind, path: &Path, reader: impl BufRead,
    checkpoint: &dyn Fn() -> Result<(), String>) -> Result<(SessionMeta, Vec<String>), io::Error> {
    let mut session_id = None;
    let mut project_dir = None;
    let mut created_at = None;
    let mut native_created_at = None;
    let mut parent_session_id = None;
    let mut last_active_at = None;
    let mut custom_title = None;
    let mut first_prompt = None;
    let scan = super::records::scan_reader(&path.display().to_string(), 0, 0, reader, checkpoint, |_, _, value| {
        session_id = session_id.take().or_else(|| session_id_from(&value).filter(|id| valid_session_id(id)));
        if value.get("type").and_then(Value::as_str) == Some("session_meta") {
            native_created_at = native_created_at.take().or_else(|| timestamp_from(&value));
            parent_session_id = parent_session_id.take().or_else(|| parent_from(value.get("payload").unwrap_or(&value)));
        }
        project_dir = project_dir.take().or_else(|| project_dir_from(&value));
        if let Some(at) = timestamp_from(&value) {
            if created_at.as_ref().is_none_or(|current| &at < current) { created_at = Some(at.clone()); }
            if last_active_at.as_ref().is_none_or(|current| &at > current) { last_active_at = Some(at); }
        }
        if let Some(title) = value.get("customTitle").or_else(|| value.get("custom_title"))
            .and_then(Value::as_str).filter(|text| !text.trim().is_empty()) {
            custom_title = Some(title.to_string());
        }
        if first_prompt.is_none() {
            first_prompt = message_from(&value).filter(|message| message.role == "user")
                .and_then(|message| prompt(&message.content)).map(|text| clamp_text(&text, SUMMARY_LIMIT));
        }
        Ok(ControlFlow::Continue(()))
    });
    checkpoint().map_err(io::Error::other)?;
    let issues = match scan { Ok(scan) => scan.issues, Err(error) => vec![error] };
    let session_id = session_id.filter(|id| valid_session_id(id))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData,
            if issues.is_empty() { "missing session id".into() } else { issues.join("；") }))?;
    let title = custom_title.or_else(|| first_prompt.clone())
        .or_else(|| project_dir.as_deref().and_then(project_name)).unwrap_or_else(|| session_id.clone());
    Ok((SessionMeta { app, resume_command: Some(super::resume::resume_command(app, &session_id)),
        session_id, title: clamp_title(&title), summary: first_prompt.unwrap_or_else(|| clamp_text(&title, SUMMARY_LIMIT)),
        project_dir, created_at: native_created_at.or(created_at), last_active_at, alias: None, pinned: false, tags: Vec::new(),
        parent_session_id, can_delete: true, preservation: None }, issues))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct SessionIdentity {
    pub id: String,
    pub parent: Option<String>,
    pub created_at: Option<String>,
}

pub(crate) fn session_identity(path: &Path) -> io::Result<Option<SessionIdentity>> {
    session_identity_observed(path, &|| Ok(()))
}

pub(super) fn session_identity_observed(path: &Path, checkpoint: &dyn Fn() -> Result<(), String>) -> io::Result<Option<SessionIdentity>> {
    checkpoint().map_err(io::Error::other)?;
    let file = File::open(path)?;
    let length = file.metadata()?.len();
    session_identity_reader(path, BufReader::new(file.take(length)), checkpoint)
}

pub(super) fn session_identity_reader(path: &Path, reader: impl BufRead,
    checkpoint: &dyn Fn() -> Result<(), String>) -> io::Result<Option<SessionIdentity>> {
    let mut found = None;
    super::records::scan_reader(&path.display().to_string(), 0, 0, reader, checkpoint, |_, _, value| {
        found = session_id_from(&value).filter(|id| valid_session_id(id)).map(|id| SessionIdentity {
            id, parent: parent_from(value.get("payload").unwrap_or(&value)), created_at: timestamp_from(&value) });
        Ok(if found.is_some() { ControlFlow::Break(()) } else { ControlFlow::Continue(()) })
    }).map_err(io::Error::other)?;
    Ok(found)
}

pub(crate) fn parent_from(payload: &Value) -> Option<String> {
    let parent = |value: Option<&Value>| value.and_then(Value::as_str).filter(|id| valid_session_id(id)).map(str::to_string);
    parent(payload.get("parent_thread_id")).or_else(|| parent(payload.get("forked_from_id")))
        .or_else(|| parent(payload.pointer("/source/subagent/thread_spawn/parent_thread_id")))
}

#[cfg(test)]
pub(super) fn collect_messages(path: &Path) -> Result<Vec<SessionMessage>, String> {
    let mut messages = Vec::new();
    let source = super::fixture_source(path, AppKind::Codex, "fixture");
    source.visit(|message| { messages.push(message); Ok(ControlFlow::Continue(())) })?;
    Ok(messages)
}

pub(super) fn message_from(value: &Value) -> Option<SessionMessage> {
    let (role, content) = content::record(value)?;
    Some(SessionMessage { id: String::new(), role, content, at: timestamp_from(value), images: super::images::references(value) })
}

pub(super) fn record_identity(value: &Value) -> String {
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}

pub(super) fn claude_record_id(value: &Value) -> Option<String> {
    let session = value.get("sessionId")?.as_str()?;
    let uuid = value.get("uuid")?.as_str()?;
    if !valid_session_id(session) || uuid::Uuid::parse_str(uuid).is_err() { return None; }
    Some(format!("claude-{}-{uuid}", value.get("type")?.as_str()?))
}

pub(super) fn native_identity(value: &Value) -> String {
    let id = value.get("id").and_then(Value::as_str).unwrap_or("unknown");
    format!("native-{id}-{}", record_identity(value))
}

fn session_id_from(value: &Value) -> Option<String> {
    let id = if value.get("type").and_then(Value::as_str) == Some("session_meta") {
        value.pointer("/payload/id")
    } else { value.get("sessionId").or_else(|| value.get("session_id")) };
    id.and_then(Value::as_str).map(str::to_string)
}

fn project_dir_from(value: &Value) -> Option<String> {
    value.get("cwd").or_else(|| value.get("payload")?.get("cwd"))
        .and_then(Value::as_str).filter(|path| !path.is_empty()).map(str::to_string)
}

pub(crate) fn timestamp_from(value: &Value) -> Option<String> {
    value.get("timestamp").or_else(|| value.get("payload")?.get("timestamp"))
        .and_then(Value::as_str).and_then(|text| chrono::DateTime::parse_from_rfc3339(text).ok())
        .map(|at| at.with_timezone(&chrono::Utc).to_rfc3339())
}

pub(super) fn valid_session_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128
        && id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

pub(super) fn clamp_title(text: &str) -> String { clamp_text(text, TITLE_LIMIT) }

pub(super) fn clamp_text(text: &str, limit: usize) -> String {
    let mut characters = super::text::compact_chars(text);
    let mut result = characters.by_ref().take(limit).collect::<String>();
    if characters.next().is_some() { result.push('…'); }
    result
}

pub(super) fn prompt(content: &str) -> Option<String> {
    let text = content.trim();
    if text.starts_with("# AGENTS.md instructions for ") || text.starts_with("<environment_context>")
        || text.starts_with("<local-command-caveat>") || text.starts_with("<command-name>") { return None; }
    if text.starts_with("# Context from my IDE setup:") {
        let (index, heading) = text.lines().enumerate().filter(|(_, line)|
            line.trim_start_matches('#').trim().to_lowercase().starts_with("my request for codex"))
            .last()?;
        let inline = heading.split_once(':').map(|(_, request)| request.trim()).unwrap_or("");
        let following = text.lines().skip(index + 1).collect::<Vec<_>>().join("\n");
        let request = [inline, following.trim()].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join("\n");
        return (!request.is_empty()).then_some(request);
    }
    (!text.is_empty()).then(|| text.to_string())
}

fn project_name(path: &str) -> Option<String> {
    Path::new(path).file_name().and_then(|name| name.to_str()).map(str::to_string)
}
