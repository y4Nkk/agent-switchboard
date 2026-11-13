use super::{apply_and_parse, ProviderSqlImportOutcome, ProviderSqlSkip};
use crate::config_store::{providers::load_provider_files, ConfigStore, StoreOperationError, PROVIDER_POSITION_STEP};
use crate::local_state::LocalState;
use asb_core::contracts::{AppKind, CodexProviderFile, ProviderFile, RouteMode};
use asb_core::provider_transfer::TransferProvider;
use std::collections::HashSet;

/// Re-applies the preview's file, then preserves existing positions while
/// appending new records in exported order through the typed store boundary.
pub fn import(
    state: &LocalState,
    ids: &[String],
    sql_path: &str,
) -> Result<ProviderSqlImportOutcome, StoreOperationError> {
    let (rows, skipped) = apply_and_parse(state, sql_path.trim())?;
    let requested: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let mut batch = ImportBatch::new(state)?;
    for row in &rows {
        if !requested.contains(row.id.as_str()) { continue; }
        match &row.provider {
            TransferProvider::Claude(source) => batch.write_claude(&row.id, source)?,
            TransferProvider::CodexOfficial(source) => batch.write_official(&row.id, source)?,
            TransferProvider::CodexCustom(source) => batch.write_custom(&row.id, source)?,
        }
    }
    for id in ids {
        if rows.iter().any(|row| row.id == *id) { continue; }
        let reason = skipped.iter().find(|skip| skip.name == *id)
            .map(|skip| skip.reason.clone())
            .unwrap_or_else(|| "预览结果已变化，请重新选择文件".to_string());
        batch.outcome.not_imported.push(ProviderSqlSkip { name: id.clone(), reason });
    }
    Ok(batch.outcome)
}

struct ImportBatch {
    configuration: ConfigStore,
    claude: Vec<ProviderFile>,
    official: Vec<ProviderFile>,
    custom: Vec<(CodexProviderFile, String)>,
    claude_next: u64,
    codex_next: u64,
    outcome: ProviderSqlImportOutcome,
}

impl ImportBatch {
    fn new(state: &LocalState) -> Result<Self, StoreOperationError> {
        let configuration = state.configuration();
        let claude = load_provider_files(&configuration, AppKind::Claude)?;
        let official = load_provider_files(&configuration, AppKind::Codex)?;
        let custom = configuration.codex_provider_snapshots()?;
        let claude_next = claude.iter().map(|file| file.position).max().unwrap_or(0);
        let codex_next = official.iter().map(|file| file.position)
            .chain(custom.iter().map(|(file, _)| file.position)).max().unwrap_or(0);
        Ok(Self { configuration, claude, official, custom, claude_next, codex_next,
            outcome: ProviderSqlImportOutcome { imported_count: 0, updated_count: 0, not_imported: Vec::new() } })
    }

    fn write_claude(&mut self, id: &str, source: &ProviderFile) -> Result<(), StoreOperationError> {
        let existing = self.claude.iter().find(|stored| stored.id == id);
        // An imported official singleton never replaces this device's own.
        if existing.is_none() && source.route_mode == RouteMode::Official
            && self.claude.iter().any(|stored| stored.route_mode == RouteMode::Official)
        {
            self.outcome.not_imported.push(ProviderSqlSkip {
                name: source.name.clone(), reason: "本机已有其他 Claude 官方登录记录".to_string(),
            });
            return Ok(());
        }
        let mut file = source.clone();
        match existing {
            Some(stored) => file.position = stored.position,
            None => { self.claude_next += PROVIDER_POSITION_STEP; file.position = self.claude_next; }
        }
        self.configuration.overwrite_provider_file(AppKind::Claude, file)?;
        if existing.is_some() { self.outcome.updated_count += 1; }
        else { self.outcome.imported_count += 1; }
        Ok(())
    }

    fn write_official(&mut self, id: &str, source: &ProviderFile) -> Result<(), StoreOperationError> {
        let mut file = source.clone();
        if let Some(existing) = self.official.iter().find(|stored| stored.id == id) {
            file.position = existing.position;
            self.configuration.overwrite_provider_file(AppKind::Codex, file)?;
            self.outcome.updated_count += 1;
        } else if self.official.iter().any(|stored| stored.route_mode == RouteMode::Official) {
            self.outcome.not_imported.push(ProviderSqlSkip {
                name: file.name.clone(), reason: "本机已有其他官方登录记录".to_string(),
            });
        } else {
            self.codex_next += PROVIDER_POSITION_STEP;
            file.position = self.codex_next;
            self.configuration.overwrite_provider_file(AppKind::Codex, file)?;
            self.outcome.imported_count += 1;
        }
        Ok(())
    }

    fn write_custom(&mut self, id: &str, source: &CodexProviderFile) -> Result<(), StoreOperationError> {
        let existing = self.custom.iter().find(|(stored, _)| stored.profile.id == id);
        let mut file = source.clone();
        match existing {
            Some((stored, _)) => file.position = stored.position,
            None => { self.codex_next += PROVIDER_POSITION_STEP; file.position = self.codex_next; }
        }
        self.configuration.overwrite_codex_provider_file(file)?;
        if existing.is_some() { self.outcome.updated_count += 1; }
        else { self.outcome.imported_count += 1; }
        Ok(())
    }
}
