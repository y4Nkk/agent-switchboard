//! The one-time migration from the legacy `profiles.json` store to the
//! per-file `configuration/` layout.
//!
//! This is a migration, not a compatibility path: only the final legacy
//! contract (`profiles` + `commonBases` + `writeHistory`) is accepted, every
//! conversion is validated before anything is enabled, and a failure leaves
//! the legacy file untouched. After a successful run the runtime never reads
//! `profiles.json` again.

use super::{history, snapshot::ConfigurationSnapshot, ConfigStore};
use asb_core::contracts::{
    AppKind, CommonSettingValue, CommonSettings, ConfigValue, ConfigWriteRecord, ModelOptions,
    ProviderFile, ProviderProfile, RouteMode, UsageQuery, WriteOperation,
};
use asb_core::ownership::default_common_settings;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const POSITION_STEP: u64 = 100;

/// Exactly the final legacy persisted shape. Older shapes fail loudly and
/// keep the legacy file in place. These types exist only at the migration
/// boundary and are never returned to runtime callers.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyStore {
    profiles: Vec<LegacyProviderProfile>,
    common_bases: LegacyCommonBases,
    write_history: Vec<LegacyConfigWriteRecord>,
}

/// The previous application-owned profile file had no explicit route mode.
/// It exists only to upgrade every valid historic profile to `Custom`; the
/// runtime accepts only the current route-mode contract.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyProviderProfile {
    id: String,
    app: AppKind,
    name: String,
    model: Option<String>,
    base_url: Option<String>,
    api_key: String,
    model_options: Option<ModelOptions>,
    notes: Option<String>,
    website_url: Option<String>,
    usage_query: Option<UsageQuery>,
}

/// Provider-file shape from immediately before `routeMode` became explicit.
/// This boundary type is never accepted by normal storage reads.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyProviderFile {
    id: String,
    name: String,
    position: u64,
    api_key: String,
    base_url: Option<String>,
    model: Option<String>,
    model_options: Option<ModelOptions>,
    notes: Option<String>,
    website_url: Option<String>,
    usage_query: Option<UsageQuery>,
}

impl LegacyProviderFile {
    fn into_current(self) -> ProviderFile {
        ProviderFile {
            id: self.id,
            name: self.name,
            position: self.position,
            route_mode: RouteMode::Custom,
            api_key: self.api_key,
            base_url: self.base_url,
            model: self.model,
            model_options: self.model_options,
            notes: self.notes,
            website_url: self.website_url,
            usage_query: self.usage_query,
        }
    }
}

impl LegacyProviderProfile {
    fn into_current(self) -> ProviderProfile {
        ProviderProfile {
            id: self.id,
            app: self.app,
            route_mode: RouteMode::Custom,
            name: self.name,
            model: self.model,
            base_url: self.base_url,
            api_key: self.api_key,
            model_options: self.model_options,
            notes: self.notes,
            website_url: self.website_url,
            usage_query: self.usage_query,
        }
    }
}

/// The retired history operation is accepted only while reading the one legacy
/// file, then normalized into the sole current projection operation.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyConfigWriteRecord {
    app: AppKind,
    profile_id: Option<String>,
    profile_name: Option<String>,
    content_hash: String,
    backup_id: String,
    at: String,
    operation: LegacyWriteOperation,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum LegacyWriteOperation {
    ProviderProjection,
    Restore,
    HistoricalBaseProjection,
}

impl LegacyConfigWriteRecord {
    fn into_current(self) -> Result<ConfigWriteRecord, String> {
        let (operation, profile_id, profile_name) = match self.operation {
            LegacyWriteOperation::ProviderProjection => (
                WriteOperation::Projection,
                self.profile_id,
                self.profile_name,
            ),
            LegacyWriteOperation::Restore => {
                if self.profile_id.is_some() || self.profile_name.is_some() {
                    return Err("恢复历史记录不能包含供应商信息".to_string());
                }
                (WriteOperation::Restore, None, None)
            }
            LegacyWriteOperation::HistoricalBaseProjection => {
                if self.profile_id.is_some() || self.profile_name.is_some() {
                    return Err("历史通用投影记录不能包含供应商信息".to_string());
                }
                (WriteOperation::Projection, None, None)
            }
        };
        Ok(ConfigWriteRecord {
            app: self.app,
            profile_id,
            profile_name,
            content_hash: self.content_hash,
            backup_id: self.backup_id,
            at: self.at,
            operation,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyCommonBases {
    codex: LegacyCommonBase,
    claude: LegacyCommonBase,
}

impl LegacyCommonBases {
    fn for_app(&self, app: AppKind) -> &LegacyCommonBase {
        match app {
            AppKind::Codex => &self.codex,
            AppKind::Claude => &self.claude,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyCommonBase {
    entries: Vec<LegacyBaseEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyBaseEntry {
    key: String,
    value: Option<ConfigValue>,
}

/// The immediately previous common-settings file stored one plain value for
/// every catalog key. This exact key set is accepted only at the one-time
/// migration boundary; normal readers never deserialize it.
const LEGACY_CODEX_COMMON_KEYS: &[&str] = &[
    "hide_agent_reasoning",
    "show_raw_agent_reasoning",
    "disable_response_storage",
    "tui.animations",
    "tui.show_tooltips",
    "tui.notifications",
    "tui.raw_output_mode",
    "tui.vim_mode_default",
    "disable_paste_burst",
    "tools.view_image",
    "features.memories",
    "features.prevent_idle_sleep",
    "check_for_update_on_startup",
    "model_reasoning_effort",
    "model_reasoning_summary",
    "model_verbosity",
    "personality",
    "web_search",
    "sandbox_mode",
    "approval_policy",
    "history.persistence",
];

const LEGACY_CLAUDE_COMMON_KEYS: &[&str] = &[
    "alwaysThinkingEnabled",
    "autoCompactEnabled",
    "showThinkingSummaries",
    "spinnerTipsEnabled",
    "autoScrollEnabled",
    "emojiCompletionEnabled",
    "promptSuggestionEnabled",
    "showTurnDuration",
    "syntaxHighlightingDisabled",
    "terminalProgressBarEnabled",
    "fileCheckpointingEnabled",
    "respectGitignore",
    "includeGitInstructions",
    "attribution.coAuthoredBy",
    "autoMemoryEnabled",
    "outputStyle",
    "preferredNotifChannel",
];

/// The first tagged common-settings contract shipped before this official
/// directory expansion. It already used automatic/explicit values, but did
/// not yet contain the newly catalogued Codex preferences. This exact shape
/// is accepted only by the one-time layout migration below.
const PREVIOUS_TAGGED_CODEX_COMMON_KEYS: &[&str] = &[
    "hide_agent_reasoning",
    "show_raw_agent_reasoning",
    "tui.animations",
    "tui.show_tooltips",
    "tui.notifications",
    "tui.raw_output_mode",
    "tui.vim_mode_default",
    "disable_paste_burst",
    "tools.view_image",
    "features.memories",
    "features.prevent_idle_sleep",
    "check_for_update_on_startup",
    "model_reasoning_effort",
    "plan_mode_reasoning_effort",
    "model_reasoning_summary",
    "model_verbosity",
    "personality",
    "web_search",
    "sandbox_mode",
    "approval_policy",
    "history.persistence",
];

const PREVIOUS_TAGGED_CLAUDE_COMMON_KEYS: &[&str] = &[
    "alwaysThinkingEnabled",
    "autoCompactEnabled",
    "showThinkingSummaries",
    "spinnerTipsEnabled",
    "autoScrollEnabled",
    "emojiCompletionEnabled",
    "promptSuggestionEnabled",
    "showTurnDuration",
    "syntaxHighlightingDisabled",
    "terminalProgressBarEnabled",
    "fileCheckpointingEnabled",
    "respectGitignore",
    "includeGitInstructions",
    "autoMemoryEnabled",
    "outputStyle",
    "preferredNotifChannel",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyPlainCommonSettings {
    settings: BTreeMap<String, ConfigValue>,
}

fn legacy_common_keys(app: AppKind) -> &'static [&'static str] {
    match app {
        AppKind::Codex => LEGACY_CODEX_COMMON_KEYS,
        AppKind::Claude => LEGACY_CLAUDE_COMMON_KEYS,
    }
}

fn is_removed_legacy_common_key(app: AppKind, key: &str) -> bool {
    (app == AppKind::Codex && key == "disable_response_storage")
        || (app == AppKind::Claude && key == "attribution.coAuthoredBy")
}

fn has_exact_key_set(settings: &BTreeMap<String, CommonSettingValue>, expected: &[&str]) -> bool {
    let actual: BTreeSet<&str> = settings.keys().map(String::as_str).collect();
    actual == expected.iter().copied().collect()
}

fn has_current_key_set(app: AppKind, settings: &BTreeMap<String, CommonSettingValue>) -> bool {
    settings
        .keys()
        .eq(default_common_settings(app).settings.keys())
}

fn is_legacy_claude_output_style(value: &CommonSettingValue) -> bool {
    matches!(
        value,
        CommonSettingValue::Explicit {
            value: ConfigValue::Str(value),
        } if matches!(value.as_str(), "default" | "explanatory" | "learning")
    )
}

fn migrate_tagged_claude_output_style(value: CommonSettingValue) -> CommonSettingValue {
    match value {
        CommonSettingValue::Explicit {
            value: ConfigValue::Str(value),
        } => match value.as_str() {
            "default" => CommonSettingValue::Automatic,
            "explanatory" => CommonSettingValue::Explicit {
                value: ConfigValue::Str("Explanatory".to_string()),
            },
            "learning" => CommonSettingValue::Explicit {
                value: ConfigValue::Str("Learning".to_string()),
            },
            _ => CommonSettingValue::Explicit {
                value: ConfigValue::Str(value),
            },
        },
        value => value,
    }
}

fn migrate_previous_tagged_common(
    app: AppKind,
    previous: CommonSettings,
) -> Result<Option<CommonSettings>, String> {
    let needs_migration = match app {
        AppKind::Codex => has_exact_key_set(&previous.settings, PREVIOUS_TAGGED_CODEX_COMMON_KEYS),
        AppKind::Claude => {
            has_exact_key_set(&previous.settings, PREVIOUS_TAGGED_CLAUDE_COMMON_KEYS)
                || (has_current_key_set(app, &previous.settings)
                    && previous
                        .settings
                        .get("outputStyle")
                        .is_some_and(is_legacy_claude_output_style))
        }
    };
    if !needs_migration {
        return Ok(None);
    }

    let mut settings = default_common_settings(app).settings;
    for (key, value) in previous.settings {
        let value = match (app, key.as_str()) {
            (AppKind::Claude, "outputStyle") => migrate_tagged_claude_output_style(value),
            _ => value,
        };
        settings.insert(key, value);
    }
    let settings = CommonSettings { settings };
    settings
        .validate_for(app)
        .map_err(|error| error.to_string())?;
    Ok(Some(settings))
}

fn migrate_legacy_common_value(
    app: AppKind,
    key: &str,
    value: ConfigValue,
) -> Result<CommonSettingValue, String> {
    if is_removed_legacy_common_key(app, key) {
        return Ok(CommonSettingValue::Automatic);
    }
    let value = match (app, key, value) {
        (AppKind::Claude, "outputStyle", ConfigValue::Str(value)) => match value.as_str() {
            "default" => return Ok(CommonSettingValue::Automatic),
            "explanatory" => ConfigValue::Str("Explanatory".to_string()),
            "learning" => ConfigValue::Str("Learning".to_string()),
            _ => ConfigValue::Str(value),
        },
        (_, _, value) => value,
    };
    let spec = asb_core::ownership::setting_spec(app, key)
        .filter(|spec| spec.owner == asb_core::ownership::SettingOwner::Common)
        .ok_or_else(|| format!("旧版通用参数不受当前目录支持：{key}"))?;
    if spec.legacy_default.as_ref() == Some(&value) {
        Ok(CommonSettingValue::Automatic)
    } else {
        Ok(CommonSettingValue::Explicit { value })
    }
}

fn migrate_legacy_plain_common(app: AppKind, text: &str) -> Result<CommonSettings, String> {
    let legacy: LegacyPlainCommonSettings =
        serde_json::from_str(text).map_err(|_| "通用设置不是受支持的上一版契约".to_string())?;
    let expected: BTreeSet<&str> = legacy_common_keys(app).iter().copied().collect();
    let actual: BTreeSet<&str> = legacy.settings.keys().map(String::as_str).collect();
    if actual != expected {
        return Err("旧版通用设置键集不完整或包含未知项".to_string());
    }
    let mut settings = default_common_settings(app).settings;
    for (key, value) in legacy.settings {
        if is_removed_legacy_common_key(app, &key) {
            continue;
        }
        settings.insert(key.clone(), migrate_legacy_common_value(app, &key, value)?);
    }
    let settings = CommonSettings { settings };
    settings
        .validate_for(app)
        .map_err(|error| error.to_string())?;
    Ok(settings)
}

fn read_current_or_legacy_common(
    app: AppKind,
    text: &str,
) -> Result<(CommonSettings, bool), String> {
    if let Ok(settings) = serde_json::from_str::<CommonSettings>(text) {
        if let Some(migrated) = migrate_previous_tagged_common(app, settings.clone())? {
            return Ok((migrated, true));
        }
        settings
            .validate_for(app)
            .map_err(|error| error.to_string())?;
        return Ok((settings, false));
    }
    migrate_legacy_plain_common(app, text).map(|settings| (settings, true))
}

/// Converts one legacy common base into complete plain-value settings.
/// A kept value must be a catalog-legal value; a `null` (the legacy
/// "remove this line" state) and every unlisted parameter fall back to the
/// directory default.
fn convert_common(app: AppKind, base: &LegacyCommonBase) -> Result<CommonSettings, String> {
    let mut settings = default_common_settings(app).settings;
    let mut seen = BTreeSet::new();
    for entry in &base.entries {
        let LegacyBaseEntry { key, value } = entry;
        if !seen.insert(key) {
            return Err(format!("通用参数重复：{key}"));
        }
        match value {
            Some(converted) => {
                if is_removed_legacy_common_key(app, key) {
                    continue;
                }
                settings.insert(
                    key.clone(),
                    migrate_legacy_common_value(app, key, converted.clone())?,
                );
            }
            None => {
                // The legacy explicit-remove state becomes the client default.
                let _ = key;
            }
        }
    }
    let settings = CommonSettings { settings };
    settings
        .validate_for(app)
        .map_err(|error| format!("通用参数 {error}"))?;
    Ok(settings)
}

fn convert_providers(
    app: AppKind,
    legacy: &[LegacyProviderProfile],
) -> Result<Vec<ProviderFile>, String> {
    let mut files = Vec::new();
    for (index, profile) in legacy
        .iter()
        .cloned()
        .map(LegacyProviderProfile::into_current)
        .enumerate()
        .filter(|(_, p)| p.app == app)
    {
        profile
            .validate()
            .map_err(|error| format!("供应商 {}：{error}", profile.name))?;
        if uuid::Uuid::parse_str(&profile.id).is_err() {
            return Err(format!("供应商标识不是稳定 UUID：{}", profile.id));
        }
        if let Some(query) = &profile.usage_query {
            crate::usage_query::validate_persisted(query)
                .map_err(|error| format!("供应商 {}：{error}", profile.name))?;
        }
        files.push(ProviderFile::from_profile(
            &profile,
            (index as u64 + 1) * POSITION_STEP,
        ));
    }
    Ok(files)
}

fn split_history(
    app: AppKind,
    legacy: &[LegacyConfigWriteRecord],
) -> Result<Vec<ConfigWriteRecord>, String> {
    let records: Vec<ConfigWriteRecord> = legacy
        .iter()
        .filter(|record| record.app == app)
        .cloned()
        .map(LegacyConfigWriteRecord::into_current)
        .collect::<Result<_, _>>()?;
    for record in &records {
        history::validate_write_record(app, record)?;
    }
    Ok(records)
}

fn read_route_migration_provider_files(
    store: &ConfigStore,
    app: AppKind,
) -> Result<(Vec<ProviderFile>, bool, bool), String> {
    let dir = store.providers_dir(app);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((vec![], false, false))
        }
        Err(_) => return Err("无法读取供应商目录".to_string()),
    };
    let mut files = Vec::new();
    let mut current = false;
    let mut legacy = false;
    for entry in entries {
        let path = entry.map_err(|_| "无法读取供应商文件".to_string())?.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|_| "无法读取供应商文件".to_string())?;
        match serde_json::from_str::<ProviderFile>(&text) {
            Ok(file) => {
                current = true;
                files.push(file);
            }
            Err(_) => {
                let legacy_file: LegacyProviderFile = serde_json::from_str(&text)
                    .map_err(|_| "供应商文件不是受支持的上一版契约".to_string())?;
                legacy = true;
                files.push(legacy_file.into_current());
            }
        }
    }
    files.sort_by_key(|file| file.position);
    Ok((files, current, legacy))
}

fn read_route_migration_common(
    store: &ConfigStore,
    app: AppKind,
) -> Result<CommonSettings, String> {
    match super::read_optional(&store.common_path(app)).map_err(|_| "通用设置不可读".to_string())?
    {
        None => Ok(default_common_settings(app)),
        Some(text) => read_current_or_legacy_common(app, &text).map(|(settings, _)| settings),
    }
}

fn read_route_migration_history(
    store: &ConfigStore,
    app: AppKind,
) -> Result<Vec<ConfigWriteRecord>, String> {
    match super::read_optional(&store.history_path(app))
        .map_err(|_| "写入历史不可读".to_string())?
    {
        None => Ok(vec![]),
        Some(text) => {
            let file: history::HistoryFile =
                serde_json::from_str(&text).map_err(|_| "写入历史不是当前契约".to_string())?;
            for record in &file.records {
                history::validate_write_record(app, record)?;
            }
            Ok(file.records)
        }
    }
}

/// Produces the current-contract text for the one previous usage-query
/// shape: a query saved before `refreshIntervalMinutes` existed gains the
/// explicit manual-only value. Any other shape returns `None` so it stays
/// with the migration or strict reader that owns its fate.
fn insert_missing_refresh_interval(text: &str) -> Option<String> {
    let mut value: serde_json::Value = serde_json::from_str(text).ok()?;
    let query = value.get_mut("usageQuery")?.as_object_mut()?;
    if query.contains_key("refreshIntervalMinutes") {
        return None;
    }
    let previous_keys = match query.get("kind").and_then(|kind| kind.as_str()) {
        Some("declarative") => &[
            "kind",
            "url",
            "remainingPath",
            "usedPath",
            "totalPath",
            "unit",
        ][..],
        Some("script") => &["kind", "source"][..],
        _ => return None,
    };
    if !query
        .keys()
        .all(|key| previous_keys.contains(&key.as_str()))
    {
        return None;
    }
    query.insert("refreshIntervalMinutes".to_string(), serde_json::json!(0));
    serde_json::to_string_pretty(&value).ok()
}

/// Atomically upgrades provider files saved before the usage query carried
/// an auto-refresh interval. Files already on the current contract stay
/// untouched; files of any other shape stay exactly as read so the route
/// migration or the strict reader rejects them loudly. This is a one-time
/// storage migration, not a runtime fallback: once rewritten, strict
/// readers reject the old shape.
pub fn migrate_usage_query_interval(store: &ConfigStore) -> Result<(), String> {
    for app in [AppKind::Codex, AppKind::Claude] {
        let entries = match std::fs::read_dir(store.providers_dir(app)) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err("无法读取供应商目录".to_string()),
        };
        for entry in entries {
            let path = entry.map_err(|_| "无法读取供应商文件".to_string())?.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let text =
                std::fs::read_to_string(&path).map_err(|_| "无法读取供应商文件".to_string())?;
            if serde_json::from_str::<ProviderFile>(&text).is_ok() {
                continue;
            }
            let Some(upgraded) = insert_missing_refresh_interval(&text) else {
                continue;
            };
            if serde_json::from_str::<ProviderFile>(&upgraded).is_err() {
                continue;
            }
            super::write_json_atomic(&path, &upgraded)
                .map_err(|_| "无法原子保存配置存储".to_string())?;
        }
    }
    Ok(())
}

/// Atomically upgrades the already-split provider files that predate the
/// explicit `routeMode` contract. This is a one-time storage migration, not
/// a runtime fallback: once enabled, strict readers reject the old shape.
pub fn migrate_provider_route_mode(store: &ConfigStore) -> Result<(), String> {
    if !store.configuration_dir().exists() {
        return Ok(());
    }
    let mut providers = BTreeMap::new();
    let mut current_count = 0;
    let mut legacy_count = 0;
    for app in [AppKind::Codex, AppKind::Claude] {
        let (files, current, legacy) = read_route_migration_provider_files(store, app)?;
        current_count += usize::from(current);
        legacy_count += usize::from(legacy);
        providers.insert(app, files);
    }
    if legacy_count == 0 {
        return Ok(());
    }
    if current_count != 0 {
        return Err("供应商文件混用了新旧路由契约，无法安全迁移".to_string());
    }

    let mut common = BTreeMap::new();
    let mut history_map = BTreeMap::new();
    for app in [AppKind::Codex, AppKind::Claude] {
        common.insert(app, read_route_migration_common(store, app)?);
        history_map.insert(app, read_route_migration_history(store, app)?);
    }
    super::snapshot::enable_snapshot(
        store,
        &ConfigurationSnapshot {
            providers,
            common,
            history: history_map,
        },
    )
}

/// Upgrades supported previous complete common-settings contracts into the
/// current tagged automatic/explicit contract. It stages the entire
/// configuration directory and swaps it only after strict readback, so there
/// is no runtime dual-read path and no partial client upgrade.
pub fn migrate_common_settings_semantics(store: &ConfigStore) -> Result<(), String> {
    if !store.configuration_dir().exists() {
        return Ok(());
    }

    let mut common = BTreeMap::new();
    let mut needs_migration = false;
    for app in [AppKind::Codex, AppKind::Claude] {
        let settings = match super::read_optional(&store.common_path(app))
            .map_err(|_| "通用设置不可读".to_string())?
        {
            None => default_common_settings(app),
            Some(text) => {
                let (settings, migrated) = read_current_or_legacy_common(app, &text)?;
                needs_migration |= migrated;
                settings
            }
        };
        common.insert(app, settings);
    }
    if !needs_migration {
        return Ok(());
    }

    let mut providers_map = BTreeMap::new();
    let mut history_map = BTreeMap::new();
    for app in [AppKind::Codex, AppKind::Claude] {
        let (files, current, legacy) = read_route_migration_provider_files(store, app)?;
        if legacy || (!current && !files.is_empty()) {
            return Err("通用设置迁移前供应商路由契约未收敛".to_string());
        }
        providers_map.insert(app, files);
        history_map.insert(app, read_route_migration_history(store, app)?);
    }
    super::snapshot::enable_snapshot(
        store,
        &ConfigurationSnapshot {
            providers: providers_map,
            common,
            history: history_map,
        },
    )
}

/// Runs the migration. On any conflict the whole migration fails, the staged
/// directory is discarded, and the legacy file stays in place.
pub fn run(store: &ConfigStore) -> Result<(), String> {
    let legacy_path = store.legacy_store_path();
    let text =
        std::fs::read_to_string(&legacy_path).map_err(|_| "旧版配置数据不可读".to_string())?;
    let legacy: LegacyStore = serde_json::from_str(&text).map_err(|_| {
        "旧版配置数据不是受支持的最后一个完整契约；请重置或使用旧版本应用导出".to_string()
    })?;

    let mut providers = BTreeMap::new();
    let mut common = BTreeMap::new();
    let mut history = BTreeMap::new();
    for app in [AppKind::Codex, AppKind::Claude] {
        providers.insert(app, convert_providers(app, &legacy.profiles)?);
        common.insert(app, convert_common(app, &legacy.common_bases.for_app(app))?);
        history.insert(app, split_history(app, &legacy.write_history)?);
    }
    let snapshot = ConfigurationSnapshot {
        providers,
        common,
        history,
    };

    // Move the old source out of its runtime-recognized name before activating
    // the new directory. A failed activation moves it back, so we never leave
    // `profiles.json` and `configuration/` as competing live stores.
    let archived = store.migration_archive_dir().join(format!(
        "migrated-profiles-{}.json",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(store.migration_archive_dir())
        .map_err(|_| "无法创建旧版配置数据隔离目录；原文件未改动".to_string())?;
    std::fs::rename(&legacy_path, &archived)
        .map_err(|_| "无法隔离旧版配置数据；原文件未改动".to_string())?;
    if let Err(error) = super::snapshot::enable_snapshot(store, &snapshot) {
        return match std::fs::rename(&archived, &legacy_path) {
            Ok(()) => Err(error),
            Err(_) => Err(format!(
                "{error}；旧版配置数据已保留在 {}，需手动恢复为 profiles.json",
                archived.display()
            )),
        };
    }

    // The source name is already absent, so an antivirus/permission failure
    // here cannot create a dual runtime path or block subsequent reads. Reset
    // removes any recoverable archive left behind by an interrupted cleanup.
    let _ = std::fs::remove_file(archived);
    let _ = std::fs::remove_dir(store.migration_archive_dir());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::ProviderDraft;
    use std::fs;

    const LEGACY_COMPLETE: &str = r#"{
  "profiles": [
    {
      "id": "0b91a2f4-6c85-4a12-9f0d-2f4a1b3c5d6e",
      "app": "codex",
      "name": "旧网关",
      "model": "gpt-5.2",
      "baseUrl": "https://legacy.example/v1",
      "apiKey": "test-legacy-key",
      "modelOptions": null,
      "notes": null,
      "websiteUrl": null,
      "usageQuery": null
    }
  ],
  "commonBases": {
    "codex": { "entries": [
      { "key": "disable_response_storage", "value": true },
      { "key": "model_reasoning_effort", "value": null }
    ]},
    "claude": { "entries": [] }
  },
  "writeHistory": [
    {
      "app": "codex",
      "profileId": "0b91a2f4-6c85-4a12-9f0d-2f4a1b3c5d6e",
      "profileName": "旧网关",
      "contentHash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "backupId": "b1",
      "at": "2026-08-28T08:00:00Z",
      "operation": "providerProjection"
    }
  ]
}"#;

    fn store_with_legacy(text: &str) -> (tempfile::TempDir, ConfigStore) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));
        fs::create_dir_all(&store.state_root).unwrap();
        fs::write(store.legacy_store_path(), text).unwrap();
        (directory, store)
    }

    #[test]
    fn legacy_store_migrates_once_into_independent_files() {
        let (_dir, store) = store_with_legacy(LEGACY_COMPLETE);

        store.ensure_layout().expect("migration succeeds");

        // One provider file per provider, named by its UUID.
        let codex_dir = store.providers_dir(AppKind::Codex);
        let entries: Vec<_> = fs::read_dir(&codex_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_str().unwrap().to_string())
            .collect();
        assert_eq!(
            entries,
            vec!["0b91a2f4-6c85-4a12-9f0d-2f4a1b3c5d6e.json".to_string()]
        );

        // Removed legacy keys disappear. The surviving gap becomes automatic.
        let codex_common = store.get_common_settings(AppKind::Codex).unwrap();
        assert!(codex_common
            .settings
            .value("disable_response_storage")
            .is_none());
        assert_eq!(
            codex_common.settings.value("model_reasoning_effort"),
            default_common_settings(AppKind::Codex).value("model_reasoning_effort")
        );
        assert_eq!(
            store
                .get_common_settings(AppKind::Claude)
                .unwrap()
                .settings
                .settings,
            default_common_settings(AppKind::Claude).settings
        );

        // History split per client.
        let codex_history = store.latest_config_write(AppKind::Codex).unwrap().unwrap();
        assert_eq!(codex_history.profile_name.as_deref(), Some("旧网关"));
        assert!(store
            .latest_config_write(AppKind::Claude)
            .unwrap()
            .is_none());

        // The legacy file is gone and a second run is a no-op.
        assert!(!store.legacy_store_path().exists());
        assert!(!store.migration_archive_dir().exists());
        store.ensure_layout().expect("second run is fine");
    }

    #[test]
    fn split_provider_files_gain_explicit_custom_route_mode_atomically() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));
        let created = store
            .create_provider(ProviderDraft {
                app: AppKind::Codex,
                route_mode: RouteMode::Custom,
                name: "旧网关".to_string(),
                model: None,
                base_url: Some("https://legacy.example/v1".to_string()),
                api_key: "test-legacy-key".to_string(),
                model_options: None,
                notes: None,
                website_url: None,
                usage_query: None,
            })
            .expect("current provider");
        let path = store
            .providers_dir(AppKind::Codex)
            .join(format!("{}.json", created.profile.id));
        let old = fs::read_to_string(&path)
            .unwrap()
            .replace("  \"routeMode\": \"custom\",\n", "");
        fs::write(&path, old).unwrap();

        store.ensure_layout().expect("route migration");

        let migrated = fs::read_to_string(path).unwrap();
        assert!(migrated.contains("\"routeMode\": \"custom\""));
        assert_eq!(
            store.list_providers().unwrap()[0].profile.route_mode,
            RouteMode::Custom
        );
    }

    #[test]
    fn plain_common_file_is_migrated_once_to_automatic_or_explicit_intent() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));
        fs::create_dir_all(store.common_path(AppKind::Claude).parent().unwrap()).unwrap();
        let legacy = serde_json::json!({
            "settings": LEGACY_CLAUDE_COMMON_KEYS
                .iter()
                .map(|key| {
                    let value = match *key {
                        "outputStyle" => serde_json::Value::String("learning".to_string()),
                        "preferredNotifChannel" => serde_json::Value::String("auto".to_string()),
                        "showTurnDuration" => serde_json::Value::Bool(true),
                        "attribution.coAuthoredBy" => serde_json::Value::Bool(true),
                        _ => serde_json::Value::Bool(true),
                    };
                    ((*key).to_string(), value)
                })
                .collect::<serde_json::Map<_, _>>(),
        });
        fs::write(
            store.common_path(AppKind::Claude),
            serde_json::to_string(&legacy).unwrap(),
        )
        .unwrap();

        store.ensure_layout().expect("plain common migration");

        let migrated = store.get_common_settings(AppKind::Claude).unwrap().settings;
        assert_eq!(
            migrated.value("outputStyle"),
            Some(&CommonSettingValue::Explicit {
                value: ConfigValue::Str("Learning".to_string()),
            })
        );
        assert_eq!(
            migrated.value("showTurnDuration"),
            Some(&CommonSettingValue::Explicit {
                value: ConfigValue::Bool(true),
            })
        );
        assert!(migrated.value("attribution.coAuthoredBy").is_none());
        let persisted = fs::read_to_string(store.common_path(AppKind::Claude)).unwrap();
        assert!(persisted.contains("\"mode\": \"explicit\""));
        assert!(!persisted.contains("attribution.coAuthoredBy"));
    }

    #[test]
    fn previous_tagged_codex_settings_gain_new_catalog_keys_automatically() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));
        fs::create_dir_all(store.common_path(AppKind::Codex).parent().unwrap()).unwrap();
        let mut settings = BTreeMap::new();
        for key in PREVIOUS_TAGGED_CODEX_COMMON_KEYS {
            settings.insert((*key).to_string(), CommonSettingValue::Automatic);
        }
        settings.insert(
            "model_reasoning_effort".to_string(),
            CommonSettingValue::Explicit {
                value: ConfigValue::Str("high".to_string()),
            },
        );
        fs::write(
            store.common_path(AppKind::Codex),
            serde_json::to_string(&CommonSettings { settings }).unwrap(),
        )
        .unwrap();

        store.ensure_layout().expect("tagged common migration");

        let migrated = store.get_common_settings(AppKind::Codex).unwrap().settings;
        assert_eq!(
            migrated.value("model_reasoning_effort"),
            Some(&CommonSettingValue::Explicit {
                value: ConfigValue::Str("high".to_string()),
            })
        );
        assert_eq!(
            migrated.value("allow_login_shell"),
            Some(&CommonSettingValue::Automatic)
        );
        assert_eq!(
            migrated.value("features.multi_agent"),
            Some(&CommonSettingValue::Automatic)
        );
        assert_eq!(
            migrated.settings.len(),
            default_common_settings(AppKind::Codex).settings.len()
        );
    }

    #[test]
    fn current_tagged_claude_settings_do_not_trigger_another_migration() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path().join("state"));
        fs::create_dir_all(store.common_path(AppKind::Claude).parent().unwrap()).unwrap();
        let current = default_common_settings(AppKind::Claude);
        let source = serde_json::to_string(&current).unwrap();
        fs::write(store.common_path(AppKind::Claude), &source).unwrap();

        store
            .ensure_layout()
            .expect("current tagged settings are stable");

        assert_eq!(
            fs::read_to_string(store.common_path(AppKind::Claude)).unwrap(),
            source
        );
    }

    #[test]
    fn an_older_or_foreign_shape_fails_without_touching_the_source() {
        for legacy in [
            r#"{"profiles":[],"common":[],"switchLog":[]}"#,
            r#"{"profiles":[]}"#,
            "not json at all",
        ] {
            let (_dir, store) = store_with_legacy(legacy);
            let error = store.ensure_layout().expect_err("must fail");
            assert!(matches!(
                error,
                super::super::ProfileStoreError::Migration(_)
            ));
            assert_eq!(
                fs::read_to_string(store.legacy_store_path()).unwrap(),
                legacy
            );
            assert!(!store.configuration_dir().exists());
        }
    }

    #[test]
    fn an_invalid_legacy_provider_fails_the_whole_migration() {
        let broken = LEGACY_COMPLETE.replace("https://legacy.example/v1", "legacy.example");
        let (_dir, store) = store_with_legacy(&broken);

        let error = store.ensure_layout().expect_err("must fail");
        assert!(error.to_string().contains("旧版配置数据迁移失败"));
        assert!(store.legacy_store_path().exists());
        assert!(!store.configuration_dir().exists());
    }

    #[test]
    fn a_conflicting_history_hash_fails_the_whole_migration() {
        let broken = LEGACY_COMPLETE.replace(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "short-hash",
        );
        let (_dir, store) = store_with_legacy(&broken);
        assert!(store.ensure_layout().is_err());
        assert!(store.legacy_store_path().exists());
    }

    #[test]
    fn duplicate_legacy_common_key_fails_without_replacing_the_source() {
        let broken = LEGACY_COMPLETE.replace(
            "{ \"key\": \"disable_response_storage\", \"value\": true },",
            "{ \"key\": \"disable_response_storage\", \"value\": true },\n      { \"key\": \"disable_response_storage\", \"value\": false },",
        );
        let (_dir, store) = store_with_legacy(&broken);

        assert!(store.ensure_layout().is_err());
        assert_eq!(
            fs::read_to_string(store.legacy_store_path()).unwrap(),
            broken
        );
        assert!(!store.configuration_dir().exists());
    }

    #[test]
    fn retired_history_operation_becomes_a_current_generic_projection() {
        let legacy = LEGACY_COMPLETE.replace("providerProjection", "historicalBaseProjection").replace(
            "\"profileId\": \"0b91a2f4-6c85-4a12-9f0d-2f4a1b3c5d6e\",\n      \"profileName\": \"旧网关\",\n      ",
            "\"profileId\": null,\n      \"profileName\": null,\n      ",
        );
        let (_dir, store) = store_with_legacy(&legacy);

        store
            .ensure_layout()
            .expect("legacy historical record migrates");
        let record = store.latest_config_write(AppKind::Codex).unwrap().unwrap();
        assert_eq!(record.operation, WriteOperation::Projection);
        assert!(record.profile_id.is_none());
        assert!(record.profile_name.is_none());
    }
}
