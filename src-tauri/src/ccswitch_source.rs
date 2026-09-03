//! Read-only access to the CC Switch SQLite database.
//!
//! The database is opened with `mode=ro&immutable=1` so a running CC Switch
//! instance is never locked or written. Only the `providers` table is read;
//! custom-provider credentials stay inside the backend mapping for profile
//! import, while scan diagnostics expose only field names.

use crate::local_state::LocalState;
use asb_core::ccswitch::{self, CcSwitchRow};
use asb_core::contracts::AppKind;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Raw scan outcome before the store marks duplicates.
struct RawScan {
    proposals: Vec<ccswitch::CcSwitchProposal>,
    skipped: Vec<ccswitch::CcSwitchSkip>,
}

/// Locates the CC Switch database under the user home directory.
fn db_path() -> Result<PathBuf, String> {
    let home = crate::local_state::user_home_dir()?;
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

/// Reads every provider row in stable `rowid` order. The current CC Switch
/// schema owns optional query scripts in `meta`.
fn read_rows(connection: &Connection) -> Result<Vec<CcSwitchRow>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, app_type, name, settings_config, website_url, notes, meta \
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
                meta: row.get::<_, Option<String>>(6)?,
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
            Ok(mut proposal) => {
                if proposal
                    .draft
                    .usage_query
                    .as_ref()
                    .is_some_and(|query| crate::usage_query::validate_persisted(query).is_err())
                {
                    proposal.draft.usage_query = None;
                    proposal
                        .warnings
                        .push("未导入: meta.usage_script.code（无法转换为本应用脚本）".to_string());
                }
                proposals.push(proposal);
            }
            Err(skip) => skipped.push(skip),
        }
    }
    Ok(RawScan { proposals, skipped })
}

/// One importable provider summary with the store's duplicate marking applied.
///
/// This is the complete scan response contract. It deliberately contains only
/// the fields displayed in the renderer and cannot carry an API key.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CcSwitchScanItem {
    pub key: String,
    pub app: AppKind,
    pub route_mode: asb_core::RouteMode,
    pub name: String,
    pub model: Option<String>,
    pub base_url: Option<String>,
    /// Whether the selected import will persist one native usage query. The
    /// source itself remains backend-only.
    pub usage_script_importable: bool,
    /// A routing-identical local profile has no query yet, so this import
    /// enriches that profile instead of creating a duplicate.
    pub usage_script_updates_existing: bool,
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
    pub imported_count: usize,
    pub usage_script_imported_count: usize,
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
        .map(|proposal| {
            let ccswitch::CcSwitchProposal {
                key,
                app,
                draft,
                warnings,
            } = proposal;
            let existing = state.configuration().provider_exists(&draft);
            let usage_script_importable = draft.usage_query.is_some();
            let usage_script_updates_existing = !existing
                && state
                    .configuration()
                    .provider_will_receive_usage_query(&draft);
            CcSwitchScanItem {
                key,
                app,
                route_mode: draft.route_mode,
                name: draft.name,
                model: draft.model,
                base_url: draft.base_url,
                usage_script_importable,
                usage_script_updates_existing,
                warnings,
                existing,
            }
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
        imported_count: 0,
        usage_script_imported_count: 0,
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
        if state.configuration().provider_exists(&proposal.draft) {
            outcome.skipped_existing.push(proposal.draft.name.clone());
            continue;
        }
        state
            .configuration()
            .import_provider(proposal.draft.clone())?;
        outcome.imported_count += 1;
        if proposal.draft.usage_query.is_some() {
            outcome.usage_script_imported_count += 1;
        }
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::{ProviderDraft, ProviderProfile};
    use rusqlite::params;

    fn profiles(state: &LocalState) -> Vec<ProviderProfile> {
        state
            .configuration()
            .list_providers()
            .unwrap()
            .into_iter()
            .map(|record| record.profile)
            .collect()
    }

    // A deliberately nonstandard credential shape proves scan serialization
    // cannot rely on matching a token prefix to keep credentials private.
    const SOURCE_TOKEN: &str = "opaque-source-credential-42";
    const SOURCE_USAGE_SCRIPT: &str = r#"({
        request: {
            url: "{{baseUrl}}/usage",
            method: "GET",
            headers: { Authorization: "Bearer {{apiKey}}" }
        },
        extractor: function(response) {
            return {
                isValid: true,
                planName: "主套餐",
                remaining: response.remaining,
                used: response.used,
                total: 100,
                unit: "USD"
            };
        }
    })"#;

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
                    meta TEXT,
                    PRIMARY KEY (id, app_type)
                );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO providers (id, app_type, name, settings_config, website_url, notes, meta) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    "id-1",
                    "claude",
                    "中继 A",
                    format!(
                        r#"{{"env":{{"ANTHROPIC_BASE_URL":"https://relay.internal","ANTHROPIC_AUTH_TOKEN":"{SOURCE_TOKEN}","ANTHROPIC_MODEL":"claude-x","CLAUDE_CODE_SUBAGENT_MODEL":"sub-x"}},"permissions":{{"defaultMode":"auto"}}}}"#
                    ),
                    "https://relay.internal",
                    "主力中继",
                    serde_json::json!({
                        "usage_script": {
                            "enabled": true,
                            "language": "javascript",
                            "code": SOURCE_USAGE_SCRIPT,
                            "timeout": 8
                        }
                    })
                    .to_string(),
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
        assert_eq!(first.providers.len(), 2);
        assert_eq!(first.skipped.len(), 1);
        assert!(first.skipped.iter().any(|s| s.reason.contains("gemini")));
        assert!(first.providers.iter().all(|item| !item.existing));
        let serialized = serde_json::to_string(&first).unwrap();
        assert!(!serialized.contains(SOURCE_TOKEN));
        assert!(!serialized.contains(SOURCE_USAGE_SCRIPT));
        assert!(!serialized.contains("\"apiKey\""));
        assert!(!serialized.contains("\"draft\""));

        let claude = first
            .providers
            .iter()
            .find(|item| item.app == AppKind::Claude)
            .expect("claude proposal");
        assert_eq!(claude.name, "中继 A");
        let official = first
            .providers
            .iter()
            .find(|item| item.app == AppKind::Codex)
            .expect("official Codex route");
        assert_eq!(official.name, "Codex 官方登录");
        assert_eq!(claude.model.as_deref(), Some("claude-x"));
        assert_eq!(claude.base_url.as_deref(), Some("https://relay.internal"));
        assert!(claude.usage_script_importable);
        let imported = import_at(&path, &state, &[claude.key.clone()]).unwrap();
        assert_eq!(imported.imported_count, 1);
        assert!(!serde_json::to_string(&imported)
            .unwrap()
            .contains(SOURCE_TOKEN));
        assert!(!serde_json::to_string(&imported)
            .unwrap()
            .contains(SOURCE_USAGE_SCRIPT));
        let mut profiles = profiles(&state);
        let profile = profiles.remove(0);
        assert_eq!(profile.api_key, SOURCE_TOKEN);
        assert!(matches!(
            profile.usage_query,
            Some(asb_core::contracts::UsageQuery::Script { .. })
        ));
        assert_eq!(imported.usage_script_imported_count, 1);
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
        assert_eq!(outcome.imported_count, 1);
        assert_eq!(outcome.not_imported.len(), 2);
        assert!(outcome
            .not_imported
            .iter()
            .any(|item| item.reason.contains("gemini")));
        assert!(!serde_json::to_string(&outcome)
            .unwrap()
            .contains(SOURCE_TOKEN));

        let again = import_at(&path, &state, &["claude:id-1".into()]).unwrap();
        assert_eq!(again.imported_count, 0);
        assert_eq!(again.skipped_existing, vec!["中继 A".to_string()]);
    }

    #[test]
    fn import_enriches_a_matching_profile_with_the_source_usage_script() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_db(dir.path());
        let state = LocalState::from_root(dir.path().join("state"));
        let existing = state
            .configuration()
            .import_provider(ProviderDraft {
                app: AppKind::Claude,
                route_mode: asb_core::RouteMode::Custom,
                name: "中继 A".to_string(),
                model: Some("claude-x".to_string()),
                base_url: Some("https://relay.internal".to_string()),
                api_key: SOURCE_TOKEN.to_string(),
                model_options: None,
                notes: Some("主力中继".to_string()),
                website_url: Some("https://relay.internal".to_string()),
                usage_query: None,
            })
            .expect("existing routing profile");

        let scan = scan_at(&path, &state).expect("scan");
        let proposal = scan
            .providers
            .iter()
            .find(|provider| provider.key == "claude:id-1")
            .expect("custom provider");
        assert!(!proposal.existing);
        assert!(proposal.usage_script_importable);
        assert!(proposal.usage_script_updates_existing);

        let outcome = import_at(&path, &state, &["claude:id-1".to_string()]).expect("import");
        assert_eq!(outcome.imported_count, 1);
        assert_eq!(outcome.usage_script_imported_count, 1);
        let profiles = profiles(&state);
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].id, existing.profile.id);
        assert!(profiles[0].usage_query.is_some());
    }

    #[test]
    fn import_persists_lowercase_one_m_as_semantic_profile_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_db(dir.path());
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE providers SET settings_config = ?1 WHERE id = ?2 AND app_type = ?3",
                params![
                    r#"{"env":{"ANTHROPIC_BASE_URL":"https://relay.internal","ANTHROPIC_AUTH_TOKEN":"<placeholder>","ANTHROPIC_MODEL":"claude-opus-4-1[1m]","ANTHROPIC_DEFAULT_SONNET_MODEL":"claude-sonnet-4-6[1m]","ANTHROPIC_DEFAULT_OPUS_MODEL":"claude-opus-4-1[1m]"}}"#,
                    "id-1",
                    "claude",
                ],
            )
            .unwrap();

        let state = LocalState::from_root(dir.path().join("state"));
        let outcome = import_at(&path, &state, &["claude:id-1".into()]).unwrap();
        assert_eq!(outcome.imported_count, 1);
        let profiles = profiles(&state);
        let imported = profiles.first().expect("one profile should import");

        assert_eq!(imported.model.as_deref(), Some("claude-opus-4-1"));
        let Some(asb_core::ModelOptions::Claude(settings)) = imported.model_options.as_ref() else {
            panic!("Claude model settings should be persisted");
        };
        assert!(settings.primary_one_m);
        assert_eq!(settings.sonnet_model.as_deref(), Some("claude-sonnet-4-6"));
        assert!(settings.sonnet_one_m);
        assert_eq!(settings.opus_model.as_deref(), Some("claude-opus-4-1"));
        assert!(settings.opus_one_m);
    }

    #[test]
    fn import_normalizes_ccswitch_uppercase_one_m_models_before_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = fixture_db(dir.path());
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE providers SET settings_config = ?1 WHERE id = ?2 AND app_type = ?3",
                params![
                    r#"{"env":{"ANTHROPIC_BASE_URL":"https://relay.internal","ANTHROPIC_AUTH_TOKEN":"<placeholder>","ANTHROPIC_MODEL":"claude-opus-4-1[1M]"}}"#,
                    "id-1",
                    "claude",
                ],
            )
            .unwrap();

        let state = LocalState::from_root(dir.path().join("state"));
        let scan = scan_at(&path, &state).unwrap();
        assert!(scan.providers.iter().any(|item| item.key == "claude:id-1"));

        let outcome = import_at(&path, &state, &["claude:id-1".into()]).unwrap();
        assert_eq!(outcome.imported_count, 1);
        let profiles = profiles(&state);
        let Some(asb_core::ModelOptions::Claude(settings)) = profiles[0].model_options.as_ref()
        else {
            panic!("Claude model settings should be persisted");
        };
        assert!(settings.primary_one_m);
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
    /// The serialized scan contract has no API-key field. Its warnings carry
    /// only unsupported field names, never values.
    #[test]
    #[ignore = "requires a real CC Switch database under the user profile"]
    fn real_database_scan_is_read_only_and_secret_free() {
        let dir = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(dir.path().join("state"));
        let scan = scan(&state).expect("real scan");
        assert!(scan.db_path.ends_with("cc-switch.db"));
        let serialized = serde_json::to_string(&scan).unwrap();
        for item in &scan.providers {
            for warning in &item.warnings {
                assert!(warning.starts_with("未导入: "));
            }
            assert!(item.base_url.as_deref().unwrap_or("").len() > 8);
            assert!(!item.name.contains(':'));
        }
        eprintln!(
            "real scan: {} providers, {} skipped",
            scan.providers.len(),
            scan.skipped.len()
        );
        assert!(!serialized.contains("\"apiKey\""));
        assert!(!serialized.contains("\"draft\""));
    }
}
