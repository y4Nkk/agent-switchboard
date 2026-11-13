//! Read-only access to the external provider SQLite database.
//!
//! A read-only transaction observes committed WAL changes without modifying
//! provider data. Only the providers table and its column metadata are read.
//! The source location has one owner, [`db::resolve_db_path`]: the default
//! home database, or a user-picked folder that directly contains
//! `cc-switch.db`. Source credentials never cross the IPC boundary: scans
//! expose routing facts and field names only, and both Claude and Codex rows
//! are imported entirely inside the backend through the batch command.

mod claude_endpoints;
mod claude_order;
pub(crate) mod codex_failover;
mod db;


use crate::config_store::StoreOperationError;
use crate::local_state::LocalState;
use asb_core::ccswitch;
use asb_core::contracts::{AppKind, RouteMode};
use db::{resolve_db_path, scan_db};
use serde::Serialize;
use std::path::Path;

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
    /// Endpoint candidates carried on import. For Claude rows the source's
    /// endpoint table replaces meta-derived entries, mirroring the source's
    /// own read path.
    pub endpoint_candidates: usize,
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
    pub endpoint_candidates_imported: usize,
    pub skipped_existing: Vec<String>,
    pub not_imported: Vec<ccswitch::CcSwitchSkip>,
}

/// Scans the real user database and marks store duplicates. `directory` is a
/// user-picked folder that directly contains `cc-switch.db`; absence keeps
/// the default home location.
pub fn scan(state: &LocalState, directory: Option<&str>) -> Result<CcSwitchScan, String> {
    scan_at(&resolve_db_path(directory)?, state)
}

/// Scans an explicit database path (test entry point; still strictly
/// read-only).
pub fn scan_at(path: &Path, state: &LocalState) -> Result<CcSwitchScan, String> {
    let raw = scan_db(path)?;
    let providers = raw
        .proposals
        .into_iter()
        .map(|proposal| scan_item(state, proposal))
        .collect();
    // The scan response carries only rows this application can act on. Rows
    // of foreign clients are invisible, never a "cannot import" wall.
    let skipped = raw
        .skipped
        .into_iter()
        .filter(|skip| matches!(skip.app_type.as_str(), "claude" | "codex"))
        .collect();
    Ok(CcSwitchScan {
        db_path: path.to_string_lossy().into_owned(),
        providers,
        skipped,
    })
}

/// Re-scans the real user database (so a stale preview can never import) and
/// imports the requested keys: Claude rows and the Codex official record go
/// to the generic store, third-party Codex rows complete into the strict
/// store inside the backend. `directory` selects the same folder the scan
/// used, so the freshness check reads the database the user previewed. Writes
/// only the app's own profile store.
pub fn import(
    state: &LocalState,
    keys: &[String],
    directory: Option<&str>,
) -> Result<CcSwitchImportOutcome, StoreOperationError> {
    import_at(&resolve_db_path(directory)?, state, keys)
}

/// Imports requested Claude keys from an explicit database path (test entry
/// point).
pub fn import_at(
    path: &Path,
    state: &LocalState,
    keys: &[String],
) -> Result<CcSwitchImportOutcome, StoreOperationError> {
    let raw = scan_db(path)?;
    let proposals: Vec<(&str, &ccswitch::CcSwitchProviderDraft)> = raw
        .proposals
        .iter()
        .map(|proposal| (proposal.key.as_str(), &proposal.draft))
        .collect();
    let mut outcome = CcSwitchImportOutcome {
        imported_count: 0,
        usage_script_imported_count: 0,
        endpoint_candidates_imported: 0,
        skipped_existing: Vec::new(),
        not_imported: Vec::new(),
    };
    for key in keys {
        let Some(draft) = proposals
            .iter()
            .find(|(proposal_key, _)| proposal_key == &key.as_str())
            .map(|(_, draft)| *draft)
        else {
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
        match draft {
            ccswitch::CcSwitchProviderDraft::Claude(draft) => {
                if state.configuration().provider_exists(draft) {
                    outcome.skipped_existing.push(draft.name.clone());
                    continue;
                }
                let endpoint_candidates = draft.connection.custom_endpoints.len();
                state.configuration().import_provider(draft.clone())?;
                outcome.endpoint_candidates_imported += endpoint_candidates;
                if draft.usage_query.is_some() {
                    outcome.usage_script_imported_count += 1;
                }
                outcome.imported_count += 1;
            }
            ccswitch::CcSwitchProviderDraft::CodexOfficial(draft) => {
                if state.configuration().provider_exists(draft) {
                    outcome.skipped_existing.push(draft.name.clone());
                    continue;
                }
                state.configuration().import_provider(draft.clone())?;
                if draft.usage_query.is_some() {
                    outcome.usage_script_imported_count += 1;
                }
                outcome.imported_count += 1;
            }
            ccswitch::CcSwitchProviderDraft::Codex(seed) => import_codex_seed(state, key, seed, &mut outcome)?,
        }
    }
    Ok(outcome)
}

fn import_codex_seed(
    state: &LocalState,
    key: &str,
    seed: &ccswitch::CodexImportSeed,
    outcome: &mut CcSwitchImportOutcome,
) -> Result<(), StoreOperationError> {
    if state
        .configuration()
        .codex_route_exists(&seed.endpoint, seed.upstream)
    {
        // A route-identical profile gains the source's missing
        // usage query; a query-less row or an already-queried
        // profile is a true duplicate.
        let enriched = match seed.usage_query.clone() {
            Some(query) => state.configuration().enrich_codex_route_usage_query(
                &seed.endpoint,
                seed.upstream,
                query,
            )?,
            None => false,
        };
        if !enriched {
            outcome.skipped_existing.push(seed.name.clone());
            return Ok(());
        }
        outcome.usage_script_imported_count += 1;
        outcome.imported_count += 1;
        return Ok(());
    }
    let draft = match seed.completion_draft() {
        Ok(draft) => draft,
        Err(reason) => {
            outcome.not_imported.push(ccswitch::CcSwitchSkip {
                key: key.to_string(),
                app_type: "codex".to_string(),
                name: seed.name.clone(),
                reason,
            });
            return Ok(());
        }
    };
    state.configuration().create_codex_provider(uuid::Uuid::new_v4().to_string(), draft)?;
    if seed.usage_query.is_some() {
        outcome.usage_script_imported_count += 1;
    }
    outcome.imported_count += 1;
    Ok(())
}

fn scan_item(state: &LocalState, proposal: ccswitch::CcSwitchProposal) -> CcSwitchScanItem {
    let ccswitch::CcSwitchProposal {
        key,
        draft,
        warnings,
    } = proposal;
    match draft {
        ccswitch::CcSwitchProviderDraft::Claude(draft) => {
            let existing = state.configuration().provider_exists(&draft);
            let usage_script_updates_existing = !existing
                && state
                    .configuration()
                    .provider_will_receive_usage_query(&draft);
            CcSwitchScanItem {
                key,
                app: AppKind::Claude,
                route_mode: draft.route_mode,
                name: draft.name,
                model: draft.model,
                base_url: draft.base_url,
                usage_script_importable: draft.usage_query.is_some(),
                usage_script_updates_existing,
                endpoint_candidates: draft.connection.custom_endpoints.len(),
                warnings,
                existing,
            }
        }
        ccswitch::CcSwitchProviderDraft::Codex(seed) => {
            let route_exists = state
                .configuration()
                .codex_route_exists(&seed.endpoint, seed.upstream);
            let route_has_query = route_exists
                && state
                    .configuration()
                    .codex_route_has_usage_query(&seed.endpoint, seed.upstream);
            CcSwitchScanItem {
                key,
                app: AppKind::Codex,
                route_mode: RouteMode::Custom,
                name: seed.name,
                model: Some(seed.default_model),
                base_url: Some(seed.endpoint.0),
                usage_script_importable: seed.usage_query.is_some(),
                // The strict store's mirror of the generic boundary: a
                // routing-identical profile gains the missing query; only a
                // query-less row or an already-queried route is a duplicate.
                usage_script_updates_existing: seed.usage_query.is_some()
                    && route_exists
                    && !route_has_query,
                endpoint_candidates: seed.connection.custom_endpoints.len(),
                warnings: seed.warnings,
                existing: route_exists && (seed.usage_query.is_none() || route_has_query),
            }
        }
        ccswitch::CcSwitchProviderDraft::CodexOfficial(draft) => {
            let existing = state.configuration().provider_exists(&draft);
            CcSwitchScanItem {
                key,
                app: AppKind::Codex,
                route_mode: RouteMode::Official,
                name: draft.name,
                model: None,
                base_url: None,
                usage_script_importable: false,
                usage_script_updates_existing: false,
                endpoint_candidates: 0,
                warnings,
                existing,
            }
        }
    }
}
