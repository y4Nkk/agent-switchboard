//! Applies exported provider SQL files and imports the restored records.
//!
//! An exported file is applied only to the app-owned scratch database under
//! `state/provider-import/` — never to the user's home databases — inside an
//! authorizer that denies every statement the exporter never emits. Rows are
//! then validated back into the typed provider documents
//! ([`asb_core::provider_transfer`]) and previewed without secrets. Importing
//! re-applies the same file first — the freshness guard — then writes each
//! selected record through the stores' validating file boundary, so another
//! device receives the complete stored configuration.

use crate::config_store::providers::load_provider_files;
use crate::local_state::LocalState;
use asb_core::contracts::{AppKind, CodexProviderFile, ProviderFile, RouteMode};
use asb_core::provider_transfer::{parse_transfer_row, TransferKind, TransferProvider, TransferRow};
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::path::{Path, PathBuf};
use uuid::Uuid;
mod importing;
pub use importing::import;

/// The largest exported file accepted for applying.
const MAX_SQL_BYTES: u64 = 32 * 1024 * 1024;
const SCRATCH_DIRECTORY: &str = "provider-import";
const SCRATCH_FILE: &str = "agent-switchboard.db";
const TRANSFER_TABLE: &str = "agent_switchboard_providers";

/// One named item the transfer could not carry or import.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSqlSkip {
    pub name: String,
    pub reason: String,
}

/// One importable provider summary with the target store's duplicate marking
/// applied. This is the complete scan response contract: only fields shown in
/// the renderer, never an API key.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSqlScanItem {
    pub key: String,
    pub kind: TransferKind,
    pub name: String,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub warnings: Vec<String>,
    /// A record with the same id already exists; importing overwrites it in
    /// place at its stored position.
    pub existing: bool,
}

/// Full preview of one applied export file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSqlScan {
    pub sql_path: String,
    pub providers: Vec<ProviderSqlScanItem>,
    pub skipped: Vec<ProviderSqlSkip>,
}

/// Outcome of a batch import.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSqlImportOutcome {
    pub imported_count: usize,
    pub updated_count: usize,
    pub not_imported: Vec<ProviderSqlSkip>,
}

/// Applies the export file at `sql_path` to the scratch database and previews
/// its rows against the current stores.
pub fn scan(state: &LocalState, sql_path: &str) -> Result<ProviderSqlScan, String> {
    let sql_path = sql_path.trim();
    let (rows, skipped) = apply_and_parse(state, sql_path)?;
    let configuration = state.configuration();
    let claude = load_provider_files(&configuration, AppKind::Claude)
        .map_err(|error| error.to_string())?;
    let official = load_provider_files(&configuration, AppKind::Codex)
        .map_err(|error| error.to_string())?;
    let custom = configuration
        .codex_provider_snapshots()
        .map_err(|error| error.to_string())?;
    let providers = rows
        .into_iter()
        .map(|row| scan_item(row, &claude, &official, &custom))
        .collect();
    Ok(ProviderSqlScan {
        sql_path: sql_path.to_string(),
        providers,
        skipped,
    })
}

fn scan_item(
    row: TransferRow,
    claude: &[ProviderFile],
    official: &[ProviderFile],
    custom: &[(CodexProviderFile, String)],
) -> ProviderSqlScanItem {
    let TransferRow { id, kind, provider } = row;
    let (name, model, base_url, existing, warnings) = match provider {
        TransferProvider::Claude(file) => {
            let mut warnings = Vec::new();
            if file.route_mode == RouteMode::Official
                && !claude.iter().any(|stored| stored.id == id)
                && claude
                    .iter()
                    .any(|stored| stored.route_mode == RouteMode::Official)
            {
                warnings
                    .push("本机已有其他 Claude 官方登录记录，导入时将跳过".to_string());
            }
            (
                file.name.clone(),
                file.model.clone(),
                file.base_url.clone(),
                claude.iter().any(|stored| stored.id == id),
                warnings,
            )
        }
        TransferProvider::CodexOfficial(file) => {
            let mut warnings = Vec::new();
            if !official.iter().any(|stored| stored.id == id)
                && official
                    .iter()
                    .any(|stored| stored.route_mode == RouteMode::Official)
            {
                warnings.push("本机已有其他官方登录记录，导入时将跳过".to_string());
            }
            (
                file.name.clone(),
                file.model.clone(),
                file.base_url.clone(),
                official.iter().any(|stored| stored.id == id),
                warnings,
            )
        }
        TransferProvider::CodexCustom(file) => (
            file.profile.name.clone(),
            Some(file.profile.default_model.clone()),
            Some(file.profile.endpoint.0.clone()),
            custom.iter().any(|(stored, _)| stored.profile.id == id),
            Vec::new(),
        ),
    };
    ProviderSqlScanItem {
        key: id,
        kind,
        name,
        model,
        base_url,
        warnings,
        existing,
    }
}

fn scratch_path(state: &LocalState) -> PathBuf {
    state.root().join(SCRATCH_DIRECTORY).join(SCRATCH_FILE)
}

/// Removes the scratch database and its journal sidecars so one apply never
/// observes leftovers of a previous file.
fn reset_scratch(scratch: &Path) -> Result<(), String> {
    let parent = scratch.parent().ok_or("导入暂存数据库路径无效")?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("无法创建导入暂存目录：{error}"))?;
    let file = scratch
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("导入暂存数据库路径无效")?;
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(parent.join(format!("{file}{suffix}")));
    }
    Ok(())
}

/// Allows exactly the statement vocabulary the exporter emits. CREATE TABLE
/// additionally writes schema rows (`sqlite_master` updates) and builds an
/// implicit autoindex for its PRIMARY KEY constraint, so those two actions
/// are part of the exporter's own vocabulary — the autoindex guard is
/// airtight because SQLite reserves the `sqlite_` name prefix. Everything
/// else — ATTACH, PRAGMA, function calls, hand-written UPDATE or CREATE
/// INDEX — is denied.
fn deny_escaping_statements(context: AuthContext<'_>) -> Authorization {
    match context.action {
        AuthAction::CreateTable { .. }
        | AuthAction::Insert { .. }
        | AuthAction::Delete { .. }
        | AuthAction::Read { .. }
        | AuthAction::Select
        | AuthAction::Transaction { .. }
        | AuthAction::Update { table_name: "sqlite_master", .. } => Authorization::Allow,
        AuthAction::CreateIndex { index_name, .. } if index_name.starts_with("sqlite_autoindex_") => {
            Authorization::Allow
        }
        _ => Authorization::Deny,
    }
}

/// Applies the file to the reset scratch database and validates every row
/// back into typed provider documents. Rows that fail validation become named
/// skips instead of aborting the whole preview.
fn apply_and_parse(
    state: &LocalState,
    sql_path: &str,
) -> Result<(Vec<TransferRow>, Vec<ProviderSqlSkip>), String> {
    let path = Path::new(sql_path);
    if !path.is_file() {
        return Err("所选文件不存在".to_string());
    }
    let size = std::fs::metadata(path)
        .map_err(|error| format!("无法读取所选文件：{error}"))?
        .len();
    if size > MAX_SQL_BYTES {
        return Err("导出的 SQL 文件超过 32MB 上限".to_string());
    }
    let sql = std::fs::read_to_string(path)
        .map_err(|error| format!("无法读取所选文件：{error}"))?;
    let scratch = scratch_path(state);
    reset_scratch(&scratch)?;
    let connection = Connection::open_with_flags(
        &scratch,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .map_err(|error| format!("无法打开导入暂存数据库：{error}"))?;
    connection.authorizer(Some(deny_escaping_statements));
    connection.execute_batch(&sql).map_err(|error| {
        if error.to_string().contains("not authorized") {
            "所选文件包含 Agent Switchboard 导出格式之外的语句，已拒绝应用".to_string()
        } else {
            format!("无法应用所选 SQL 文件：{error}")
        }
    })?;
    let not_export_file = || "所选文件不是 Agent Switchboard 导出的供应商文件".to_string();
    let mut statement = connection
        .prepare(&format!(
            "SELECT id, kind, position, file FROM {TRANSFER_TABLE} ORDER BY position, id"
        ))
        .map_err(|_| not_export_file())?;
    let applied = statement
        .query_map([], |record| {
            Ok((
                record.get::<_, String>(0)?,
                record.get::<_, String>(1)?,
                record.get::<_, i64>(2)?,
                record.get::<_, String>(3)?,
            ))
        })
        .map_err(|_| not_export_file())?;
    let mut rows = Vec::new();
    let mut skipped = Vec::new();
    for entry in applied {
        let (id, kind, position, file) =
            entry.map_err(|error| format!("无法读取导入暂存数据：{error}"))?;
        if Uuid::parse_str(&id).is_err() {
            skipped.push(ProviderSqlSkip {
                name: id,
                reason: "档案 id 不是有效的 UUID".to_string(),
            });
            continue;
        }
        match parse_transfer_row(&id, &kind, position, &file) {
            Ok(row) => rows.push(row),
            Err(reason) => skipped.push(ProviderSqlSkip { name: id, reason }),
        }
    }
    Ok((rows, skipped))
}

#[cfg(test)]
mod tests;
