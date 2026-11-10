mod store;

use super::{codex_client, SessionSource};
use asb_core::AppKind;
use std::{path::Path, time::Duration};

pub(super) fn apply(root: &Path, sources: &mut [SessionSource]) -> Result<(), String> {
    if !sources.iter().any(|source| source.meta.app == AppKind::Claude) { return Ok(()); }
    let archived = store::load(root)?;
    for source in sources.iter_mut().filter(|source| source.meta.app == AppKind::Claude) {
        source.archived = archived.contains(&source.meta.session_id);
    }
    Ok(())
}

pub(super) fn set(root: &Path, source: &SessionSource, sources: &[SessionSource], archived: bool) -> Result<Vec<String>, String> {
    if source.meta.app == AppKind::Claude {
        store::set(root, &source.meta.session_id, archived)?;
        return Ok(vec![source.meta.session_id.clone()]);
    }
    let id = uuid::Uuid::parse_str(&source.meta.session_id).map_err(|_| "Codex 归档需要有效的会话 UUID")?;
    let _lease = crate::client_software::gate::launch(AppKind::Codex)?;
    super::provider_transfer::ensure_settled(root).map_err(|error| error.to_string())?;
    let scope = codex_client::Scope::resolve(source)?;
    let mut command = codex_client::command(&scope, &id, if archived { "archive" } else { "unarchive" })?;
    crate::client_software::output(&mut command, Duration::from_secs(120))
        .map_err(|error| format!("Codex 归档操作未完成：{error}"))?;
    let (current_sources, issues) = super::scan_filtered_sources(root, Some(AppKind::Codex), None)?;
    let current = super::locate(&current_sources, &issues, AppKind::Codex, &source.meta.session_id)?;
    if current.archived != archived { return Err("Codex 命令结束后归档状态仍不匹配，请刷新后重试".into()); }
    Ok(super::codex_deletion::related_sources(source, sources).iter().filter(|member|
        current_sources.iter().any(|current| current.meta.session_id == member.meta.session_id && current.archived == archived))
        .map(|member| member.meta.session_id.clone()).collect())
}

pub(super) fn forget(root: &Path, app: AppKind, ids: &[String]) -> Result<(), String> {
    if app == AppKind::Claude { store::remove(root, ids)?; }
    Ok(())
}
