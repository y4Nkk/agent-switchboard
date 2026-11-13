//! Explicit, staged repair of the current configuration layout.

#[path = "repair/settings.rs"]
mod settings;
use settings::{fill_automatic, repair_claude_provider};

mod files;
mod transaction;
use files::{copy_tree, json_files, relative};
#[cfg(test)]
use files::file_tree;
use super::{history, write_json_atomic, ConfigStore, PROFILE_PREIMAGE_FILE, SWITCH_INTENT_FILE};
use super::error::json_diagnostic;
use asb_core::contracts::{
    AppKind, CodexProviderFile, ProviderConnectionOptions, ProviderFile, RouteMode,
    SettingsValues, UpstreamProtocol,
};
use asb_core::endpoint::strip_anthropic_version_prefix;
#[cfg(test)]
use asb_core::{contracts::SettingValue, ownership::{default_client_settings, default_provider_parameters}};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairReport {
    pub repaired_files: Vec<String>,
    pub backup_path: Option<String>,
}

#[derive(Clone, Copy)]
enum FileKind {
    ClientSettings(AppKind),
    GenericProvider(AppKind),
    CodexProvider,
}

impl ConfigStore {
    /// Repairs only files whose meaning is unambiguous, then validates the
    /// complete staged store. The original directory is retained on success.
    pub fn repair(&self) -> Result<RepairReport, String> {
        if let Some(report) = self.recover_pending_repair()? { return Ok(report); }
        reject_legacy_and_pending(self)?;
        if let Ok(snapshot) = super::snapshot::read_configuration_snapshot(self) {
            if super::snapshot::validate_snapshot(&snapshot).is_ok() {
                return Ok(RepairReport { repaired_files: Vec::new(), backup_path: None });
            }
        }
        let mut transaction = transaction::Transaction::begin(self)?;
        let staged = transaction.staged();
        let prepared = (|| {
            copy_tree(&self.configuration_dir(), &staged)?;
            let repaired = repair_stage(&staged)?;
            let probe = ConfigStore::new(transaction.stage_root());
            let snapshot = super::snapshot::read_configuration_snapshot(&probe)
                .map_err(|error| format!("修复后的配置仍未通过校验：{}",
                    error.original_path(&staged, &self.configuration_dir())))?;
            super::snapshot::validate_snapshot(&snapshot)?;
            if repaired.is_empty() { return Err("配置无法通过当前修复规则恢复，原件已保留".into()); }
            Ok(repaired)
        })();
        let repaired = match prepared {
            Ok(files) => files,
            Err(error) => {
                transaction.discard().map_err(|cleanup| format!("{error}；暂存清理失败：{cleanup}"))?;
                return Err(error);
            }
        };
        transaction.prepare(repaired)?;
        transaction.commit()
    }
}

fn reject_legacy_and_pending(store: &ConfigStore) -> Result<(), String> {
    for path in store.retired_layout_markers() {
        if path.exists() {
            return Err(format!("旧格式无法无损修复：{}；原数据未改动", path.display()));
        }
    }
    for name in [SWITCH_INTENT_FILE, PROFILE_PREIMAGE_FILE, "save-journal.json"] {
        let path = store.configuration_dir().join(name);
        if path.exists() {
            return Err(format!("存在待恢复的配置事务：{}；请先完成事务恢复", path.display()));
        }
    }
    Ok(())
}

fn repair_stage(root: &Path) -> Result<Vec<String>, String> {
    let mut repaired = Vec::new();
    for app in [AppKind::Codex, AppKind::Claude] {
        let settings = root.join("client-settings").join(format!("{}.json", app.dir_name()));
        if settings.exists() && repair_file(root, &settings, FileKind::ClientSettings(app))? {
            repaired.push(relative(root, &settings));
        }
        let provider_dir = root.join("providers").join(app.dir_name());
        for path in json_files(&provider_dir)? {
            let kind = if app == AppKind::Codex {
                FileKind::CodexProvider
            } else {
                FileKind::GenericProvider(app)
            };
            if repair_file(root, &path, kind)? {
                repaired.push(relative(root, &path));
            }
        }
        if app == AppKind::Codex {
            for path in json_files(&provider_dir.join("official"))? {
                if repair_file(root, &path, FileKind::GenericProvider(app))? {
                    repaired.push(relative(root, &path));
                }
            }
        }
        let history_path = root.join("history").join(format!("{}.json", app.dir_name()));
        if history_path.exists() && repair_history(root, &history_path, app)? {
            repaired.push(relative(root, &history_path));
        }
    }
    Ok(repaired)
}

fn repair_file(root: &Path, path: &Path, kind: FileKind) -> Result<bool, String> {
    let (app, provider) = match kind {
        FileKind::ClientSettings(app) => (app, false),
        FileKind::GenericProvider(app) => (app, true),
        FileKind::CodexProvider => (AppKind::Codex, true),
    };
    let label = relative(root, path);
    let text = fs::read_to_string(path).map_err(|error| format!("{label} 不可读：{error}"))?;
    let source = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut value: Value = serde_json::from_str(source)
        .map_err(|error| format!("{label}：{}", json_diagnostic(&error)))?;
    let mut changed = false;
    if provider && app == AppKind::Claude {
        if let Some(options) = value.get_mut("modelOptions") {
            changed |= asb_core::contracts::upgrade_claude_model_options(options)?;
        }
    }
    if matches!(kind, FileKind::CodexProvider)
        && value.pointer("/profile/routeMode").and_then(Value::as_str) == Some("direct")
        && value.pointer("/profile/modelRoutes").and_then(Value::as_array).is_some_and(|routes| !routes.is_empty()) {
        value["profile"]["routeMode"] = Value::String("gateway".into());
        changed = true;
    }
    changed |= if provider {
        repair_provider_urls(kind, &mut value).map_err(|error| format!("{label}：{error}"))?
    } else {
        false
    };
    if provider && app == AppKind::Claude {
        changed |= repair_claude_provider(&mut value).map_err(|error| format!("{label}：{error}"))?;
    }
    let settings = if provider {
        value.get_mut("parameters").ok_or_else(|| format!("{label} 缺少供应商参数"))?
    } else {
        &mut value
    };
    changed |= fill_automatic(settings, app, provider)
        .map_err(|error| format!("{label}：{error}"))?
        || source.len() != text.len();
    if provider {
        inspect_provider(&label, kind, &value)?;
    } else {
        serde_json::from_value::<SettingsValues>(value.clone())
            .map_err(|error| format!("{label}：{}", json_diagnostic(&error)))?
            .validate_client_settings(app)
            .map_err(|error| format!("{label}：{error}"))?;
    }
    if changed {
        let json = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
        write_json_atomic(path, &json)?;
    }
    Ok(changed)
}

/// Repairs a terminal Anthropic version prefix using the endpoint owner's
/// source and result validation. Ambiguous candidates reject the transaction.
/// Full-URL mode and other
/// protocols keep their declared targets. Typed files retain serde as the
/// routing fields' owner; unrepairable inputs fail in `inspect_provider`.
fn repair_provider_urls(kind: FileKind, value: &mut Value) -> Result<bool, String> {
    match kind {
        FileKind::ClientSettings(_) => Ok(false),
        FileKind::CodexProvider => {
            let Ok(mut file) = serde_json::from_value::<CodexProviderFile>(value.clone()) else {
                return Ok(false);
            };
            if file.profile.upstream.protocol() != UpstreamProtocol::AnthropicMessages
                || file.profile.connection.is_full_url
            {
                return Ok(false);
            }
            let mut changed = false;
            if let Some(root) = strip_anthropic_version_prefix(&file.profile.endpoint.0) {
                file.profile.endpoint.0 = root;
                changed = true;
            }
            reject_version_prefixed_candidates(&file.profile.connection)?;
            write_back_if_repaired(value, &file, changed)
        }
        FileKind::GenericProvider(_) => {
            let Ok(mut file) = serde_json::from_value::<ProviderFile>(value.clone()) else {
                return Ok(false);
            };
            if file.route_mode != RouteMode::Custom
                || file.upstream_protocol != Some(UpstreamProtocol::AnthropicMessages)
                || file.connection.is_full_url
            {
                return Ok(false);
            }
            let mut changed = false;
            if let Some(root) = file
                .base_url
                .as_deref()
                .and_then(strip_anthropic_version_prefix)
            {
                file.base_url = Some(root);
                changed = true;
            }
            reject_version_prefixed_candidates(&file.connection)?;
            write_back_if_repaired(value, &file, changed)
        }
    }
}

fn reject_version_prefixed_candidates(connection: &ProviderConnectionOptions) -> Result<(), String> {
    if connection.custom_endpoints.keys().any(|url| strip_anthropic_version_prefix(url).is_some()) {
        return Err("自定义端点的目标含义无法确定，已保留全部候选和原始配置；请明确修改连接设置".into());
    }
    Ok(())
}

fn write_back_if_repaired<T: Serialize>(
    value: &mut Value,
    file: &T,
    changed: bool,
) -> Result<bool, String> {
    if !changed {
        return Ok(false);
    }
    *value = serde_json::to_value(file).map_err(|error| format!("供应商文件序列化失败：{error}"))?;
    Ok(true)
}

#[cfg(test)]
#[path = "repair/upgrade_tests.rs"]
mod upgrade_tests;
#[cfg(test)]
#[path = "repair/current_settings_tests.rs"]
mod current_settings_tests;

fn inspect_provider(label: &str, kind: FileKind, value: &Value) -> Result<(), String> {
    let invalid = |error: String| format!("{label}：{error}");
    match kind {
        FileKind::CodexProvider => {
            let file: CodexProviderFile = serde_json::from_value(value.clone())
                .map_err(|error| invalid(json_diagnostic(&error)))?;
            file.validate().map_err(invalid)?;
        }
        FileKind::GenericProvider(app) => {
            let file: ProviderFile = serde_json::from_value(value.clone())
                .map_err(|error| invalid(json_diagnostic(&error)))?;
            file.into_profile(app).validate().map_err(|error| invalid(error.to_string()))?;
        }
        FileKind::ClientSettings(_) => unreachable!("client settings are validated separately"),
    }
    Ok(())
}

fn repair_history(root: &Path, path: &Path, app: AppKind) -> Result<bool, String> {
    let label = relative(root, path);
    let text = fs::read_to_string(path).map_err(|error| format!("{label} 不可读：{error}"))?;
    let source = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let file: history::HistoryFile = serde_json::from_str(source)
        .map_err(|error| format!("{label}：{}", json_diagnostic(&error)))?;
    for record in &file.records {
        history::validate_write_record(app, record)
            .map_err(|error| format!("{label}：{error}"))?;
    }
    if source.len() != text.len() {
        write_json_atomic(path, source)?;
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_automatic_intent_is_repaired_with_original_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(directory.path().join("state"));
        let path = store.client_settings_path(AppKind::Codex);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut old = default_client_settings(AppKind::Codex);
        let missing = old.settings.keys().next().unwrap().clone();
        old.settings.remove(&missing);
        let original = serde_json::to_string_pretty(&old).unwrap();
        fs::write(&path, &original).unwrap();

        let report = store.repair().unwrap();

        assert_eq!(report.repaired_files, vec![format!("client-settings{}codex.json", std::path::MAIN_SEPARATOR)]);
        let backup = PathBuf::from(report.backup_path.unwrap());
        assert_eq!(fs::read_to_string(backup.join("client-settings/codex.json")).unwrap(), original);
        assert_eq!(store.get_client_settings(AppKind::Codex).unwrap().settings,
            default_client_settings(AppKind::Codex));
    }

    #[test]
    fn invalid_json_is_reported_without_replacing_the_original() {
        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(directory.path().join("state"));
        let path = store.client_settings_path(AppKind::Codex);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{broken").unwrap();

        let error = store.repair().unwrap_err();

        assert!(error.contains("client-settings"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "{broken");
        assert_eq!(fs::read_dir(store.state_root()).unwrap().count(), 1);
    }

    #[test]
    fn leading_bom_is_removed_without_changing_settings() {
        let directory = tempfile::tempdir().unwrap();
        let store = ConfigStore::new(directory.path().join("state"));
        let path = store.client_settings_path(AppKind::Codex);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let settings = default_client_settings(AppKind::Codex);
        let original = format!("\u{feff}{}", serde_json::to_string(&settings).unwrap());
        fs::write(&path, &original).unwrap();

        let report = store.repair().unwrap();

        assert_eq!(report.repaired_files.len(), 1);
        assert_eq!(fs::read_to_string(&path).unwrap().chars().next(), Some('{'));
        assert_eq!(store.get_client_settings(AppKind::Codex).unwrap().settings, settings);
        assert_eq!(fs::read_to_string(PathBuf::from(report.backup_path.unwrap())
            .join("client-settings/codex.json")).unwrap(), original);
    }
}
