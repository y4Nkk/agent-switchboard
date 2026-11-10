use super::SessionSource;
use asb_core::AppKind;
use std::{path::PathBuf, process::Command};

pub(super) struct Scope { pub home: PathBuf, pub database: PathBuf }

impl Scope {
    pub fn resolve(source: &SessionSource) -> Result<Self, String> {
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

pub(super) fn command(scope: &Scope, id: &uuid::Uuid, action: &str) -> Result<Command, String> {
    if !matches!(action, "archive" | "unarchive" | "delete") { return Err("无效的 Codex 会话操作".into()); }
    let sqlite = scope.database.parent().ok_or("Codex 原生登记目录无效")?;
    let home = crate::client_software::command_path(&scope.home);
    let sqlite = crate::client_software::command_path(sqlite);
    let sqlite_text = sqlite.to_str().ok_or("Codex 原生登记目录无法传递给客户端")?;
    let installation = crate::client_software::discovery::resolve(AppKind::Codex)?;
    let mut command = crate::client_software::discovery::launch::command(&installation)?;
    command.arg(action);
    if action == "delete" { command.arg("--force"); }
    command.args([&id.to_string(), "-c", &format!("sqlite_home={}", toml_edit::Value::from(sqlite_text))])
        .env("CODEX_HOME", &home).env("CODEX_SQLITE_HOME", &sqlite).current_dir(home);
    for key in ["CODEX_EXEC_SERVER_URL", "CODEX_EXEC_SERVER_NOISE_REGISTRY_URL",
        "CODEX_EXEC_SERVER_NOISE_ENVIRONMENT_ID", "CODEX_EXEC_SERVER_NOISE_CHATGPT_ACCOUNT_ID",
        "CODEX_EXEC_SERVER_NOISE_AUTH_TOKEN"] { command.env_remove(key); }
    Ok(command)
}
