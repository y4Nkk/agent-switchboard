use super::load::{
    check_expected_files, load_all, next_position, provider_path, record_of, same_provider,
    same_provider_routing, write_provider_file, LoadedProvider, POSITION_STEP,
};
use crate::config_store::{ConfigStore, ProfileStoreError, StoreOperationError};
use asb_core::contracts::{
    AppKind, ProviderDraft, ProviderEndpoint, ProviderFile, ProviderProfile, ProviderRecord,
    RouteMode,
};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use uuid::Uuid;

impl ConfigStore {
    /// Every provider of the generic store: all Claude providers plus the
    /// Codex official-login records, each group in stored order.
    pub fn list_providers(&self) -> Result<Vec<ProviderRecord>, ProfileStoreError> {
        let (codex, claude) = load_all(self)?;
        let mut records = claude
            .iter()
            .map(|loaded| record_of(AppKind::Claude, loaded))
            .collect::<Vec<_>>();
        records.extend(codex.iter().map(|loaded| record_of(AppKind::Codex, loaded)));
        Ok(records)
    }

    fn locate(&self, id: &str) -> Result<(AppKind, LoadedProvider), StoreOperationError> {
        let (codex, claude) = load_all(self)?;
        if let Some(loaded) = claude.into_iter().find(|loaded| loaded.file.id == id) {
            return Ok((AppKind::Claude, loaded));
        }
        if let Some(loaded) = codex.into_iter().find(|loaded| loaded.file.id == id) {
            return Ok((AppKind::Codex, loaded));
        }
        Err("供应商不存在".into())
    }

    pub fn find_provider(&self, id: &str) -> Result<ProviderProfile, StoreOperationError> {
        let (app, loaded) = self.locate(id)?;
        Ok(loaded.file.into_profile(app))
    }

    /// One provider together with its current storage revision.
    pub fn find_provider_record(&self, id: &str) -> Result<ProviderRecord, StoreOperationError> {
        let (app, loaded) = self.locate(id)?;
        Ok(record_of(app, &loaded))
    }

    /// The exact persisted file of one provider, including its sort
    /// position. The save transaction keeps it as the in-memory pre-image
    /// used to roll a partially applied save back.
    pub fn load_provider_file(
        &self,
        id: &str,
    ) -> Result<(AppKind, ProviderFile), StoreOperationError> {
        let (app, loaded) = self.locate(id)?;
        Ok((app, loaded.file))
    }

    /// Rewrites one provider file through the validating atomic boundary,
    /// preserving its stored position. The save transaction's rollback path
    /// and the provider-transfer import call this, always with a file that
    /// was previously persisted by this application or exported by it.
    pub fn overwrite_provider_file(
        &self,
        app: AppKind,
        file: ProviderFile,
    ) -> Result<String, StoreOperationError> {
        write_provider_file(self, app, &file)
    }

    pub fn create_provider(
        &self,
        id: String,
        draft: ProviderDraft,
    ) -> Result<ProviderRecord, StoreOperationError> {
        self.validate_new_provider_id(&id)?;
        if draft.app == AppKind::Codex && draft.route_mode != RouteMode::Official {
            return Err("Codex 第三方供应商必须使用专用档案格式".into());
        }
        draft.validate().map_err(|error| error.to_string())?;
        let (codex, claude) = load_all(self)?;
        let existing = match draft.app {
            AppKind::Codex => codex,
            AppKind::Claude => claude,
        };
        if draft.route_mode == RouteMode::Official
            && existing
                .iter()
                .any(|provider| provider.file.route_mode == RouteMode::Official)
        {
            return Err("该客户端已有官方登录入口".into());
        }
        let profile = ProviderProfile::from_draft(id, draft);
        crate::gateway::claude_targets::validate_references(self, &profile)?;
        let position = if profile.app == AppKind::Codex {
            self.list_codex_providers()?
                .iter()
                .map(|record| record.position)
                .chain(existing.iter().map(|provider| provider.file.position))
                .max()
                .unwrap_or(0)
                .checked_add(POSITION_STEP)
                .ok_or_else(|| StoreOperationError::Invalid("供应商排序位置溢出".to_string()))?
        } else {
            next_position(&existing)
        };
        let file = ProviderFile::from_profile(&profile, position);
        let hash = write_provider_file(self, profile.app, &file)?;
        Ok(ProviderRecord {
            profile,
            position,
            file_hash: hash,
        })
    }

    /// The Codex official-login record, creating the canonical one when
    /// absent. Completing an official login must leave official routing
    /// enableable, so every login path can call this idempotently; an
    /// existing record — including a user-customized one — is returned
    /// untouched. The bool says whether this call created the record.
    pub fn ensure_codex_official_record(
        &self,
    ) -> Result<(ProviderRecord, bool), StoreOperationError> {
        let (codex, _) = load_all(self)?;
        if let Some(loaded) = codex
            .iter()
            .find(|loaded| loaded.file.route_mode == RouteMode::Official)
        {
            return Ok((record_of(AppKind::Codex, loaded), false));
        }
        self.create_provider(Uuid::new_v4().to_string(), asb_core::codex_official_draft(
            asb_core::ownership::default_provider_parameters(AppKind::Codex),
        ))
        .map(|record| (record, true))
    }

    /// Rewrites exactly one provider file after the optimistic revision
    /// check; the id and sort position are preserved.
    pub fn update_provider(
        &self,
        id: &str,
        draft: ProviderDraft,
        expected_file_hash: &str,
    ) -> Result<ProviderRecord, StoreOperationError> {
        if draft.app == AppKind::Codex && draft.route_mode != RouteMode::Official {
            return Err("Codex 第三方供应商必须使用专用档案格式".into());
        }
        draft.validate().map_err(|error| error.to_string())?;
        let (app, loaded) = self.locate(id)?;
        if app != draft.app {
            return Err("供应商不能变更所属客户端".into());
        }
        if loaded.hash != expected_file_hash {
            return Err("供应商文件已被外部修改，请重新读取后再保存".into());
        }
        if draft.route_mode == RouteMode::Official {
            let (codex, claude) = load_all(self)?;
            let siblings = match app {
                AppKind::Codex => codex,
                AppKind::Claude => claude,
            };
            if siblings.iter().any(|candidate| {
                candidate.file.id != id && candidate.file.route_mode == RouteMode::Official
            }) {
                return Err("该客户端已有官方登录入口".into());
            }
        }
        let profile = ProviderProfile::from_draft(id.to_string(), draft);
        crate::gateway::claude_targets::validate_references(self, &profile)?;
        let file = ProviderFile::from_profile(&profile, loaded.file.position);
        let hash = write_provider_file(self, app, &file)?;
        Ok(ProviderRecord {
            profile,
            position: file.position,
            file_hash: hash,
        })
    }

    pub fn delete_provider(
        &self,
        id: &str,
        expected_file_hash: &str,
    ) -> Result<(), StoreOperationError> {
        let (app, loaded) = self.locate(id)?;
        if loaded.hash != expected_file_hash {
            return Err("供应商文件已被外部修改，请重新读取后再删除".into());
        }
        for record in self.list_providers()? {
            if record.profile.id != id && crate::gateway::claude_targets::references(&record.profile, id) {
                return Err(format!("供应商「{}」的用途路由正在引用此档案，请先解除引用", record.profile.name).into());
            }
        }
        fs::remove_file(provider_path(self, app, &loaded.file.id))
            .map_err(|_| StoreOperationError::Invalid("无法删除供应商文件".to_string()))
    }

    /// Adds one Claude custom upstream target while preserving all unrelated
    /// provider fields and guarding the file with its current revision.
    pub fn add_provider_endpoint(
        &self,
        id: &str,
        url: &str,
        expected_file_hash: &str,
    ) -> Result<ProviderRecord, StoreOperationError> {
        let (app, loaded) = self.endpoint_target(id, expected_file_hash)?;
        let normalized =
            asb_core::contracts::ProviderConnectionOptions::normalize_endpoint_url(url);
        if normalized.is_empty() {
            return Err("供应商服务端点不能为空".into());
        }
        if loaded
            .file
            .connection
            .custom_endpoints
            .keys()
            .any(|candidate| {
                asb_core::contracts::ProviderConnectionOptions::normalize_endpoint_url(candidate)
                    == normalized
            })
        {
            return Err("供应商服务端点已存在".into());
        }
        let mut file = loaded.file;
        file.connection.custom_endpoints.insert(
            normalized.clone(),
            ProviderEndpoint {
                url: normalized,
                added_at: chrono::Utc::now().timestamp_millis(),
                last_used: None,
            },
        );
        let hash = write_provider_file(self, app, &file)?;
        let position = file.position;
        Ok(ProviderRecord {
            profile: file.into_profile(app),
            position,
            file_hash: hash,
        })
    }

    /// Removes one Claude custom upstream target. A trailing slash is treated
    /// as equivalent to the canonical endpoint spelling used on save.
    pub fn remove_provider_endpoint(
        &self,
        id: &str,
        url: &str,
        expected_file_hash: &str,
    ) -> Result<ProviderRecord, StoreOperationError> {
        let (app, loaded) = self.endpoint_target(id, expected_file_hash)?;
        let normalized =
            asb_core::contracts::ProviderConnectionOptions::normalize_endpoint_url(url);
        let key = loaded
            .file
            .connection
            .custom_endpoints
            .keys()
            .find(|candidate| {
                asb_core::contracts::ProviderConnectionOptions::normalize_endpoint_url(candidate)
                    == normalized
            })
            .cloned()
            .ok_or_else(|| StoreOperationError::Invalid("供应商服务端点不存在".to_string()))?;
        let mut file = loaded.file;
        file.connection.custom_endpoints.remove(&key);
        let hash = write_provider_file(self, app, &file)?;
        let position = file.position;
        Ok(ProviderRecord {
            profile: file.into_profile(app),
            position,
            file_hash: hash,
        })
    }

    /// Records successful use of a Claude custom endpoint without changing the
    /// provider's routing identity. `false` means the URL is the primary
    /// endpoint or is not a configured custom target.
    pub fn mark_claude_endpoint_used(
        &self,
        id: &str,
        url: &str,
    ) -> Result<bool, StoreOperationError> {
        let (app, loaded) = self.locate(id)?;
        if app != AppKind::Claude || loaded.file.route_mode != RouteMode::Custom {
            return Ok(false);
        }
        let normalized =
            asb_core::contracts::ProviderConnectionOptions::normalize_endpoint_url(url);
        let Some(key) = loaded
            .file
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
        let mut file = loaded.file;
        if let Some(endpoint) = file.connection.custom_endpoints.get_mut(&key) {
            endpoint.last_used = Some(chrono::Utc::now().timestamp_millis());
        }
        write_provider_file(self, app, &file)?;
        Ok(true)
    }

    fn endpoint_target(
        &self,
        id: &str,
        expected_file_hash: &str,
    ) -> Result<(AppKind, LoadedProvider), StoreOperationError> {
        let (app, loaded) = self.locate(id)?;
        if app != AppKind::Claude || loaded.file.route_mode != RouteMode::Custom {
            return Err("只有 Claude 自定义供应商支持服务端点".into());
        }
        if loaded.hash != expected_file_hash {
            return Err("供应商文件已被外部修改，请重新读取后再保存".into());
        }
        Ok((app, loaded))
    }

    /// Persists a drag reorder as new `position` values in every Claude
    /// provider file. The list must cover the Claude providers exactly.
    pub fn reorder_claude_providers(
        &self,
        ordered_ids: &[String],
        expected_file_hashes: &BTreeMap<String, String>,
    ) -> Result<(), StoreOperationError> {
        let (_, claude) = load_all(self)?;
        let loaded = claude;
        check_expected_files(&loaded, expected_file_hashes)?;
        let unique: HashSet<&str> = ordered_ids.iter().map(String::as_str).collect();
        if unique.len() != ordered_ids.len() || ordered_ids.len() != loaded.len() {
            return Err("排序清单必须覆盖该客户端的全部供应商且不得重复".into());
        }
        // Re-read the complete snapshot and the raw file revisions before
        // constructing the replacement. This rejects a concurrent/manual
        // provider edit (including a formatting-only edit) instead of
        // replacing it blindly.
        let mut snapshot = crate::config_store::snapshot::read_configuration_snapshot(self)?;
        let (_, claude_after) = load_all(self)?;
        let loaded_after = claude_after;
        check_expected_files(&loaded_after, expected_file_hashes)?;
        let current_files: Vec<ProviderFile> =
            loaded_after.into_iter().map(|loaded| loaded.file).collect();
        if snapshot.claude_providers != current_files {
            return Err("供应商文件已被外部修改，请重新读取后再排序".into());
        }
        for id in ordered_ids {
            if !loaded.iter().any(|loaded| &loaded.file.id == id) {
                return Err(format!("排序清单包含不属于该客户端的供应商：{id}").into());
            }
        }
        // A reorder is one logical mutation. Rebuild it through the same
        // verified directory replacement used by restore, so a later file
        // failure cannot leave a subset of positions persisted.
        let files = &mut snapshot.claude_providers;
        for (order, id) in ordered_ids.iter().enumerate() {
            let file = files
                .iter_mut()
                .find(|file| &file.id == id)
                .expect("validated ordering ids exist in the snapshot");
            file.position = (order as u64 + 1) * POSITION_STEP;
        }
        files.sort_by_key(|file| file.position);
        crate::config_store::snapshot::enable_snapshot(self, &snapshot)?;
        Ok(())
    }

    /// Imports one draft as a provider file: an exactly equal provider is a
    /// no-op, a routing-identical one gains the missing usage query, and
    /// anything else creates a new file. Codex accepts its official-login
    /// record only; third-party Codex stays in the dedicated store.
    pub fn import_provider(
        &self,
        draft: ProviderDraft,
    ) -> Result<ProviderRecord, StoreOperationError> {
        if draft.app == AppKind::Codex && draft.route_mode != RouteMode::Official {
            return Err("Codex 第三方供应商必须使用专用档案格式".into());
        }
        draft.validate().map_err(|error| error.to_string())?;
        let (codex_official, claude) = load_all(self)?;
        let existing = match draft.app {
            AppKind::Codex => codex_official,
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
                let position = file.position;
                return Ok(ProviderRecord {
                    profile: file.into_profile(draft.app),
                    position,
                    file_hash: hash,
                });
            }
        }
        self.create_provider(Uuid::new_v4().to_string(), draft)
    }

    /// Whether an exactly equal provider already exists (scan-side view of
    /// the import dedup rule).
    pub fn provider_exists(&self, draft: &ProviderDraft) -> bool {
        if draft.app == AppKind::Codex && draft.route_mode != RouteMode::Official {
            return false;
        }
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

    /// The stored provider with the same routing identity as an import draft.
    /// The failover import uses this to map source queue rows onto local ids;
    /// the identity rule stays owned by this store, never reimplemented.
    #[expect(dead_code)] // reserved: duplicate-route audit
    pub fn find_routing_match(&self, draft: &ProviderDraft) -> Option<ProviderRecord> {
        match load_all(self) {
            Ok((codex, claude)) => {
                let existing = match draft.app {
                    AppKind::Codex => codex,
                    AppKind::Claude => claude,
                };
                existing
                    .into_iter()
                    .find(|loaded| {
                        same_provider_routing(&loaded.file.clone().into_profile(draft.app), draft)
                    })
                    .map(|loaded| record_of(draft.app, &loaded))
            }
            Err(_) => None,
        }
    }

    /// Whether a selected source import will enrich its otherwise matching
    /// local provider with a currently absent usage query.
    pub fn provider_will_receive_usage_query(&self, draft: &ProviderDraft) -> bool {
        if draft.app == AppKind::Codex && draft.route_mode != RouteMode::Official {
            return false;
        }
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
