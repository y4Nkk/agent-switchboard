//! Per-profile application of the existing visual Codex common file.
//! Settings values remain exclusively in client-settings/codex.json.
use asb_core::{
    contracts::{ClientSettingsSnapshot, SettingsValues},
    AppKind,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path, sync::Mutex};

mod fragment;
pub(crate) use fragment::resolve as resolve_fragment;
pub(crate) use fragment::view as view_fragment;
#[allow(unused_imports)] // reserved: fragment write path (test-covered)
pub(crate) use fragment::save as save_fragment;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CommonPolicy {
    pub version: u8,
    pub disabled_profile_ids: BTreeSet<String>,
}
impl Default for CommonPolicy {
    fn default() -> Self {
        Self {
            version: 1,
            disabled_profile_ids: BTreeSet::new(),
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommonView {
    pub policy: CommonPolicy,
    pub revision: String,
    pub settings: ClientSettingsSnapshot,
    pub fragment: fragment::FragmentView,
}
#[allow(dead_code)] // reserved: fragment write path (test-covered)
static LOCK: Mutex<()> = Mutex::new(());
fn path(root: &Path) -> std::path::PathBuf {
    root.join("codex/common-config-policy.json")
}
fn load(root: &Path) -> Result<(CommonPolicy, String), String> {
    let raw =
        crate::config_store::read_optional(&path(root)).map_err(|_| "Codex 通用配置策略不可读")?;
    let policy: CommonPolicy = match &raw {
        None => CommonPolicy::default(),
        Some(raw) => serde_json::from_str(raw).map_err(|_| "Codex 通用配置策略无效")?,
    };
    if policy.version != 1
        || policy
            .disabled_profile_ids
            .iter()
            .any(|id| uuid::Uuid::parse_str(id).is_err())
    {
        return Err("Codex 通用配置策略版本或供应商标识无效".into());
    }
    Ok((policy, asb_switch::sha256_hex(raw.as_deref().unwrap_or(""))))
}
pub(crate) fn view(state: &crate::local_state::LocalState) -> Result<CommonView, String> {
    let (policy, revision) = load(state.root())?;
    let settings = state
        .configuration()
        .get_client_settings(AppKind::Codex)
        .map_err(|e| e.to_string())?;
    let fragment = fragment::view(state.root())?;
    Ok(CommonView {
        policy,
        revision,
        settings,
        fragment,
    })
}
pub(crate) fn resolve(
    state: &crate::local_state::LocalState,
    profile_id: &str,
) -> Result<SettingsValues, String> {
    let view = view(state)?;
    Ok(if view.policy.disabled_profile_ids.contains(profile_id) {
        asb_core::ownership::default_client_settings(AppKind::Codex)
    } else {
        view.settings.settings
    })
}
#[allow(dead_code)] // reserved: fragment enable toggle (test-covered)
pub(crate) fn set_enabled(
    state: &crate::local_state::LocalState,
    profile_id: &str,
    enabled: bool,
    expected: &str,
) -> Result<CommonView, String> {
    let _guard = LOCK.lock().map_err(|_| "Codex 通用配置策略锁不可用")?;
    let (mut policy, revision) = load(state.root())?;
    if revision != expected {
        return Err("Codex 通用配置应用策略已变化，请重新读取".into());
    }
    let configuration = state.configuration();
    if configuration.find_codex_provider_file(profile_id).is_err()
        && !configuration
            .find_provider(profile_id)
            .is_ok_and(|p| p.app == AppKind::Codex)
    {
        return Err("Codex 供应商不存在".into());
    }
    if enabled {
        policy.disabled_profile_ids.remove(profile_id);
    } else {
        policy.disabled_profile_ids.insert(profile_id.into());
    }
    crate::config_store::write_json_atomic(
        &path(state.root()),
        &serde_json::to_string_pretty(&policy).map_err(|_| "Codex 通用配置策略无法序列化")?,
    )?;
    view(state)
}
#[allow(dead_code)] // reserved: fragment extraction (test-covered)
pub(crate) fn extract(target: &Path) -> Result<SettingsValues, String> {
    let text =
        std::fs::read_to_string(target).map_err(|_| "无法读取 Codex 原生配置，请先检查配置文件")?;
    asb_core::adapter::codex::extract_client_settings(&text).map_err(|e| e.to_string())
}
/// 通用片段当前声明的 `mcp_servers` 顶层键（空片段为空集）。供扩展执行
/// 侧的争写闸口读取；片段存储或片段文本不可解析时按 Err 透传，调用方
/// fail-closed 拒绝——无法证明无冲突就不执行。
pub(crate) fn declared_fragment_mcp_keys(
    state: &crate::local_state::LocalState,
) -> Result<Vec<String>, String> {
    let fragment = fragment::view(state.root())?;
    asb_core::adapter::codex::declared_mcp_server_keys(&fragment.text).map_err(|e| e.to_string())
}
#[cfg(test)]
pub(crate) fn test_draft() -> asb_core::contracts::CodexProviderDraft {
    use asb_core::contracts::{
        CodexCatalogEntry, CodexEndpoint, CodexProviderDraft, CodexUpstream,
        ResponsesRequestMode, DEFAULT_CODEX_CAPABILITIES,
    };
    CodexProviderDraft {
        name: "Relay".into(),
        endpoint: CodexEndpoint("https://relay.example/v1".into()),
        api_key: "fixture-key".into(),
        authentication: None,
        connection: Default::default(),
        upstream: CodexUpstream::Responses,
        request_mode: ResponsesRequestMode::Standard,
        default_model: "relay-model".into(),
        catalog: vec![CodexCatalogEntry::default_entry("relay-model")],
        model_routes: Vec::new(),
        subagent_route: None,
        capabilities: DEFAULT_CODEX_CAPABILITIES,
        parameters: asb_core::ownership::default_provider_parameters(AppKind::Codex),
        notes: None,
        website_url: None,
        usage_query: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_only_changes_codex_projection_and_never_copies_settings_into_providers() {
        let directory = tempfile::tempdir().unwrap();
        let state = crate::local_state::LocalState::from_root(directory.path().join("state"));
        let file = state
            .configuration()
            .create_codex_provider(uuid::Uuid::new_v4().to_string(),
                test_draft(),
            )
            .unwrap();
        let initial = view(&state).unwrap();
        let mut settings = initial.settings.settings.clone();
        settings.settings.insert(
            "approval_policy".into(),
            asb_core::SettingValue::Explicit {
                value: asb_core::ConfigValue::Str("on-request".into()),
            },
        );
        state
            .configuration()
            .save_client_settings(
                AppKind::Codex,
                settings.clone(),
                &initial.settings.settings_hash,
            )
            .unwrap();
        assert_eq!(resolve(&state, &file.profile.id).unwrap(), settings);
        let disabled = set_enabled(&state, &file.profile.id, false, &initial.revision).unwrap();
        assert_eq!(
            resolve(&state, &file.profile.id).unwrap(),
            asb_core::ownership::default_client_settings(AppKind::Codex)
        );
        assert_eq!(disabled.settings.settings, settings);
        assert_eq!(
            state.configuration().list_codex_providers().unwrap()[0].file_hash,
            file.file_hash
        );
        assert!(set_enabled(&state, &file.profile.id, true, &initial.revision).is_err());
    }
}
