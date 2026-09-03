//! Provider storage: exactly one JSON file per provider, named by its stable
//! UUID under `providers/{client}/`. The directory owns the client
//! association; the file owns its sort position. No index file exists.

use super::{
    content_revision, parse_strict, read_optional, write_json_atomic, ConfigStore,
    ProfileStoreError,
};
use asb_core::contracts::{
    AppKind, ProviderDraft, ProviderFile, ProviderProfile, ProviderRecord, RouteMode,
};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

const POSITION_STEP: u64 = 100;

/// One loaded provider file with its storage revision.
struct LoadedProvider {
    file: ProviderFile,
    hash: String,
}

fn load_client(
    store: &ConfigStore,
    app: AppKind,
) -> Result<Vec<LoadedProvider>, ProfileStoreError> {
    store.ensure_layout()?;
    let dir = store.providers_dir(app);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(_) => return Err(ProfileStoreError::Unreadable),
    };
    let mut loaded = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| ProfileStoreError::Unreadable)?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or(ProfileStoreError::Unsupported)?
            .to_string();
        let text = read_optional(&path)?.ok_or(ProfileStoreError::Unsupported)?;
        let file: ProviderFile = parse_strict(&text)?;
        if Uuid::parse_str(&file.id).is_err() || file.id != stem {
            return Err(ProfileStoreError::Unsupported);
        }
        let profile = file.clone().into_profile(app);
        profile
            .validate()
            .map_err(|_| ProfileStoreError::Unsupported)?;
        if let Some(query) = &file.usage_query {
            crate::usage_query::validate_persisted(query)
                .map_err(|_| ProfileStoreError::Unsupported)?;
        }
        loaded.push(LoadedProvider {
            file,
            hash: content_revision(text.as_bytes()),
        });
    }
    loaded.sort_by(|left, right| {
        left.file
            .position
            .cmp(&right.file.position)
            .then_with(|| left.file.name.cmp(&right.file.name))
            .then_with(|| left.file.id.cmp(&right.file.id))
    });
    let mut seen_ids = HashSet::new();
    let mut previous_position = 0;
    for provider in &loaded {
        if !seen_ids.insert(provider.file.id.clone()) || provider.file.position <= previous_position
        {
            return Err(ProfileStoreError::Unsupported);
        }
        previous_position = provider.file.position;
    }
    Ok(loaded)
}

/// Loads both provider directories and enforces the globally stable UUID
/// namespace used by every provider command. The directory determines the
/// client, but an id alone remains an unambiguous application identity.
fn load_all(
    store: &ConfigStore,
) -> Result<(Vec<LoadedProvider>, Vec<LoadedProvider>), ProfileStoreError> {
    let codex = load_client(store, AppKind::Codex)?;
    let claude = load_client(store, AppKind::Claude)?;
    let mut seen_ids = HashSet::new();
    for provider in codex.iter().chain(&claude) {
        if !seen_ids.insert(provider.file.id.clone()) {
            return Err(ProfileStoreError::Unsupported);
        }
    }
    Ok((codex, claude))
}

fn check_expected_files(
    loaded: &[LoadedProvider],
    expected_file_hashes: &BTreeMap<String, String>,
) -> Result<(), String> {
    if loaded.len() != expected_file_hashes.len() {
        return Err("供应商排序版本必须覆盖该客户端的全部供应商".to_string());
    }
    for provider in loaded {
        let expected = expected_file_hashes
            .get(&provider.file.id)
            .ok_or_else(|| "供应商排序版本必须覆盖该客户端的全部供应商".to_string())?;
        if expected != &provider.hash {
            return Err("供应商文件已被外部修改，请重新读取后再排序".to_string());
        }
    }
    Ok(())
}

fn record_of(app: AppKind, loaded: &LoadedProvider) -> ProviderRecord {
    ProviderRecord {
        profile: loaded.file.clone().into_profile(app),
        file_hash: loaded.hash.clone(),
    }
}

/// Every validated provider file of one client, in stored order. Snapshot
/// and migration consumers need the files, not the per-file revisions.
pub(crate) fn load_provider_files(
    store: &ConfigStore,
    app: AppKind,
) -> Result<Vec<ProviderFile>, ProfileStoreError> {
    Ok(load_client(store, app)?
        .into_iter()
        .map(|loaded| loaded.file)
        .collect())
}

/// Writes one provider file through the validating, atomic boundary and
/// returns the new storage revision.
fn write_provider_file(
    store: &ConfigStore,
    app: AppKind,
    file: &ProviderFile,
) -> Result<String, String> {
    let profile = file.clone().into_profile(app);
    profile.validate().map_err(|error| error.to_string())?;
    if let Some(query) = &file.usage_query {
        crate::usage_query::validate_persisted(query)?;
    }
    let json =
        serde_json::to_string_pretty(file).map_err(|_| "供应商文件序列化失败".to_string())?;
    let path = provider_path(store, app, &file.id);
    write_json_atomic(&path, &json)?;
    Ok(content_revision(json.as_bytes()))
}

fn provider_path(store: &ConfigStore, app: AppKind, id: &str) -> PathBuf {
    store.providers_dir(app).join(format!("{id}.json"))
}

fn next_position(loaded: &[LoadedProvider]) -> u64 {
    loaded
        .iter()
        .map(|provider| provider.file.position)
        .max()
        .unwrap_or(0)
        + POSITION_STEP
}

/// The routing identity is intentionally narrower than a complete provider:
/// a selected source import may add a missing optional usage query without
/// overwriting the existing provider's notes, website, or configured query.
fn same_provider_routing(profile: &ProviderProfile, draft: &ProviderDraft) -> bool {
    if draft.route_mode == RouteMode::Official {
        return profile.app == draft.app && profile.route_mode == RouteMode::Official;
    }
    profile.app == draft.app
        && profile.route_mode == draft.route_mode
        && profile.name == draft.name
        && profile.model == draft.model
        && profile.base_url == draft.base_url
        && profile.api_key == draft.api_key
        && profile.model_options == draft.model_options
}

fn same_provider(profile: &ProviderProfile, draft: &ProviderDraft) -> bool {
    same_provider_routing(profile, draft)
        && profile.notes == draft.notes
        && profile.website_url == draft.website_url
        && profile.usage_query == draft.usage_query
}

impl ConfigStore {
    /// Every provider of both clients, in stored order (Codex first).
    pub fn list_providers(&self) -> Result<Vec<ProviderRecord>, ProfileStoreError> {
        let (codex, claude) = load_all(self)?;
        let mut records = Vec::new();
        records.extend(codex.iter().map(|loaded| record_of(AppKind::Codex, loaded)));
        records.extend(
            claude
                .iter()
                .map(|loaded| record_of(AppKind::Claude, loaded)),
        );
        Ok(records)
    }

    fn locate(&self, id: &str) -> Result<(AppKind, LoadedProvider), String> {
        let (codex, claude) = load_all(self).map_err(|error| error.to_string())?;
        if let Some(loaded) = codex.into_iter().find(|loaded| loaded.file.id == id) {
            return Ok((AppKind::Codex, loaded));
        }
        if let Some(loaded) = claude.into_iter().find(|loaded| loaded.file.id == id) {
            return Ok((AppKind::Claude, loaded));
        }
        Err("供应商不存在".to_string())
    }

    pub fn find_provider(&self, id: &str) -> Result<ProviderProfile, String> {
        let (app, loaded) = self.locate(id)?;
        Ok(loaded.file.into_profile(app))
    }

    pub fn create_provider(&self, draft: ProviderDraft) -> Result<ProviderRecord, String> {
        draft.validate().map_err(|error| error.to_string())?;
        let (codex, claude) = load_all(self).map_err(|error| error.to_string())?;
        let existing = match draft.app {
            AppKind::Codex => codex,
            AppKind::Claude => claude,
        };
        if draft.route_mode == RouteMode::Official
            && existing
                .iter()
                .any(|provider| provider.file.route_mode == RouteMode::Official)
        {
            return Err("该客户端已有官方登录入口".to_string());
        }
        let profile = ProviderProfile::from_draft(Uuid::new_v4().to_string(), draft);
        let file = ProviderFile::from_profile(&profile, next_position(&existing));
        let hash = write_provider_file(self, profile.app, &file)?;
        Ok(ProviderRecord {
            profile,
            file_hash: hash,
        })
    }

    /// Rewrites exactly one provider file after the optimistic revision
    /// check; the id and sort position are preserved.
    pub fn update_provider(
        &self,
        id: &str,
        draft: ProviderDraft,
        expected_file_hash: &str,
    ) -> Result<ProviderRecord, String> {
        draft.validate().map_err(|error| error.to_string())?;
        let (app, loaded) = self.locate(id)?;
        if app != draft.app {
            return Err("供应商不能变更所属客户端".to_string());
        }
        if loaded.hash != expected_file_hash {
            return Err("供应商文件已被外部修改，请重新读取后再保存".to_string());
        }
        if draft.route_mode == RouteMode::Official {
            let (codex, claude) = load_all(self).map_err(|error| error.to_string())?;
            let siblings = match app {
                AppKind::Codex => codex,
                AppKind::Claude => claude,
            };
            if siblings.iter().any(|candidate| {
                candidate.file.id != id && candidate.file.route_mode == RouteMode::Official
            }) {
                return Err("该客户端已有官方登录入口".to_string());
            }
        }
        let profile = ProviderProfile::from_draft(id.to_string(), draft);
        let file = ProviderFile::from_profile(&profile, loaded.file.position);
        let hash = write_provider_file(self, app, &file)?;
        Ok(ProviderRecord {
            profile,
            file_hash: hash,
        })
    }

    pub fn delete_provider(&self, id: &str, expected_file_hash: &str) -> Result<(), String> {
        let (app, loaded) = self.locate(id)?;
        if loaded.hash != expected_file_hash {
            return Err("供应商文件已被外部修改，请重新读取后再删除".to_string());
        }
        fs::remove_file(provider_path(self, app, &loaded.file.id))
            .map_err(|_| "无法删除供应商文件".to_string())
    }

    /// Persists a drag reorder as new `position` values in the affected
    /// provider files. The list must cover the client's providers exactly.
    pub fn reorder_providers(
        &self,
        app: AppKind,
        ordered_ids: &[String],
        expected_file_hashes: &BTreeMap<String, String>,
    ) -> Result<Vec<ProviderRecord>, String> {
        let (codex, claude) = load_all(self).map_err(|error| error.to_string())?;
        let loaded = match app {
            AppKind::Codex => codex,
            AppKind::Claude => claude,
        };
        check_expected_files(&loaded, expected_file_hashes)?;
        let unique: HashSet<&str> = ordered_ids.iter().map(String::as_str).collect();
        if unique.len() != ordered_ids.len() || ordered_ids.len() != loaded.len() {
            return Err("排序清单必须覆盖该客户端的全部供应商且不得重复".to_string());
        }
        // Re-read the complete snapshot and the raw file revisions before
        // constructing the replacement. This rejects a concurrent/manual
        // provider edit (including a formatting-only edit) instead of
        // replacing it blindly.
        let mut snapshot = super::snapshot::read_configuration_snapshot(self)
            .map_err(|error| error.to_string())?;
        let (codex_after, claude_after) = load_all(self).map_err(|error| error.to_string())?;
        let loaded_after = match app {
            AppKind::Codex => codex_after,
            AppKind::Claude => claude_after,
        };
        check_expected_files(&loaded_after, expected_file_hashes)?;
        let current_files: Vec<ProviderFile> =
            loaded_after.into_iter().map(|loaded| loaded.file).collect();
        if snapshot.providers[&app] != current_files {
            return Err("供应商文件已被外部修改，请重新读取后再排序".to_string());
        }
        for id in ordered_ids {
            if !loaded.iter().any(|loaded| &loaded.file.id == id) {
                return Err(format!("排序清单包含不属于该客户端的供应商：{id}"));
            }
        }
        // A reorder is one logical mutation. Rebuild it through the same
        // verified directory replacement used by restore, so a later file
        // failure cannot leave a subset of positions persisted.
        let files = snapshot
            .providers
            .get_mut(&app)
            .expect("complete snapshot always has both clients");
        for (order, id) in ordered_ids.iter().enumerate() {
            let file = files
                .iter_mut()
                .find(|file| &file.id == id)
                .expect("validated ordering ids exist in the snapshot");
            file.position = (order as u64 + 1) * POSITION_STEP;
        }
        files.sort_by_key(|file| file.position);
        super::snapshot::enable_snapshot(self, &snapshot)?;
        self.list_providers().map_err(|error| error.to_string())
    }

    /// Imports one draft as a provider file: an exactly equal provider is a
    /// no-op, a routing-identical one gains the missing usage query, and
    /// anything else creates a new file.
    pub fn import_provider(&self, draft: ProviderDraft) -> Result<ProviderRecord, String> {
        draft.validate().map_err(|error| error.to_string())?;
        let (codex, claude) = load_all(self).map_err(|error| error.to_string())?;
        let existing = match draft.app {
            AppKind::Codex => codex,
            AppKind::Claude => claude,
        };
        if let Some(loaded) = existing
            .iter()
            .find(|loaded| same_provider(&loaded.file.clone().into_profile(draft.app), &draft))
        {
            return Ok(record_of(draft.app, loaded));
        }
        if let Some(query) = draft.usage_query.clone() {
            let target = existing.iter().find(|loaded| {
                same_provider_routing(&loaded.file.clone().into_profile(draft.app), &draft)
                    && loaded.file.usage_query.is_none()
            });
            if let Some(loaded) = target {
                let mut file = loaded.file.clone();
                file.usage_query = Some(query);
                let hash = write_provider_file(self, draft.app, &file)?;
                return Ok(ProviderRecord {
                    profile: file.into_profile(draft.app),
                    file_hash: hash,
                });
            }
        }
        self.create_provider(draft)
    }

    /// Whether an exactly equal provider already exists (scan-side view of
    /// the import dedup rule).
    pub fn provider_exists(&self, draft: &ProviderDraft) -> bool {
        match load_all(self) {
            Ok((codex, claude)) => {
                let existing = match draft.app {
                    AppKind::Codex => codex,
                    AppKind::Claude => claude,
                };
                existing.iter().any(|loaded| {
                    same_provider(&loaded.file.clone().into_profile(draft.app), draft)
                })
            }
            Err(_) => false,
        }
    }

    /// Whether a selected source import will enrich its otherwise matching
    /// local provider with a currently absent usage query.
    pub fn provider_will_receive_usage_query(&self, draft: &ProviderDraft) -> bool {
        if draft.usage_query.is_none() {
            return false;
        }
        match load_all(self) {
            Ok((codex, claude)) => {
                let existing = match draft.app {
                    AppKind::Codex => codex,
                    AppKind::Claude => claude,
                };
                existing.iter().any(|loaded| {
                    same_provider_routing(&loaded.file.clone().into_profile(draft.app), draft)
                        && loaded.file.usage_query.is_none()
                })
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::UsageQuery;

    fn codex_draft(name: &str) -> ProviderDraft {
        ProviderDraft {
            app: AppKind::Codex,
            route_mode: RouteMode::Custom,
            name: name.to_string(),
            model: Some("gpt-5.3-codex".to_string()),
            base_url: Some("https://gateway.example/v1".to_string()),
            api_key: "OPENAI_API_KEY".to_string(),
            model_options: None,
            notes: None,
            website_url: None,
            usage_query: None,
        }
    }

    fn claude_draft(name: &str) -> ProviderDraft {
        ProviderDraft {
            app: AppKind::Claude,
            route_mode: RouteMode::Custom,
            name: name.to_string(),
            model: None,
            base_url: Some("https://claude-relay.example".to_string()),
            api_key: "test-api-key".to_string(),
            model_options: None,
            notes: None,
            website_url: None,
            usage_query: None,
        }
    }

    fn official_draft(app: AppKind) -> ProviderDraft {
        ProviderDraft {
            app,
            route_mode: RouteMode::Official,
            name: match app {
                AppKind::Codex => "Codex 官方登录",
                AppKind::Claude => "Claude 官方登录",
            }
            .to_string(),
            model: None,
            base_url: None,
            api_key: String::new(),
            model_options: None,
            notes: None,
            website_url: None,
            usage_query: None,
        }
    }

    fn store() -> (tempfile::TempDir, ConfigStore) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));
        (directory, store)
    }

    fn provider_files(store: &ConfigStore, app: AppKind) -> Vec<PathBuf> {
        let entries = match fs::read_dir(store.providers_dir(app)) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return vec![],
            Err(error) => panic!("provider dir: {error}"),
        };
        let mut paths: Vec<PathBuf> = entries
            .map(|entry| entry.expect("entry").path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
            .collect();
        paths.sort();
        paths
    }

    /// Reads the position straight out of the named provider file.
    fn stored_position(store: &ConfigStore, app: AppKind, id: &str) -> u64 {
        let path = store.providers_dir(app).join(format!("{id}.json"));
        let file: ProviderFile = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        file.position
    }

    fn file_for(store: &ConfigStore, app: AppKind, id: &str) -> PathBuf {
        store.providers_dir(app).join(format!("{id}.json"))
    }

    fn revisions(records: &[ProviderRecord], app: AppKind) -> BTreeMap<String, String> {
        records
            .iter()
            .filter(|record| record.profile.app == app)
            .map(|record| (record.profile.id.clone(), record.file_hash.clone()))
            .collect()
    }

    #[test]
    fn legacy_usage_query_files_are_upgraded_once_to_explicit_manual_only() {
        let (directory, store) = store();
        let mut draft = codex_draft("旧查询档案");
        draft.usage_query = Some(UsageQuery::Declarative {
            url: "{{baseUrl}}/usage".to_string(),
            remaining_path: Some("/remaining".to_string()),
            used_path: None,
            total_path: None,
            unit: None,
            refresh_interval_minutes: 30,
        });
        store.create_provider(draft).expect("create provider");
        let path = provider_files(&store, AppKind::Codex).remove(0);

        // Rewind the stored file to the previous contract: the interval line
        // simply did not exist.
        let current = fs::read_to_string(&path).unwrap();
        let legacy = current
            .replace("  \"refreshIntervalMinutes\": 30,\n", "")
            .replace(",\n    \"refreshIntervalMinutes\": 30", "");
        assert_ne!(legacy, current, "fixture must remove the interval field");
        fs::write(&path, &legacy).unwrap();

        // Loading upgrades exactly that shape, persists the explicit 0, and
        // keeps reporting the query's own configured position in the file.
        let records = store.list_providers().unwrap();
        assert_eq!(
            records[0]
                .profile
                .usage_query
                .as_ref()
                .unwrap()
                .refresh_interval_minutes(),
            0
        );
        let upgraded = fs::read_to_string(&path).unwrap();
        assert!(upgraded.contains("\"refreshIntervalMinutes\": 0"));

        // A second load is a plain strict read; the file no longer changes.
        let before = upgraded.clone();
        store.list_providers().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), before);
        drop(directory);
    }

    #[test]
    fn unknown_usage_query_shapes_stay_rejected_without_a_write() {
        let (directory, store) = store();
        let mut draft = codex_draft("坏查询档案");
        draft.usage_query = Some(UsageQuery::Script {
            source: "({ request() {}, extract() {} })".to_string(),
            refresh_interval_minutes: 0,
        });
        store.create_provider(draft).expect("create provider");
        let path = provider_files(&store, AppKind::Codex).remove(0);

        let current = fs::read_to_string(&path).unwrap();
        // An unknown key is not the previous shape; the upgrade must refuse
        // to touch the file and the load must fail loudly.
        let unknown = current.replace("\"refreshIntervalMinutes\": 0", "\"legacyTimer\": true");
        assert_ne!(unknown, current);
        fs::write(&path, &unknown).unwrap();

        assert!(store.list_providers().is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), unknown);
        drop(directory);
    }

    #[test]
    fn create_writes_one_uuid_named_file_per_provider() {
        let (_dir, store) = store();
        let first = store.create_provider(codex_draft("网关 A")).expect("first");
        let second = store
            .create_provider(codex_draft("网关 B"))
            .expect("second");
        store
            .create_provider(claude_draft("Claude 中继"))
            .expect("claude");

        assert!(Uuid::parse_str(&first.profile.id).is_ok());
        let codex_files = provider_files(&store, AppKind::Codex);
        assert_eq!(codex_files.len(), 2);
        assert_eq!(
            stored_position(&store, AppKind::Codex, &first.profile.id),
            100
        );
        assert_eq!(
            stored_position(&store, AppKind::Codex, &second.profile.id),
            200
        );
        assert_eq!(provider_files(&store, AppKind::Claude).len(), 1);

        // The file carries no client field.
        let text = fs::read_to_string(file_for(&store, AppKind::Codex, &first.profile.id)).unwrap();
        assert!(!text.contains("\"app\""));
    }

    #[test]
    fn update_preserves_position_and_rejects_external_changes() {
        let (_dir, store) = store();
        let created = store.create_provider(codex_draft("网关")).expect("create");
        let updated = store
            .update_provider(
                &created.profile.id,
                codex_draft("改名网关"),
                &created.file_hash,
            )
            .expect("update");
        assert_ne!(updated.file_hash, created.file_hash);
        assert_eq!(
            stored_position(&store, AppKind::Codex, &created.profile.id),
            100
        );
        assert_eq!(store.list_providers().unwrap()[0].profile.name, "改名网关");

        let error = store
            .update_provider(&created.profile.id, codex_draft("外部改过"), "stale-hash")
            .expect_err("stale hash must fail");
        assert!(error.contains("外部修改"));
        assert_eq!(store.list_providers().unwrap()[0].profile.name, "改名网关");
    }

    #[test]
    fn delete_removes_only_that_file() {
        let (_dir, store) = store();
        let first = store.create_provider(codex_draft("网关 A")).expect("first");
        let second = store
            .create_provider(codex_draft("网关 B"))
            .expect("second");
        store
            .delete_provider(&first.profile.id, &first.file_hash)
            .expect("delete");

        let records = store.list_providers().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].profile.id, second.profile.id);
    }

    #[test]
    fn reorder_rewrites_positions_for_that_client_only() {
        let (_dir, store) = store();
        let a = store.create_provider(codex_draft("网关 A")).expect("A");
        let claude = store
            .create_provider(claude_draft("Claude 中继"))
            .expect("claude");
        let b = store.create_provider(codex_draft("网关 B")).expect("B");
        let claude_hash_before =
            fs::read_to_string(file_for(&store, AppKind::Claude, &claude.profile.id)).unwrap();
        let before = store.list_providers().unwrap();

        let reordered = store
            .reorder_providers(
                AppKind::Codex,
                &[b.profile.id.clone(), a.profile.id.clone()],
                &revisions(&before, AppKind::Codex),
            )
            .expect("reorder");
        let codex_order: Vec<&str> = reordered
            .iter()
            .filter(|record| record.profile.app == AppKind::Codex)
            .map(|record| record.profile.name.as_str())
            .collect();
        assert_eq!(codex_order, vec!["网关 B", "网关 A"]);
        assert!(reordered
            .iter()
            .any(|record| record.profile.id == claude.profile.id));
        let claude_hash_after =
            fs::read_to_string(file_for(&store, AppKind::Claude, &claude.profile.id)).unwrap();
        assert_eq!(claude_hash_before, claude_hash_after);

        let error = store
            .reorder_providers(
                AppKind::Codex,
                &[a.profile.id.clone()],
                &revisions(&reordered, AppKind::Codex),
            )
            .expect_err("incomplete list must fail");
        assert!(error.contains("不得重复") || error.contains("一一对应"));
    }

    #[test]
    fn delete_and_reorder_reject_stale_provider_revisions() {
        let (_dir, store) = store();
        let first = store.create_provider(codex_draft("网关 A")).unwrap();
        let second = store.create_provider(codex_draft("网关 B")).unwrap();

        let first_path = file_for(&store, AppKind::Codex, &first.profile.id);
        fs::write(
            &first_path,
            fs::read_to_string(&first_path)
                .unwrap()
                .replace("网关 A", "外部修改"),
        )
        .unwrap();
        let delete_error = store
            .delete_provider(&first.profile.id, &first.file_hash)
            .expect_err("external provider edit must block delete");
        assert!(delete_error.contains("外部修改"));
        assert!(first_path.exists());

        let stale = BTreeMap::from([
            (first.profile.id.clone(), first.file_hash.clone()),
            (second.profile.id.clone(), second.file_hash.clone()),
        ]);
        let reorder_error = store
            .reorder_providers(
                AppKind::Codex,
                &[second.profile.id.clone(), first.profile.id.clone()],
                &stale,
            )
            .expect_err("external provider edit must block reorder");
        assert!(reorder_error.contains("外部修改"));
        assert_eq!(
            stored_position(&store, AppKind::Codex, &first.profile.id),
            100
        );
        assert_eq!(
            stored_position(&store, AppKind::Codex, &second.profile.id),
            200
        );
    }

    #[test]
    fn duplicate_ids_or_positions_in_provider_files_are_rejected() {
        let (_dir, store) = store();
        let first = store.create_provider(codex_draft("网关 A")).unwrap();
        let second = store.create_provider(codex_draft("网关 B")).unwrap();
        let second_path = file_for(&store, AppKind::Codex, &second.profile.id);
        let mut second_file: ProviderFile =
            serde_json::from_str(&fs::read_to_string(&second_path).unwrap()).unwrap();
        second_file.position = 100;
        fs::write(&second_path, serde_json::to_string(&second_file).unwrap()).unwrap();
        assert_eq!(store.list_providers(), Err(ProfileStoreError::Unsupported));

        fs::remove_file(&second_path).unwrap();
        let claude_path = file_for(&store, AppKind::Claude, &first.profile.id);
        fs::create_dir_all(claude_path.parent().unwrap()).unwrap();
        let mut duplicate: ProviderFile = serde_json::from_str(
            &fs::read_to_string(file_for(&store, AppKind::Codex, &first.profile.id)).unwrap(),
        )
        .unwrap();
        duplicate.position = 100;
        fs::write(&claude_path, serde_json::to_string(&duplicate).unwrap()).unwrap();
        assert_eq!(store.list_providers(), Err(ProfileStoreError::Unsupported));
    }

    #[test]
    fn import_is_idempotent_and_enriches_a_routing_match_with_a_query() {
        let (_dir, store) = store();
        let first = store
            .import_provider(codex_draft("导入网关"))
            .expect("first");
        let second = store
            .import_provider(codex_draft("导入网关"))
            .expect("second");
        assert_eq!(first.profile.id, second.profile.id);
        assert_eq!(store.list_providers().unwrap().len(), 1);

        let mut with_query = codex_draft("导入网关");
        with_query.usage_query = Some(UsageQuery::Script {
            source: r#"({
                request() { return { url: "https://gateway.example/usage", method: "GET" }; },
                extract() { return { remaining: 1, unit: "USD" }; }
            })"#
            .to_string(),
            refresh_interval_minutes: 0,
        });
        assert!(!store.provider_exists(&with_query));
        assert!(store.provider_will_receive_usage_query(&with_query));
        let updated = store.import_provider(with_query.clone()).expect("enrich");
        assert_eq!(updated.profile.id, first.profile.id);
        assert_eq!(updated.profile.usage_query, with_query.usage_query);
        assert_eq!(store.list_providers().unwrap().len(), 1);
    }

    #[test]
    fn official_import_is_idempotent_per_client_and_never_needs_a_credential() {
        let (_dir, store) = store();
        let codex = store
            .import_provider(official_draft(AppKind::Codex))
            .expect("official Codex route");
        let duplicate = store
            .import_provider(official_draft(AppKind::Codex))
            .expect("same official route");
        let claude = store
            .import_provider(official_draft(AppKind::Claude))
            .expect("official Claude route");

        assert_eq!(codex.profile.id, duplicate.profile.id);
        assert_eq!(codex.profile.route_mode, RouteMode::Official);
        assert!(codex.profile.api_key.is_empty());
        assert_eq!(claude.profile.route_mode, RouteMode::Official);
        assert_eq!(store.list_providers().unwrap().len(), 2);
    }

    #[test]
    fn invalid_usage_scripts_never_reach_a_file() {
        let (_dir, store) = store();
        let mut draft = codex_draft("坏脚本");
        draft.usage_query = Some(UsageQuery::Script {
            source: "({ request() {} })".to_string(),
            refresh_interval_minutes: 0,
        });
        assert!(store.create_provider(draft).is_err());
        assert_eq!(provider_files(&store, AppKind::Codex).len(), 0);
    }

    #[test]
    fn foreign_files_are_rejected_loudly() {
        let (_dir, store) = store();
        store.create_provider(codex_draft("网关")).expect("create");
        let path = provider_files(&store, AppKind::Codex)
            .into_iter()
            .next()
            .unwrap();
        fs::write(&path, "{\"id\":\"not-a-uuid\",\"name\":\"x\"}").unwrap();
        assert!(matches!(
            store.list_providers().expect_err("bad file must fail"),
            ProfileStoreError::Migration(_)
        ));
    }
}
