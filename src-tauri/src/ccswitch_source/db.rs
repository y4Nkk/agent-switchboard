use asb_core::ccswitch::{self, CcSwitchRow};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::{Path, PathBuf};

/// Raw scan outcome before the store marks duplicates.
pub(super) struct RawScan {
    pub(super) proposals: Vec<ccswitch::CcSwitchProposal>,
    pub(super) skipped: Vec<ccswitch::CcSwitchSkip>,
}

/// Locates the default source database under the user home directory.
fn db_path() -> Result<PathBuf, String> {
    let home = crate::local_state::user_home_dir()?;
    Ok(Path::new(&home).join(".cc-switch").join("cc-switch.db"))
}

/// Resolves the source database file for a scan or import. A user-picked
/// folder (chosen with the native directory picker) must directly contain
/// `cc-switch.db`; absence keeps the default home location. This is the one
/// owner of the source-path contract shared by both commands.
pub(super) fn resolve_db_path(directory: Option<&str>) -> Result<PathBuf, String> {
    let Some(directory) = directory
        .map(str::trim)
        .filter(|directory| !directory.is_empty())
    else {
        return db_path();
    };
    let directory = Path::new(directory);
    if !directory.is_dir() {
        return Err("所选文件夹不存在".to_string());
    }
    let path = directory.join("cc-switch.db");
    if !path.is_file() {
        return Err("所选文件夹中没有 cc-switch.db".to_string());
    }
    Ok(path)
}

pub(super) fn open_read_only(path: &Path) -> Result<Connection, String> {
    if !path.is_file() {
        return Err("未找到导入源数据库".to_string());
    }
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("无法以只读方式打开数据库: {error}"))?;
    connection
        .busy_timeout(std::time::Duration::from_secs(2))
        .map_err(|error| format!("无法配置数据库只读等待: {error}"))?;
    Ok(connection)
}

/// Reads every provider row in stable `rowid` order. The source schema owns
/// optional query scripts in `meta`. The display columns arrived across
/// source versions, so a column the table lacks is selected as a literal
/// NULL, which keeps every result position stable.
fn read_rows(connection: &Connection) -> Result<Vec<CcSwitchRow>, String> {
    let present: std::collections::BTreeSet<String> = connection
        .prepare("PRAGMA table_info(providers)")
        .map_err(|error| format!("无法读取 providers 表: {error}"))?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("无法读取 providers 表: {error}"))?
        .collect::<Result<_, _>>()
        .map_err(|error| format!("无法读取 providers 表: {error}"))?;
    let column = |name: &str| {
        if present.contains(name) {
            name.to_string()
        } else {
            format!("NULL AS {name}")
        }
    };
    let mut statement = connection
        .prepare(&format!(
            "SELECT id, app_type, name, settings_config, website_url, notes, meta, {}, {}, {}, {} \
             FROM providers ORDER BY rowid",
            column("icon"),
            column("icon_color"),
            column("category"),
            column("created_at"),
        ))
        .map_err(|error| format!("无法读取 providers 表: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            let display = asb_core::contracts::ProviderDisplay {
                icon: row.get(7)?,
                icon_color: row.get(8)?,
                category: row.get(9)?,
                created_at: row.get(10)?,
            };
            let display = (display.icon.is_some()
                || display.icon_color.is_some()
                || display.category.is_some()
                || display.created_at.is_some())
            .then_some(display);
            Ok(CcSwitchRow {
                id: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                app_type: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                settings_config: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                website_url: row.get::<_, Option<String>>(4)?,
                notes: row.get::<_, Option<String>>(5)?,
                meta: row.get::<_, Option<String>>(6)?,
                display,
            })
        })
        .map_err(|error| format!("无法读取 providers 表: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取 providers 表: {error}"))
}

pub(super) fn scan_db(path: &Path) -> Result<RawScan, String> {
    let mut connection = open_read_only(path)?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法读取来源快照: {error}"))?;
    let mut rows = read_rows(&transaction)?;
    super::claude_order::reorder(&transaction, &mut rows)?;
    let endpoints = super::claude_endpoints::read(&transaction)?;
    let mut proposals = Vec::new();
    let mut skipped = Vec::new();
    for row in rows {
        match ccswitch::map_row(&row) {
            Ok(mut proposal) => {
                if let ccswitch::CcSwitchProviderDraft::Claude(draft) = &mut proposal.draft {
                    if let Some(source) = endpoints.get(&row.id) {
                        super::claude_endpoints::merge_into(draft, source);
                    }
                }
                if has_invalid_usage_query(&proposal) {
                    clear_usage_query(&mut proposal);
                    proposal
                        .warnings
                        .push("未导入: meta.usage_script.code（无法转换为本应用脚本）".to_string());
                }
                proposals.push(proposal);
            }
            Err(skip) => skipped.push(skip),
        }
    }
    Ok(RawScan {
        proposals,
        skipped,
    })
}

/// Whether one source table exists. Old source databases legitimately lack
/// the newer tables, so absence is data, not an error.
pub(super) fn table_exists(connection: &Connection, table: &str) -> Result<bool, String> {
    let found: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("无法读取来源表结构: {error}"))?;
    Ok(found.is_some())
}

/// Whether one source column exists. Older source schemas predate the
/// failover columns, so a missing column reads as "no queue", not an error.
pub(super) fn column_exists(
    connection: &Connection,
    table: &str,
    column: &str,
) -> Result<bool, String> {
    if !table_exists(connection, table)? {
        return Ok(false);
    }
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| format!("无法读取来源表结构: {error}"))?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("无法读取来源表结构: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取来源表结构: {error}"))?;
    Ok(names.iter().any(|name| name == column))
}

fn has_invalid_usage_query(proposal: &ccswitch::CcSwitchProposal) -> bool {
    let usage_query = match &proposal.draft {
        ccswitch::CcSwitchProviderDraft::Claude(draft) => draft.usage_query.as_ref(),
        ccswitch::CcSwitchProviderDraft::Codex(draft) => draft.usage_query.as_ref(),
        // Official rows never carry a usage query.
        ccswitch::CcSwitchProviderDraft::CodexOfficial(_) => None,
    };
    usage_query.is_some_and(|query| asb_core::validate::validate_usage_query(query).is_err())
}

fn clear_usage_query(proposal: &mut ccswitch::CcSwitchProposal) {
    match &mut proposal.draft {
        ccswitch::CcSwitchProviderDraft::Claude(draft) => draft.usage_query = None,
        ccswitch::CcSwitchProviderDraft::Codex(draft) => draft.usage_query = None,
        ccswitch::CcSwitchProviderDraft::CodexOfficial(_) => {}
    }
}
