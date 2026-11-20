//! Safe switch-away backfill for the specialized Codex provider store.
//!
//! The live file is a projection, not a second provider database. Only typed
//! provider-owned values that can be read without credentials are adopted.

use crate::commands::error::CommandError;
use crate::config_store::{ConfigStore, ProfileStoreError};
use crate::gateway::GatewayController;
use crate::local_state::LocalState;
use asb_core::contracts::{CodexProviderFile, ConfigValue, SettingValue, SettingsValues};
use asb_core::ownership::{setting_specs, SettingControl, SettingOwner};
use asb_core::AppKind;
use std::fs;
use toml_edit::{DocumentMut, Item};

pub(super) struct PreparedCodexBackfill {
    pub(super) profile_id: String,
    pub(super) before: CodexProviderFile,
    pub(super) before_hash: String,
    pub(super) candidate: CodexProviderFile,
    pub(super) after_hash: String,
}

pub(super) struct AppliedCodexBackfill {
    pub(super) profile_id: String,
    pub(super) before: CodexProviderFile,
    pub(super) before_hash: String,
    pub(super) after_hash: String,
}

/// Finds the outgoing routed Codex profile and computes a typed candidate.
/// An unmanaged or externally changed live file is intentionally a no-op.
pub(super) fn prepare(
    state: &LocalState,
    gateway: &GatewayController,
    selected_profile_id: &str,
    expected_config_hash: &str,
) -> Result<Option<PreparedCodexBackfill>, CommandError> {
    let target = state
        .target(AppKind::Codex)
        .map_err(|error| CommandError::new("config-path-unavailable", error))?;
    let text = match fs::read_to_string(target) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            return Err(CommandError::keyed(
                "codex-live-backfill-read-failed",
                "errors.sw.codexLiveReadFailed",
                "无法读取当前 Codex 配置，已取消切换以保护供应商档案",
            ));
        }
    };
    if asb_switch::sha256_hex(&text) != expected_config_hash {
        return Err(CommandError::keyed(
            "switch-stale",
            "errors.sw.codexLiveChanged",
            "当前 Codex 配置已变化，请重新预览后再切换",
        ));
    }
    let Some(profile_id) = gateway
        .active_profile_id(AppKind::Codex, &text)
        .map_err(|error| CommandError::new("codex-live-backfill-identity-failed", error))?
    else {
        return Ok(None);
    };
    if profile_id == selected_profile_id {
        return Ok(None);
    }
    let (before, before_hash) = state
        .configuration()
        .find_codex_provider_with_revision(&profile_id)
        .map_err(|error| match error {
            ProfileStoreError::NotFound { .. } => CommandError::keyed(
                "codex-live-backfill-profile-failed",
                "errors.sw.codexLiveProfileMissing",
                "当前 Codex 网关路由找不到对应供应商档案",
            ),
            error => CommandError::new("codex-live-backfill-profile-failed", error.to_string()),
        })?;
    let candidate = merge_live(&before, &text).map_err(|error| {
        CommandError::localized(
            "codex-live-backfill-invalid",
            "errors.sw.codexBackfillFailed",
            format!("无法回填 Codex 供应商：{error}"),
            serde_json::json!({ "error": error }),
        )
    })?;
    if candidate == before {
        return Ok(None);
    }
    let after_hash = ConfigStore::codex_provider_revision(&candidate).map_err(|error| {
        let detail = error.to_string();
        CommandError::localized(
            "codex-live-backfill-invalid",
            "errors.sw.codexBackfillRevisionFailed",
            format!("无法计算 Codex 供应商回填版本：{detail}"),
            serde_json::json!({ "error": detail }),
        )
    })?;
    Ok(Some(PreparedCodexBackfill {
        profile_id,
        before,
        before_hash,
        candidate,
        after_hash,
    }))
}

pub(super) fn apply(
    state: &LocalState,
    prepared: PreparedCodexBackfill,
) -> Result<AppliedCodexBackfill, CommandError> {
    let record = state
        .configuration()
        .update_codex_provider_file(prepared.candidate, &prepared.before_hash)
        .map_err(|error| CommandError::new("codex-live-backfill-save-failed", error.to_string()))?;
    if record.file_hash != prepared.after_hash {
        return Err(CommandError::keyed(
            "codex-live-backfill-save-failed",
            "errors.sw.codexBackfillRevisionMismatch",
            "Codex 供应商回填版本与 durable 事务记录不一致",
        ));
    }
    Ok(AppliedCodexBackfill {
        profile_id: prepared.profile_id,
        before: prepared.before,
        before_hash: prepared.before_hash,
        after_hash: record.file_hash,
    })
}

pub(super) fn restore(state: &LocalState, backfill: &AppliedCodexBackfill) -> Result<(), String> {
    let (current, revision) = state
        .configuration()
        .find_codex_provider_with_revision(&backfill.profile_id)
        .map_err(|error| error.to_string())?;
    if revision == backfill.before_hash && current == backfill.before {
        return Ok(());
    }
    if revision != backfill.after_hash || backfill.before.profile.id != backfill.profile_id {
        return Err("回填后的 Codex 供应商已被额外修改，拒绝覆盖".into());
    }
    state
        .configuration()
        .update_codex_provider_file(backfill.before.clone(), &revision)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub(super) fn merge_live(
    source: &CodexProviderFile,
    live_text: &str,
) -> Result<CodexProviderFile, String> {
    let document = live_text
        .parse::<DocumentMut>()
        .map_err(|error| format!("TOML 格式无效：{error}"))?;
    let mut candidate = source.clone();
    merge_model_projection(&mut candidate, &document)?;
    merge_provider_parameters(
        &mut candidate.parameters,
        &document,
        candidate.profile.upstream,
    );
    candidate
        .validate()
        .map_err(|error| format!("回填结果未通过 Codex 档案校验：{error}"))?;
    Ok(candidate)
}

fn merge_model_projection(
    file: &mut CodexProviderFile,
    document: &DocumentMut,
) -> Result<(), String> {
    if let Some(model) = top_string(document, "model") {
        if file.profile.catalog.iter().any(|entry| entry.id == model) {
            file.profile.default_model = model;
        }
    }
    if let Some(item) = item_at(document, "agents.default_subagent_model") {
        let live = item
            .as_value()
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let route = asb_core::contracts::CodexSubagentRoute::parse_wire_id(live)
            .filter(|route| route.has_canonical_profile_id() && !route.model.trim().is_empty())
            .ok_or_else(|| {
                format!("实时 config.toml 的 agents.default_subagent_model 不是有效的子代理路由引用：{live:?}")
            })?;
        file.profile.subagent_route = Some(route);
    }
    let Some(context_window) = top_positive_integer(document, "model_context_window") else {
        return Ok(());
    };
    let model = &file.profile.default_model;
    if let Some(entry) = file
        .profile
        .catalog
        .iter_mut()
        .find(|entry| &entry.id == model)
    {
        entry.context_window = context_window;
    }
    Ok(())
}

fn merge_provider_parameters(
    parameters: &mut SettingsValues,
    document: &DocumentMut,
    upstream: asb_core::contracts::CodexUpstream,
) {
    let mut values = parameters.settings.clone();
    for spec in setting_specs(AppKind::Codex)
        .into_iter()
        .filter(|spec| spec.owner == SettingOwner::Provider && spec.control != SettingControl::None)
    {
        let synthetic_web_search = spec.key == asb_core::ownership::CODEX_WEB_SEARCH_KEY
            && upstream != asb_core::contracts::CodexUpstream::Responses;
        if synthetic_web_search {
            continue;
        }
        let value = live_setting(document, &spec.key, &spec);
        if let Some(value) = value {
            values.insert(spec.key.to_string(), value);
        }
    }
    parameters.settings = values;
}

fn live_setting(
    document: &DocumentMut,
    key: &str,
    spec: &asb_core::ownership::SettingSpec,
) -> Option<SettingValue> {
    let item = item_at(document, key)?;
    let value = config_value(item, key)?;
    accepts(spec, &value).then_some(SettingValue::Explicit { value })
}

fn accepts(spec: &asb_core::ownership::SettingSpec, value: &ConfigValue) -> bool {
    spec.control != SettingControl::None && asb_core::validate::validate_setting_value(spec, value).is_ok()
}

fn config_value(item: &Item, key: &str) -> Option<ConfigValue> {
    asb_core::adapter::codex::toml_value(item.as_value()?, key).ok()
}

fn item_at<'a>(document: &'a DocumentMut, path: &str) -> Option<&'a Item> {
    let mut item = document.as_item();
    for segment in path.split('.') {
        item = item.as_table_like()?.get(segment)?;
    }
    Some(item)
}

fn top_string(document: &DocumentMut, key: &str) -> Option<String> {
    item_at(document, key)
        .and_then(Item::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn top_positive_integer(document: &DocumentMut, key: &str) -> Option<u64> {
    item_at(document, key)
        .and_then(Item::as_integer)
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::{
        CodexCapabilities, CodexCatalogEntry, CodexEndpoint, CodexModelRoute, CodexProviderDraft,
        CodexReasoningLevel, CodexUpstream, ResponsesRequestMode,
    };

    fn source() -> CodexProviderFile {
        CodexProviderDraft {
            name: "relay".into(),
            endpoint: CodexEndpoint("https://relay.example/v1".into()),
            api_key: "fixture-key".into(),
            authentication: None,
            connection: Default::default(),
            upstream: CodexUpstream::ChatCompletions,
            request_mode: ResponsesRequestMode::Standard,
            default_model: "model-a".into(),
            catalog: vec![CodexCatalogEntry {
                id: "model-a".into(),
                context_window: 128_000,
                max_output_tokens: 16_384,
                function_tools: true,
                custom_tools: true,
                tool_search: true,
                reasoning: true,
                default_reasoning_level: CodexReasoningLevel::High,
                supported_reasoning_levels: vec![
                    CodexReasoningLevel::None,
                    CodexReasoningLevel::High,
                ],
                images: false,
                compact: true,
                display_name: None,
                description: None,
                base_instructions: None,
                supports_parallel_tool_calls: None,
            }],
            model_routes: vec![CodexModelRoute {
                client_model: "model-a".into(),
                upstream_model: "vendor-a".into(),
            }],
            subagent_route: None,
            capabilities: CodexCapabilities {
                responses: true,
                compact: true,
                models: true,
                chat_completions: true,
                alpha_search: false,
                image_generation: false,
                image_edit: false,
                function_tools: true,
                custom_tools: true,
                tool_search: true,
                reasoning: true,
                chat_reasoning: asb_core::contracts::CodexChatReasoning::Unsupported,
            },
            parameters: asb_core::ownership::default_provider_parameters(AppKind::Codex),
            notes: None,
            website_url: None,
            usage_query: None,
        }
        .into_file("00000000-0000-0000-0000-000000000001".into(), 100)
    }

    #[test]
    fn backfill_adopts_model_context_and_provider_settings() {
        let live = r#"
# host-owned content remains in the live document
model = "model-a"
model_context_window = 256000
model_reasoning_effort = "high"
hide_agent_reasoning = true
custom_host_key = "keep"
"#;
        let result = merge_live(&source(), live).unwrap();
        assert_eq!(result.profile.catalog[0].context_window, 256_000);
        assert_eq!(
            result.parameters.settings["model_reasoning_effort"],
            SettingValue::Explicit {
                value: ConfigValue::Str("high".into())
            }
        );
        assert_eq!(
            result.parameters.settings["hide_agent_reasoning"],
            SettingValue::Explicit {
                value: ConfigValue::Bool(true)
            }
        );
    }

    #[test]
    fn backfill_does_not_persist_synthetic_web_search_disable() {
        let mut source = source();
        source.parameters.settings.insert(
            asb_core::ownership::CODEX_WEB_SEARCH_KEY.into(),
            SettingValue::Explicit {
                value: ConfigValue::Str("indexed".into()),
            },
        );
        let result = merge_live(&source, "web_search = \"disabled\"\n").unwrap();
        assert_eq!(
            result.parameters.settings[asb_core::ownership::CODEX_WEB_SEARCH_KEY],
            source.parameters.settings[asb_core::ownership::CODEX_WEB_SEARCH_KEY]
        );
    }

    #[test]
    fn backfill_keeps_stored_catalog_when_live_has_no_catalog_pointer() {
        let source = source();
        let result = merge_live(&source, "model = \"model-a\"\n").unwrap();
        assert_eq!(result.profile.catalog, source.profile.catalog);
    }

    #[test]
    fn backfill_keeps_explicit_provider_settings_when_live_omits_them() {
        let mut source = source();
        source.parameters.settings.insert(
            "model_reasoning_effort".into(),
            SettingValue::Explicit {
                value: ConfigValue::Str("high".into()),
            },
        );
        let result = merge_live(&source, "model = \"model-a\"\n").unwrap();
        assert_eq!(
            result.parameters.settings["model_reasoning_effort"],
            source.parameters.settings["model_reasoning_effort"]
        );
    }
}
