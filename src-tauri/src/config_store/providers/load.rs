use crate::config_store::{
    content_revision, parse_file, read_optional, write_json_atomic, ConfigStore,
    ProfileStoreError, StoreOperationError, PROVIDER_POSITION_STEP,
};
use asb_core::contracts::{
    AppKind, ProviderDraft, ProviderFile, ProviderProfile, ProviderRecord, RouteMode,
};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

pub(super) const POSITION_STEP: u64 = PROVIDER_POSITION_STEP;
/// One loaded provider file with its storage revision.
pub(super) struct LoadedProvider {
    pub(super) file: ProviderFile,
    pub(super) hash: String,
}

/// The directory one client's generic provider files live in. Claude keeps
/// official-login records with its other providers; Codex third-party records
/// own the strict `providers/codex` store, so Codex official-login records
/// live in the `providers/codex/official` subdirectory (the strict reader
/// skips directory entries because they carry no `.json` extension).
fn provider_dir(store: &ConfigStore, app: AppKind) -> PathBuf {
    match app {
        AppKind::Claude => store.providers_dir(app),
        AppKind::Codex => store.providers_dir(app).join("official"),
    }
}

fn load_client(
    store: &ConfigStore,
    app: AppKind,
) -> Result<Vec<LoadedProvider>, ProfileStoreError> {
    store.ensure_layout()?;
    let dir = provider_dir(store, app);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(ProfileStoreError::unreadable(&dir, &error)),
    };
    let mut loaded = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| ProfileStoreError::unreadable(&dir, &error))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| ProfileStoreError::invalid(&path, "供应商文件名或文件内容不存在"))?
            .to_string();
        let text = read_optional(&path)?.ok_or_else(|| ProfileStoreError::invalid(&path, "供应商文件名或文件内容不存在"))?;
        let file = parse_file::<ProviderFile>(&text, &path)?;
        if Uuid::parse_str(&file.id).is_err() || file.id != stem {
            return Err(ProfileStoreError::invalid(&path, "供应商标识不是 UUID 或与文件名不一致"));
        }
        let profile = file.clone().into_profile(app);
        profile
            .validate()
            .map_err(|error| ProfileStoreError::invalid(&path, error.to_string()))?;
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
            return Err(ProfileStoreError::invalid(
                &dir.join(format!("{}.json", provider.file.id)),
                "供应商标识重复或排序位置无效",
            ));
        }
        previous_position = provider.file.position;
    }
    Ok(loaded)
}

/// Loads the generic provider boundary: every Claude provider plus the Codex
/// official-login records. Codex third-party providers stay in their own
/// typed store and are never projected through `ProviderFile` here.
pub(super) fn load_all(
    store: &ConfigStore,
) -> Result<(Vec<LoadedProvider>, Vec<LoadedProvider>), ProfileStoreError> {
    Ok((
        load_client(store, AppKind::Codex)?,
        load_client(store, AppKind::Claude)?,
    ))
}

pub(super) fn check_expected_files(
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

pub(super) fn record_of(app: AppKind, loaded: &LoadedProvider) -> ProviderRecord {
    ProviderRecord {
        profile: loaded.file.clone().into_profile(app),
        position: loaded.file.position,
        file_hash: loaded.hash.clone(),
    }
}

/// Every validated provider file of one client, in stored order. Snapshot
/// consumers need the files, not the per-file revisions.
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
/// returns the new storage revision. Codex accepts official-login records
/// only; its third-party providers must use the dedicated strict store.
pub(super) fn write_provider_file(
    store: &ConfigStore,
    app: AppKind,
    file: &ProviderFile,
) -> Result<String, StoreOperationError> {
    let profile = file.clone().into_profile(app);
    if app == AppKind::Codex && profile.route_mode != RouteMode::Official {
        return Err("Codex 第三方供应商必须使用专用档案格式".into());
    }
    profile.validate().map_err(|error| error.to_string())?;
    let json =
        serde_json::to_string_pretty(file).map_err(|_| "供应商文件序列化失败".to_string())?;
    let path = provider_path(store, app, &file.id);
    store.initialize_current_layout()?;
    write_json_atomic(&path, &json)?;
    Ok(content_revision(json.as_bytes()))
}

pub(super) fn provider_path(store: &ConfigStore, app: AppKind, id: &str) -> PathBuf {
    provider_dir(store, app).join(format!("{id}.json"))
}

pub(super) fn next_position(loaded: &[LoadedProvider]) -> u64 {
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
pub(super) fn same_provider_routing(profile: &ProviderProfile, draft: &ProviderDraft) -> bool {
    if profile.parameters != draft.parameters {
        return false;
    }
    if draft.route_mode == RouteMode::Official {
        return profile.app == draft.app && profile.route_mode == RouteMode::Official;
    }
    profile.app == draft.app
        && profile.route_mode == draft.route_mode
        && profile.name == draft.name
        && profile.model == draft.model
        && profile.base_url == draft.base_url
        && profile.connection == draft.connection
        && profile.api_key == draft.api_key
        && profile.authentication == draft.authentication
        && profile.upstream_protocol == draft.upstream_protocol
        && profile.max_output_tokens == draft.max_output_tokens
        && profile.model_options == draft.model_options
}

pub(super) fn same_provider(profile: &ProviderProfile, draft: &ProviderDraft) -> bool {
    same_provider_routing(profile, draft)
        && profile.notes == draft.notes
        && profile.website_url == draft.website_url
        && profile.usage_query == draft.usage_query
}
