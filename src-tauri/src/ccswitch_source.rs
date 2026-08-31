//! Read-only access to the CC Switch SQLite database.
//!
//! The database is opened with `mode=ro&immutable=1` so a running CC Switch
//! instance is never locked or written. Only the `providers` table is read;
//! secret-bearing values stay inside `settings_config` text and are handed to
//! `asb_core::ccswitch`, whose output has no field capable of carrying them.

use crate::local_state::LocalState;
use asb_core::ccswitch::{self, CcSwitchRow};
use asb_core::contracts::{AppKind, ProviderDraft, ProviderProfile};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Raw scan outcome before the store marks duplicates.
struct RawScan {
    proposals: Vec<ccswitch::CcSwitchProposal>,
    skipped: Vec<ccswitch::CcSwitchSkip>,
}

/// Locates the CC Switch database under the user profile.
fn db_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("USERPROFILE")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "无法确定 Windows 用户目录".to_string())?;
    Ok(Path::new(&home).join(".cc-switch").join("cc-switch.db"))
}

fn open_read_only(path: &Path) -> Result<Connection, String> {
    if !path.is_file() {
        return Err("未找到 CC Switch 数据库(需要 CC Switch 3.x 已创建数据)".to_string());
    }
    let uri = format!(
        "file:{}?mode=ro&immutable=1",
        path.to_string_lossy().replace('\\', "/")
    );
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI;
    Connection::open_with_flags(&uri, flags).map_err(|error| format!("无法打开数据库: {error}"))
}

/// Reads every provider row. `rowid` order is stable across schema versions
/// that may lack `sort_index`; both metadata columns are nullable in the
/// CC Switch 3.x schema this scan requires.
fn read_rows(connection: &Connection) -> Result<Vec<CcSwitchRow>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, app_type, name, settings_config, website_url, notes \
             FROM providers ORDER BY rowid",
        )
        .map_err(|error| format!("无法读取 providers 表: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(CcSwitchRow {
                id: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                app_type: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                settings_config: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                website_url: row.get::<_, Option<String>>(4)?,
                notes: row.get::<_, Option<String>>(5)?,
            })
        })
        .map_err(|error| format!("无法读取 providers 表: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取 providers 表: {error}"))
}

fn scan_db(path: &Path) -> Result<RawScan, String> {
    let connection = open_read_only(path)?;
    let mut proposals = Vec::new();
    let mut skipped = Vec::new();
    for row in read_rows(&connection)? {
        match ccswitch::map_row(&row) {
            Ok(proposal) => proposals.push(proposal),
            Err(skip) => skipped.push(skip),
        }
    }
    Ok(RawScan { proposals, skipped })
}

/// One importable provider with the store's duplicate marking applied.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CcSwitchScanItem {
    pub key: String,
    pub app: AppKind,
    pub draft: ProviderDraft,
    pub warnings: Vec<String>,
    /// An exactly equal profile already exists; importing is a no-op.
    pub existing: bool,
}

/// Full scan result for the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CcSwitchScan {
    pub db_path: String,
    pub providers: Vec<CcSwitchScanItem>,
    pub skipped: Vec<ccswitch::CcSwitchSkip>,
}

/// Outcome of a batch import.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CcSwitchImportOutcome {
    pub imported: Vec<ProviderProfile>,
    pub skipped_existing: Vec<String>,
    pub not_imported: Vec<ccswitch::CcSwitchSkip>,
}

/// Scans the real user database and marks store duplicates.
pub fn scan(state: &LocalState) -> Result<CcSwitchScan, String> {
    scan_at(&db_path()?, state)
}

/// Scans an explicit database path (test entry point; still strictly
/// read-only).
pub fn scan_at(path: &Path, state: &LocalState) -> Result<CcSwitchScan, String> {
    let raw = scan_db(path)?;
    let providers = raw
        .proposals
        .into_iter()
        .map(|proposal| CcSwitchScanItem {
            existing: state.profile_exists(&proposal.draft),
            key: proposal.key,
            app: proposal.app,
            draft: proposal.draft,
            warnings: proposal.warnings,
        })
        .collect();
    Ok(CcSwitchScan {
        db_path: path.to_string_lossy().into_owned(),
        providers,
        skipped: raw.skipped,
    })
}

/// Re-scans the real user database (so a stale preview can never import) and
/// imports the requested keys. Writes only the app's own profile store.
pub fn import(state: &LocalState, keys: &[String]) -> Result<CcSwitchImportOutcome, String> {
    import_at(&db_path()?, state, keys)
}

/// Imports requested keys from an explicit database path (test entry point).
pub fn import_at(
    path: &Path,
    state: &LocalState,
    keys: &[String],
) -> Result<CcSwitchImportOutcome, String> {
    let raw = scan_db(path)?;
    let mut outcome = CcSwitchImportOutcome {
        imported: Vec::new(),
        skipped_existing: Vec::new(),
        not_imported: Vec::new(),
    };
    for key in keys {
        let Some(proposal) = raw.proposals.iter().find(|p| &p.key == key) else {
            let (name, reason) = match raw.skipped.iter().find(|s| &s.key == key) {
                Some(skip) => (skip.name.clone(), skip.reason.clone()),
                None => (key.clone(), "扫描结果已变化,请重新扫描".to_string()),
            };
            outcome.not_imported.push(ccswitch::CcSwitchSkip {
                key: key.clone(),
                app_type: key.split(':').next().unwrap_or("").to_string(),
                name,
                reason,
            });
            continue;
        };
        if state.profile_exists(&proposal.draft) {
            outcome.skipped_existing.push(proposal.draft.name.clone());
            continue;
        }
        outcome
            .imported
            .push(state.import_profile(proposal.draft.clone())?);
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    // Synthetic shapes only; secret-looking slots hold an obvious placeholder.
    fn fixture_db(dir: &Path) -> PathBuf {
        let path = dir.join("cc-switch.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE providers (
                    id TEXT,
                    app_type TEXT,
                    name TEXT,
                    settings_config TEXT,
                    website_url TEXT,
                    notes TEXT,
                    PRIMARY KEY (id, app_type)
                );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO providers (id, app_type, name, settings_config, website_url, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "id-1",
                    "claude",
                    "中继 A",
                    r#"{"env":{"ANTHROPIC_BASE_URL":"https://relay.internal","ANTHROPIC_AUTH_TOKEN":"<placeholder>","ANTHROPIC_MODEL":"claude-x","CLAUDE_CODE_SUBAGENT_MODEL":"sub-x"},"permissions":{"defaultMode":"auto"}}"#,
                    "https://relay.internal",
                    "主力中继",
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO providers (id, app_type, name, settings_config) VALUES (?1, ?2, ?3, ?4)",
                params![
                    "id-2",
                    "codex",
                    "订阅",
                    r#"{"auth":{"OPENAI_API_KEY":null,"tokens":{"refresh_token":"<placeholder>"}},"config":""}"#
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO providers (id, app_type, name, settings_config) VALUES (?1, ?2, ?3, ?4)",
                params!["id-3", "gemini", "双子", "{}"],
            )
            .unwrap();
        path
    }

    #[test]
    fn scan_marks_duplicates_and_filters_out_of_scope_clients() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_db(dir.path());
        let state = LocalState::from_root(dir.path().join("state"));

        let first = scan_at(&path, &state).unwrap();
        assert_eq!(first.db_path, path.to_string_lossy());
        // The official Codex row is skipped alongside the out-of-scope one.
        assert_eq!(first.providers.len(), 1);
        assert_eq!(first.skipped.len(), 2);
        assert!(first.skipped.iter().any(|s| s.reason.contains("gemini")));
        assert!(first.skipped.iter().any(|s| s.reason.contains("官方登录")));
        assert!(first.providers.iter().all(|item| !item.existing));
        assert!(!format!("{first:?}").contains("<placeholder>"));

        let claude = first
            .providers
            .iter()
            .find(|item| item.app == AppKind::Claude)
            .expect("claude proposal");
        assert_eq!(claude.draft.api_key, "<placeholder>");
        state.import_profile(claude.draft.clone()).unwrap();
        let second = scan_at(&path, &state).unwrap();
        assert!(
            second
                .providers
                .iter()
                .find(|item| item.key == claude.key)
                .expect("same proposal")
                .existing
        );
    }

    #[test]
    fn import_reuses_dedup_and_reports_skips() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_db(dir.path());
        let state = LocalState::from_root(dir.path().join("state"));

        let outcome = import_at(
            &path,
            &state,
            &[
                "claude:id-1".into(),
                "gemini:id-3".into(),
                "claude:id-9".into(),
            ],
        )
        .unwrap();
        assert_eq!(outcome.imported.len(), 1);
        assert_eq!(outcome.imported[0].name, "中继 A");
        assert_eq!(outcome.not_imported.len(), 2);
        assert!(outcome
            .not_imported
            .iter()
            .any(|item| item.reason.contains("gemini")));
        assert!(!format!("{outcome:?}").contains("<placeholder>"));

        let again = import_at(&path, &state, &["claude:id-1".into()]).unwrap();
        assert!(again.imported.is_empty());
        assert_eq!(again.skipped_existing, vec!["中继 A".to_string()]);
    }

    #[test]
    fn missing_database_reports_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let error = open_read_only(&dir.path().join("none.db")).unwrap_err();
        assert!(error.contains("未找到"));
    }

    /// Read-only smoke test against the user's real CC Switch database.
    /// Ignored by default: it needs a machine that actually has CC Switch 3.x
    /// data; run explicitly with `cargo test -p agent-switchboard -- --ignored`.
    ///
    /// Warnings legitimately contain secret FIELD NAMES ("凭据
    /// env.ANTHROPIC_AUTH_TOKEN 不导入"); what must never appear are VALUE
    /// shapes, which the fixed warning templates in asb-core guarantee.
    #[test]
    #[ignore = "requires a real CC Switch database under the user profile"]
    fn real_database_scan_is_read_only_and_secret_free() {
        let dir = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(dir.path().join("state"));
        let scan = scan(&state).expect("real scan");
        assert!(scan.db_path.ends_with("cc-switch.db"));
        let rendered = format!("{scan:?}");
        for item in &scan.providers {
            for warning in &item.warnings {
                assert!(
                    warning.starts_with("凭据 ")
                        || warning.starts_with("未导入: ")
                        || warning.starts_with("API 密钥不导入")
                        || warning.starts_with("官方登录凭据")
                );
            }
            assert!(item.draft.base_url.as_deref().unwrap_or("").len() > 8);
            assert!(!item.draft.name.contains(':'));
        }
        eprintln!(
            "real scan: {} providers, {} skipped",
            scan.providers.len(),
            scan.skipped.len()
        );
        assert!(!rendered.contains("sk-"));
    }
}
