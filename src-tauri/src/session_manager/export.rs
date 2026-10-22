use super::{organization, resolve_session, SessionMessage, SessionMeta};
use asb_core::contracts::AppKind;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

struct ExportFile(PathBuf);

impl Drop for ExportFile {
    fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
}

pub fn export_markdown(root: &Path, app: AppKind, session_id: &str, target: &Path) -> Result<(), String> {
    let mut source = resolve_session(root, app, session_id)?;
    validate_target(root, &source, target)?;
    organization::apply(&mut source.meta, &organization::load(root)?);
    let parent = target.parent().ok_or("导出位置无效")?;
    let temporary = ExportFile(parent.join(format!(".asb-export-{}.tmp", uuid::Uuid::new_v4())));
    let mut file = File::create_new(&temporary.0).map_err(export_error)?;
    {
        let mut writer = BufWriter::new(&mut file);
        write_header(&mut writer, &source.meta).map_err(export_error)?;
        let issues = source.visit(|mut message| {
            let (images, issues) = source.resolve_images(&message.images);
            if !issues.is_empty() { return Err(issues.join("；")); }
            message.images = images;
            write_message(&mut writer, &message).map_err(export_error)?;
            Ok(ControlFlow::Continue(()))
        })?;
        if !issues.is_empty() {
            writeln!(writer, "\n---\n\n## 读取说明\n\n{}", issues.join("\n\n")).map_err(export_error)?;
        }
        writer.flush().map_err(export_error)?;
    }
    file.sync_all().map_err(export_error)?;
    drop(file);
    crate::storage::publish::replace(&temporary.0, target)
}

fn validate_target(root: &Path, source: &super::SessionSource, target: &Path) -> Result<(), String> {
    let exists = target.try_exists().map_err(|error| error.to_string())?;
    let resolved = if exists { target.canonicalize().map_err(|error| error.to_string())? }
        else { target.parent().ok_or("导出位置无效")?.canonicalize().map_err(|error| error.to_string())?
            .join(target.file_name().ok_or("导出文件名无效")?) };
    if resolved.starts_with(root.canonicalize().map_err(|error| error.to_string())?) {
        return Err("导出目标不能位于应用状态目录，请选择其他保存位置".into());
    }
    let target = resolved;
    let mut originals = source.paths.clone();
    if source.meta.app == AppKind::Codex {
        let directory = crate::local_state::codex_paths::sqlite_root(&super::codex_root()?)?;
        originals.extend([directory.join("state_5.sqlite"), directory.join("thread_history_1.sqlite")]);
    }
    for original in originals {
        let resolved = if original.try_exists().map_err(|error| error.to_string())? {
            Some(original.canonicalize().map_err(|error| error.to_string())?)
        } else if let Some(parent) = original.parent().filter(|parent| parent.is_dir()) {
            Some(parent.canonicalize().map_err(|error| error.to_string())?.join(original.file_name().ok_or("来源文件名无效")?))
        } else { None };
        if resolved.as_ref() == Some(&target) {
            return Err("导出目标不能覆盖原会话记录或原生数据库".into());
        }
    }
    Ok(())
}

fn write_header(writer: &mut impl Write, meta: &SessionMeta) -> std::io::Result<()> {
    writeln!(writer, "# {}\n\n- 来源：{}\n- 会话 ID：{}",
        escape(meta.alias.as_deref().unwrap_or(&meta.title)), meta.app.label(), escape(&meta.session_id))?;
    for (label, value) in [
        ("项目", meta.project_dir.as_deref()),
        ("创建时间", meta.created_at.as_deref()),
        ("最后活动", meta.last_active_at.as_deref()),
    ] {
        if let Some(value) = value { writeln!(writer, "- {label}：{}", escape(value))?; }
    }
    Ok(())
}

fn write_message(writer: &mut impl Write, message: &SessionMessage) -> std::io::Result<()> {
    write!(writer, "\n---\n\n## {}", escape(&message.role))?;
    if let Some(at) = &message.at { write!(writer, " · {}", escape(at))?; }
    let content = super::images::copy_text(message).map_err(std::io::Error::other)?;
    writeln!(writer, "\n\n{content}")
}

fn export_error(error: std::io::Error) -> String { format!("导出文件写入失败：{error}") }

fn escape(text: &str) -> String {
    text.chars().flat_map(|character| {
        if "\\`*_{}[]<>()#+-.!|".contains(character) { vec!['\\', character] }
        else if character == '\n' || character == '\r' { vec![' '] }
        else { vec![character] }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_complete_markdown_content_and_escapes_metadata() {
        let meta = SessionMeta { app: AppKind::Codex, session_id: "s-1".into(),
            title: "[title]".into(), summary: String::new(), project_dir: None,
            created_at: Some("2026-09-22T00:00:00Z".into()), last_active_at: None,
            resume_command: None, alias: None, pinned: false, tags: Vec::new(), parent_session_id: None, can_delete: true, preservation: None };
        let content = "```rust\nlet x = 1;\n```\n\n完整正文";
        let mut output = Vec::new();
        write_header(&mut output, &meta).unwrap();
        write_message(&mut output, &SessionMessage { id: "id".into(),
            role: "assistant".into(), content: content.into(), at: None, images: Vec::new() }).unwrap();
        let markdown = String::from_utf8(output).unwrap();
        assert!(markdown.starts_with("# \\[title\\]"));
        assert!(markdown.contains(content));
        assert!(markdown.contains("来源：Codex"));
        assert!(markdown.contains("创建时间：2026"));
    }

    #[test]
    fn export_cannot_replace_its_application_owned_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("state");
        fs::create_dir_all(root.join("sessions")).unwrap();
        let target = root.join("sessions/organization.sqlite3");
        fs::write(&target, b"retained metadata").unwrap();
        let source = crate::session_manager::fixture_source(&temp.path().join("transcript.jsonl"), AppKind::Claude, "source");
        assert!(validate_target(&root, &source, &target).is_err());
        assert_eq!(fs::read(target).unwrap(), b"retained metadata");
    }
}
