//! Strict persistence boundary for third-party Codex provider files.
//!
//! Codex files intentionally do not share Claude's `ProviderFile` schema.
//! A file that predates this boundary is rejected rather than reinterpreted.

use crate::config_store::{
    content_revision, parse_file, read_optional, write_json_atomic, ConfigStore,
    ProfileStoreError, StoreOperationError,
};
use asb_core::contracts::{
    CodexEndpoint, CodexProviderDraft, CodexProviderFile, CodexProviderRecord, CodexUpstream,
    UsageQuery,
};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use uuid::Uuid;

pub(super) struct LoadedCodexProvider {
    pub(super) file: CodexProviderFile,
    pub(super) hash: String,
}

impl ConfigStore {
    /// Every third-party Codex profile in stored order. Only the current
    /// Codex-specific file shape is accepted from this boundary.
    pub fn list_codex_providers(&self) -> Result<Vec<CodexProviderRecord>, ProfileStoreError> {
        Ok(load(self)?
            .into_iter()
            .map(|loaded| loaded.file.record(loaded.hash))
            .collect())
    }

    /// Creates one fully typed Codex provider file. The generic provider
    /// mutation API never writes into the Codex directory.
    pub fn create_codex_provider(
        &self,
        draft: CodexProviderDraft,
    ) -> Result<CodexProviderRecord, StoreOperationError> {
        let loaded = load(self)?;
        let position = loaded.iter().map(|provider| provider.file.position)
            .chain(self.list_providers()?.into_iter()
                .filter(|record| record.profile.app == asb_core::contracts::AppKind::Codex)
                .map(|record| record.position))
            .max()
            .unwrap_or(0)
            .checked_add(crate::config_store::PROVIDER_POSITION_STEP)
            .ok_or_else(|| StoreOperationError::Invalid("Codex 供应商排序位置溢出".to_string()))?;
        let file = draft.into_file(Uuid::new_v4().to_string(), position);
        let hash = write(self, &file)?;
        Ok(file.record(hash))
    }

    /// Whether a stored provider already routes through the seed's endpoint
    /// and upstream. A scan-only hint: completing the seed in the editor
    /// stays allowed even when a matching route exists.
    pub fn codex_route_exists(&self, endpoint: &CodexEndpoint, upstream: CodexUpstream) -> bool {
        load(self)
            .map(|loaded| {
                loaded.iter().any(|existing| {
                    existing.file.profile.endpoint == *endpoint
                        && existing.file.profile.upstream == upstream
                })
            })
            .unwrap_or(false)
    }

    /// Whether the stored provider routing through this endpoint and
    /// upstream already carries a usage query (scan-side input for the
    /// import hint).
    pub fn codex_route_has_usage_query(
        &self,
        endpoint: &CodexEndpoint,
        upstream: CodexUpstream,
    ) -> bool {
        load(self)
            .map(|loaded| {
                loaded.iter().any(|existing| {
                    existing.file.profile.endpoint == *endpoint
                        && existing.file.profile.upstream == upstream
                        && existing.file.usage_query.is_some()
                })
            })
            .unwrap_or(false)
    }

    /// Delivers one source usage query to the stored provider routing
    /// through this endpoint and upstream when it has none yet — the strict
    /// store's mirror of the generic import boundary's enrichment. Returns
    /// whether a profile changed; a missing route or an existing query
    /// leaves everything untouched.
    pub fn enrich_codex_route_usage_query(
        &self,
        endpoint: &CodexEndpoint,
        upstream: CodexUpstream,
        query: UsageQuery,
    ) -> Result<bool, StoreOperationError> {
        let Some(mut file) = load(self)?
            .into_iter()
            .find(|loaded| {
                loaded.file.profile.endpoint == *endpoint
                    && loaded.file.profile.upstream == upstream
            })
            .map(|loaded| loaded.file)
        else {
            return Ok(false);
        };
        if file.usage_query.is_some() {
            return Ok(false);
        }
        file.usage_query = Some(query);
        write(self, &file).map(|_| true)
    }

    pub fn update_codex_provider(
        &self,
        id: &str,
        draft: CodexProviderDraft,
        expected_file_hash: &str,
    ) -> Result<CodexProviderRecord, StoreOperationError> {
        let existing = load(self)?
            .into_iter()
            .find(|loaded| loaded.file.profile.id == id)
            .ok_or_else(|| StoreOperationError::Invalid("Codex 供应商不存在".to_string()))?;
        if existing.hash != expected_file_hash {
            return Err("Codex 供应商文件已被外部修改，请重新读取后再保存".into());
        }
        let file = draft.into_file(id.to_string(), existing.file.position);
        let hash = write(self, &file)?;
        Ok(file.record(hash))
    }

    /// Rewrites one already materialized Codex file after an optimistic
    /// revision check. Switching uses this for live backfill because the
    /// candidate must preserve every specialized field that was not present
    /// in `config.toml`.
    pub(crate) fn update_codex_provider_file(
        &self,
        file: CodexProviderFile,
        expected_file_hash: &str,
    ) -> Result<CodexProviderRecord, StoreOperationError> {
        let existing = load(self)?
            .into_iter()
            .find(|loaded| loaded.file.profile.id == file.profile.id)
            .ok_or_else(|| StoreOperationError::Invalid("Codex 供应商不存在".to_string()))?;
        if existing.hash != expected_file_hash {
            return Err("Codex 供应商文件已被外部修改，请重新读取后再保存".into());
        }
        if existing.file.position != file.position {
            return Err("Codex 供应商排序位置已变化，拒绝覆盖".into());
        }
        let hash = write(self, &file)?;
        Ok(file.record(hash))
    }

    /// Restores a previously validated Codex provider preimage. This is
    /// deliberately crate-visible: only the active-save recovery transaction
    /// and the provider-transfer import may overwrite a specialized provider
    /// file.
    pub(crate) fn overwrite_codex_provider_file(
        &self,
        file: CodexProviderFile,
    ) -> Result<(), StoreOperationError> {
        write(self, &file).map(|_| ())
    }

    pub fn delete_codex_provider(
        &self,
        id: &str,
        expected_file_hash: &str,
    ) -> Result<(), StoreOperationError> {
        let existing = load(self)?
            .into_iter()
            .find(|loaded| loaded.file.profile.id == id)
            .ok_or_else(|| StoreOperationError::Invalid("Codex 供应商不存在".to_string()))?;
        if existing.hash != expected_file_hash {
            return Err("Codex 供应商文件已被外部修改，请重新读取后再删除".into());
        }
        let path = self
            .providers_dir(asb_core::contracts::AppKind::Codex)
            .join(format!("{id}.json"));
        fs::remove_file(path)
            .map_err(|_| StoreOperationError::Invalid("无法删除 Codex 供应商文件".to_string()))
    }

    pub fn reorder_codex_profiles(
        &self,
        ordered_ids: &[String],
        expected_file_hashes: &BTreeMap<String, String>,
    ) -> Result<(), StoreOperationError> {
        let loaded = load(self)?;
        let official = self.list_providers()?
            .into_iter()
            .filter(|record| record.profile.app == asb_core::contracts::AppKind::Codex)
            .collect::<Vec<_>>();
        verify_codex_order(&loaded, &official, ordered_ids, expected_file_hashes)?;
        let mut snapshot = crate::config_store::snapshot::read_configuration_snapshot(self)?;
        let loaded_after = load(self)?;
        let official_after = self.list_providers()?
            .into_iter()
            .filter(|record| record.profile.app == asb_core::contracts::AppKind::Codex)
            .collect::<Vec<_>>();
        verify_codex_order(&loaded_after, &official_after, ordered_ids, expected_file_hashes)?;
        let current_codex_files = loaded_after
            .into_iter()
            .map(|loaded| loaded.file)
            .collect::<Vec<_>>();
        let current_official_files = crate::config_store::providers::load_provider_files(
            self,
            asb_core::contracts::AppKind::Codex,
        )?;
        if snapshot.codex_providers != current_codex_files
            || snapshot.codex_official != current_official_files {
            return Err("Codex 供应商文件已被外部修改，请重新读取后再排序".into());
        }
        for (index, id) in ordered_ids.iter().enumerate() {
            let position = (index as u64 + 1) * crate::config_store::PROVIDER_POSITION_STEP;
            if let Some(file) = snapshot
                .codex_providers
                .iter_mut()
                .find(|file| &file.profile.id == id)
            {
                file.position = position;
            } else if let Some(file) = snapshot.codex_official.iter_mut().find(|file| &file.id == id) {
                file.position = position;
            } else {
                return Err(StoreOperationError::Invalid("Codex 排序清单包含未知供应商".to_string()));
            }
        }
        snapshot.codex_providers.sort_by_key(|file| file.position);
        snapshot.codex_official.sort_by_key(|file| file.position);
        crate::config_store::snapshot::enable_snapshot(self, &snapshot)?;
        Ok(())
    }
}

fn verify_codex_order(
    third_party: &[LoadedCodexProvider],
    official: &[asb_core::contracts::ProviderRecord],
    ordered_ids: &[String],
    expected_file_hashes: &BTreeMap<String, String>,
) -> Result<(), StoreOperationError> {
    if third_party.len() + official.len() != ordered_ids.len()
        || ordered_ids.len() != expected_file_hashes.len() {
        return Err("Codex 排序版本必须覆盖全部供应商".into());
    }
    let known = third_party
        .iter()
        .map(|item| (item.file.profile.id.as_str(), item.hash.as_str()))
        .chain(official.iter().map(|record| (record.profile.id.as_str(), record.file_hash.as_str())))
        .collect::<BTreeMap<_, _>>();
    let unique = ordered_ids.iter().collect::<HashSet<_>>();
    if known.len() != expected_file_hashes.len()
        || unique.len() != ordered_ids.len()
        || ordered_ids.iter().any(|id| {
            known.get(id.as_str()).is_none_or(|hash| {
                expected_file_hashes
                    .get(id)
                    .is_none_or(|expected| *hash != expected)
            })
        })
    {
        return Err("Codex 供应商文件已被外部修改，请重新读取后再排序".into());
    }
    Ok(())
}

/// Raw Codex files for boundaries that must preserve the specialized
/// persistence contract, including configuration snapshots and recovery.
pub(crate) fn load_codex_provider_files(
    store: &ConfigStore,
) -> Result<Vec<CodexProviderFile>, ProfileStoreError> {
    Ok(load(store)?.into_iter().map(|loaded| loaded.file).collect())
}

/// Finds one current Codex file without converting it into the retired
/// generic provider shape.
impl ConfigStore {
    pub(crate) fn codex_provider_snapshots(
        &self,
    ) -> Result<Vec<(CodexProviderFile, String)>, ProfileStoreError> {
        Ok(load(self)?
            .into_iter()
            .map(|loaded| (loaded.file, loaded.hash))
            .collect())
    }

    /// Records successful use of a configured Codex fallback endpoint without
    /// changing the provider's primary endpoint or routing identity. `false`
    /// means the URL is primary or is not a configured custom target.
    pub(crate) fn mark_codex_endpoint_used(
        &self,
        id: &str,
        url: &str,
    ) -> Result<bool, StoreOperationError> {
        let mut file = load(self)?
            .into_iter()
            .find(|loaded| loaded.file.profile.id == id)
            .map(|loaded| loaded.file)
            .ok_or_else(|| StoreOperationError::Invalid("Codex 供应商不存在".to_string()))?;
        let normalized =
            asb_core::contracts::ProviderConnectionOptions::normalize_endpoint_url(url);
        let Some(key) = file
            .profile
            .connection
            .custom_endpoints
            .keys()
            .find(|candidate| {
                asb_core::contracts::ProviderConnectionOptions::normalize_endpoint_url(candidate)
                    == normalized
            })
            .cloned()
        else {
            return Ok(false);
        };
        if let Some(endpoint) = file.profile.connection.custom_endpoints.get_mut(&key) {
            endpoint.last_used = Some(chrono::Utc::now().timestamp_millis());
        }
        write(self, &file)?;
        Ok(true)
    }
    pub(crate) fn find_codex_provider_with_revision(
        &self,
        id: &str,
    ) -> Result<(CodexProviderFile, String), ProfileStoreError> {
        load(self)?
            .into_iter()
            .find(|loaded| loaded.file.profile.id == id)
            .map(|loaded| (loaded.file, loaded.hash))
            .ok_or_else(|| ProfileStoreError::NotFound { id: id.to_string() })
    }

    pub(crate) fn find_codex_provider_file(
        &self,
        id: &str,
    ) -> Result<CodexProviderFile, ProfileStoreError> {
        load(self)?
            .into_iter()
            .find(|loaded| loaded.file.profile.id == id)
            .map(|loaded| loaded.file)
            .ok_or_else(|| ProfileStoreError::NotFound { id: id.to_string() })
    }
}

pub(super) fn load(store: &ConfigStore) -> Result<Vec<LoadedCodexProvider>, ProfileStoreError> {
    store.ensure_layout()?;
    let directory = store.providers_dir(asb_core::contracts::AppKind::Codex);
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(ProfileStoreError::unreadable(&directory, &error)),
    };
    let mut loaded = Vec::new();
    for entry in entries {
        let path = entry.map_err(|error| ProfileStoreError::unreadable(&directory, &error))?.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| ProfileStoreError::invalid(&path, "供应商文件名或文件内容不存在"))?;
        let text = read_optional(&path)?.ok_or_else(|| ProfileStoreError::invalid(&path, "供应商文件名或文件内容不存在"))?;
        let file: CodexProviderFile = parse_file(&text, &path)?;
        if Uuid::parse_str(&file.profile.id).is_err() || file.profile.id != stem {
            return Err(ProfileStoreError::invalid(&path, "供应商标识不是 UUID 或与文件名不一致"));
        }
        file.validate().map_err(|error| ProfileStoreError::invalid(&path, error))?;
        loaded.push(LoadedCodexProvider {
            file,
            hash: content_revision(text.as_bytes()),
        });
    }
    loaded.sort_by(|left, right| {
        left.file
            .position
            .cmp(&right.file.position)
            .then_with(|| left.file.profile.name.cmp(&right.file.profile.name))
            .then_with(|| left.file.profile.id.cmp(&right.file.profile.id))
    });
    let mut ids = HashSet::new();
    let mut previous_position = 0;
    for provider in &loaded {
        if !ids.insert(provider.file.profile.id.as_str())
            || provider.file.position <= previous_position
        {
            return Err(ProfileStoreError::invalid(
                &directory.join(format!("{}.json", provider.file.profile.id)),
                "供应商标识重复或排序位置无效",
            ));
        }
        previous_position = provider.file.position;
    }
    Ok(loaded)
}

pub(super) fn write(
    store: &ConfigStore,
    file: &CodexProviderFile,
) -> Result<String, StoreOperationError> {
    file.validate()?;
    let json =
        serde_json::to_string_pretty(file).map_err(|_| "Codex 供应商文件序列化失败".to_string())?;
    store.initialize_current_layout()?;
    let path = store
        .providers_dir(asb_core::contracts::AppKind::Codex)
        .join(format!("{}.json", file.profile.id));
    write_json_atomic(&path, &json)?;
    Ok(content_revision(json.as_bytes()))
}

#[cfg(test)]
mod tests;

