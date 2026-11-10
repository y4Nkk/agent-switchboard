use super::{native, organization, SessionDeleteOutcome, SessionSource};
use asb_core::contracts::AppKind;
use rusqlite::{Connection, OptionalExtension};
use std::{collections::HashSet, path::{Path, PathBuf}, process::Command, time::Duration};

struct Scope { home: PathBuf, database: PathBuf }

impl Scope {
    fn resolve(source: &SessionSource) -> Result<Self, String> {
        let home = super::codex_root()?.canonicalize().map_err(|error| error.to_string())?;
        let database = crate::local_state::codex_paths::sqlite_root(&home)?.join("state_5.sqlite");
        if let Some(observed) = &source.native_database {
            if observed.canonicalize().map_err(|error| error.to_string())?
                != database.canonicalize().map_err(|error| error.to_string())? {
                return Err("Codex 原生登记位置已变化，请刷新后重试".into());
            }
        } else {
            let roots = crate::local_state::codex_paths::session_roots(&home);
            if source.paths.is_empty() { return Err("Codex 会话缺少可核实的原始位置".into()); }
            for path in &source.paths {
                let path = path.canonicalize().map_err(|error| error.to_string())?;
                if !roots.iter().any(|root| path.starts_with(root)) {
                    return Err("Codex 会话原始位置已变化，请刷新后重试".into());
                }
            }
        }
        Ok(Self { home, database })
    }
}

pub(super) fn remove(state: &Path, source: &SessionSource, sources: &[SessionSource]) -> Result<Vec<SessionDeleteOutcome>, String> {
    let id = uuid::Uuid::parse_str(&source.meta.session_id).map_err(|_| "Codex 原生删除需要有效的会话 UUID")?;
    let _lease = crate::client_software::gate::launch(AppKind::Codex)?;
    super::provider_transfer::ensure_settled(state).map_err(|error| error.to_string())?;
    let scope = Scope::resolve(source)?;
    let mut command = command(&scope, &id)?;
    let error = crate::client_software::output(&mut command, Duration::from_secs(120)).err()
        .map(|error| format!("Codex 原生删除未完成，请确认本机 CLI 支持 delete 并处理以下错误：{error}"));
    complete(state, source, sources, &scope, error)
}

fn command(scope: &Scope, id: &uuid::Uuid) -> Result<Command, String> {
    let sqlite = scope.database.parent().ok_or("Codex 原生登记目录无效")?;
    let home = crate::client_software::command_path(&scope.home);
    let sqlite = crate::client_software::command_path(sqlite);
    let sqlite_text = sqlite.to_str().ok_or("Codex 原生登记目录无法传递给客户端")?;
    let installation = crate::client_software::discovery::resolve(AppKind::Codex)?;
    let mut command = crate::client_software::discovery::launch::command(&installation)?;
    command.args(["delete", "--force", &id.to_string(), "-c", &format!("sqlite_home={}", toml_edit::Value::from(sqlite_text))])
        .env("CODEX_HOME", &home).env("CODEX_SQLITE_HOME", &sqlite).current_dir(home);
    for key in ["CODEX_EXEC_SERVER_URL", "CODEX_EXEC_SERVER_NOISE_REGISTRY_URL",
        "CODEX_EXEC_SERVER_NOISE_ENVIRONMENT_ID", "CODEX_EXEC_SERVER_NOISE_CHATGPT_ACCOUNT_ID",
        "CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN"] {
        command.env_remove(key);
    }
    Ok(command)
}

struct Verification { state: Option<Connection>, history: Option<Connection> }

impl Verification {
    fn open(scope: &Scope, source: &SessionSource) -> Result<Self, String> {
        Ok(Self { state: open_existing(&scope.database, source.native_database.is_some())?,
            history: open_existing(&scope.database.with_file_name("thread_history_1.sqlite"), source.native_history.is_some())? })
    }

    fn removed(&self, source: &SessionSource) -> Result<bool, String> {
        let id = &source.meta.session_id;
        if let Some(state) = &self.state {
            let registered = state.query_row("SELECT 1 FROM threads WHERE id=?1", [id], |row| row.get::<_, i32>(0))
                .optional().map_err(|error| format!("无法核验 Codex 会话 {id} 的登记：{error}"))?.is_some();
            if registered { return Ok(false); }
        }
        if let Some(history) = &self.history {
            for table in ["thread_turns", "thread_items", "thread_realtime_items", "thread_history_projection_state"] {
                let remaining: bool = history.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE thread_id=?1)"),
                    [id], |row| row.get(0)).map_err(|error| format!("无法核验 Codex 会话 {id} 的 {table}：{error}"))?;
                if remaining { return Ok(false); }
            }
        }
        for path in &source.paths {
            if path.try_exists().map_err(|error| format!("无法核验会话文件 {}：{error}", path.display()))? { return Ok(false); }
        }
        Ok(true)
    }
}

fn open_existing(path: &Path, expected: bool) -> Result<Option<Connection>, String> {
    if !path.try_exists().map_err(|error| format!("无法检查 Codex 历史库 {}：{error}", path.display()))? {
        return if expected { Err(format!("Codex 历史库已消失，无法核验删除结果：{}", path.display())) } else { Ok(None) };
    }
    let connection = native::open(path)?;
    connection.execute_batch("BEGIN DEFERRED").map_err(|error| error.to_string())?;
    Ok(Some(connection))
}

fn complete(
    state: &Path, source: &SessionSource, sources: &[SessionSource], scope: &Scope, operation_error: Option<String>,
) -> Result<Vec<SessionDeleteOutcome>, String> {
    let verification = Verification::open(scope, source).map_err(|error| join_error(operation_error.clone(), error))?;
    let mut outcomes = Vec::new();
    for candidate in related_sources(source, sources) {
        let requested = candidate.meta.session_id == source.meta.session_id;
        let mut error = if requested { operation_error.clone() } else { None };
        let deleted = match verification.removed(candidate) {
            Ok(false) if !requested => continue,
            Ok(deleted) => {
                if !deleted && error.is_none() { error = Some("Codex 删除命令已结束，但登记、正文或原始文件仍存在；请刷新后重试".into()); }
                deleted
            }
            Err(cause) => { error = Some(join_error(error, cause)); false }
        };
        outcomes.push(SessionDeleteOutcome { app: AppKind::Codex, session_id: candidate.meta.session_id.clone(), deleted, error });
    }
    let removed: Vec<_> = outcomes.iter().filter(|outcome| outcome.deleted).map(|outcome| outcome.session_id.clone()).collect();
    if !removed.is_empty() {
        if let Err(error) = organization::delete_with_sources(state, AppKind::Codex, &removed, || Ok(())) {
            for outcome in outcomes.iter_mut().filter(|outcome| outcome.deleted) {
                outcome.error = Some(join_error(outcome.error.take(), format!("会话已删除，本地整理数据清理未完成：{error}")));
            }
        }
    }
    Ok(outcomes)
}

fn join_error(previous: Option<String>, detail: String) -> String {
    match previous { Some(previous) => format!("{previous}；{detail}"), None => detail }
}

fn related_sources<'a>(source: &'a SessionSource, sources: &'a [SessionSource]) -> Vec<&'a SessionSource> {
    let mut related = vec![source];
    let mut seen = HashSet::from([source.meta.session_id.clone()]);
    let mut position = 0;
    while position < related.len() {
        for candidate in sources.iter().filter(|candidate| candidate.meta.app == AppKind::Codex) {
            if candidate.meta.parent_session_id.as_deref() == Some(related[position].meta.session_id.as_str())
                && seen.insert(candidate.meta.session_id.clone()) {
                related.push(candidate);
            }
        }
        position += 1;
    }
    related
}

#[cfg(test)]
mod tests;
