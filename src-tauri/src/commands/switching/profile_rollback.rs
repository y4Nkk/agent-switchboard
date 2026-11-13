//! Durable pre-image of an explicitly confirmed active profile save.
use crate::local_state::LocalState;
use asb_core::{contracts::CodexProviderFile, AppKind, ProviderFile};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProfilePreimage {
    expected_after_hash: String,
    target: String,
    config_before_hash: String,
    config_after_hash: String,
    app: AppKind,
    file: ProfilePreimageFile,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum ProfilePreimageFile {
    Claude { file: ProviderFile },
    Codex { file: CodexProviderFile },
}
fn path(state: &LocalState) -> std::path::PathBuf {
    state
        .root()
        .join("configuration")
        .join(crate::config_store::PROFILE_PREIMAGE_FILE)
}

pub(crate) fn save(
    state: &LocalState,
    app: AppKind,
    file: &ProviderFile,
    candidate: &asb_core::ProviderProfile,
    config_before_hash: &str,
    config_after_hash: &str,
) -> Result<(), String> {
    let candidate_file = ProviderFile::from_profile(candidate, file.position);
    let candidate_json =
        serde_json::to_string_pretty(&candidate_file).map_err(|_| "供应商候选无法序列化")?;
    let expected_after_hash = crate::config_store::content_revision(candidate_json.as_bytes());
    let text = serde_json::to_string(&ProfilePreimage {
        expected_after_hash,
        target: state.target(app)?.to_string_lossy().into_owned(),
        config_before_hash: config_before_hash.into(),
        config_after_hash: config_after_hash.into(),
        app,
        file: ProfilePreimageFile::Claude { file: file.clone() },
    })
    .map_err(|_| "无法序列化供应商事务前像")?;
    let path = path(state);
    crate::config_store::write_json_atomic(&path, &text)?;
    Ok(())
}

pub(super) fn save_codex(
    state: &LocalState,
    file: &CodexProviderFile,
    candidate: &CodexProviderFile,
    config_before_hash: &str,
    config_after_hash: &str,
) -> Result<(), String> {
    let candidate_json =
        serde_json::to_string_pretty(candidate).map_err(|_| "Codex 供应商候选无法序列化")?;
    let expected_after_hash = crate::config_store::content_revision(candidate_json.as_bytes());
    let text = serde_json::to_string(&ProfilePreimage {
        expected_after_hash,
        target: state.target(AppKind::Codex)?.to_string_lossy().into_owned(),
        config_before_hash: config_before_hash.into(),
        config_after_hash: config_after_hash.into(),
        app: AppKind::Codex,
        file: ProfilePreimageFile::Codex { file: file.clone() },
    })
    .map_err(|_| "无法序列化 Codex 供应商事务前像")?;
    crate::config_store::write_json_atomic(&path(state), &text)
}
pub(super) fn restore(
    state: &LocalState,
    profile_id: &str,
    expected_hash: &str,
) -> Result<(), String> {
    let text = std::fs::read_to_string(path(state)).map_err(|_| "供应商事务前像不可读")?;
    let before: ProfilePreimage = serde_json::from_str(&text).map_err(|_| "供应商事务前像无效")?;
    match before.file {
        ProfilePreimageFile::Claude { file } => {
            let record = state
                .configuration()
                .find_provider_record(profile_id)
                .map_err(|error| error.to_string())?;
            if before.app != record.profile.app || file.id != profile_id {
                return Err("供应商事务前像身份不匹配".into());
            }
            if record.profile == file.clone().into_profile(before.app) {
                return Ok(());
            }
            if record.file_hash != expected_hash {
                return Err("待补偿供应商已发生额外修改，拒绝覆盖".into());
            }
            state
                .configuration()
                .overwrite_provider_file(before.app, file)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        ProfilePreimageFile::Codex { file } => {
            if before.app != AppKind::Codex || file.profile.id != profile_id {
                return Err("Codex 供应商事务前像身份不匹配".into());
            }
            let record = state
                .configuration()
                .list_codex_providers()
                .map_err(|error| error.to_string())?
                .into_iter()
                .find(|record| record.profile.id == profile_id)
                .ok_or_else(|| "Codex 供应商不存在".to_string())?;
            if record.profile == file.profile
                && record.parameters == file.parameters
                && record.notes == file.notes
                && record.website_url == file.website_url
                && record.usage_query == file.usage_query
            {
                return Ok(());
            }
            if record.file_hash != expected_hash {
                return Err("待补偿 Codex 供应商已发生额外修改，拒绝覆盖".into());
            }
            state
                .configuration()
                .overwrite_codex_provider_file(file)
                .map_err(|error| error.to_string())
        }
    }
}
pub(super) fn clear(state: &LocalState) -> Result<(), String> {
    match std::fs::remove_file(path(state)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("供应商事务已完成，但前像清理失败".into()),
    }
}

pub(super) fn validate_saved_revision(
    state: &LocalState,
    id: &str,
    hash: &str,
) -> Result<(), String> {
    let text = std::fs::read_to_string(path(state))
        .map_err(|_| "供应商确认版本前像不可读，不能自动采用当前版本")?;
    let before: ProfilePreimage =
        serde_json::from_str(&text).map_err(|_| "供应商确认版本前像无效")?;
    let matches_id = match &before.file {
        ProfilePreimageFile::Claude { file } => file.id == id,
        ProfilePreimageFile::Codex { file } => file.profile.id == id,
    };
    if !matches_id || before.expected_after_hash != hash {
        return Err("供应商版本既非保存前版本，也非已确认候选，拒绝恢复最新修改".into());
    }
    Ok(())
}

pub(super) fn validate_projection(
    state: &LocalState,
    app: AppKind,
    preview: &asb_switch::FilePreview,
) -> Result<(), String> {
    let text = std::fs::read_to_string(path(state)).map_err(|_| "供应商确认配置前像不可读")?;
    let before: ProfilePreimage =
        serde_json::from_str(&text).map_err(|_| "供应商确认配置前像无效")?;
    if before.app != app
        || std::path::Path::new(&before.target) != state.target(app)?
        || before.config_before_hash != preview.content_hash
        || before.config_after_hash != preview.rendered_hash
    {
        return Err("客户端配置或设置已偏离保存时确认的快照，拒绝重新解释待恢复保存".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::{
        CodexCapabilities, CodexCatalogEntry, CodexEndpoint, CodexModelRoute, CodexProviderDraft,
        CodexUpstream,
    };

    fn draft(name: &str) -> CodexProviderDraft {
        CodexProviderDraft {
            name: name.to_string(),
            endpoint: CodexEndpoint("https://relay.example/v1".to_string()),
            api_key: "fixture-key".to_string(),
            authentication: None,
            connection: Default::default(),
            upstream: CodexUpstream::Responses,
            request_mode: asb_core::contracts::ResponsesRequestMode::Standard,
            default_model: "codex".to_string(),
            catalog: vec![CodexCatalogEntry {
                id: "codex".to_string(),
                context_window: 128_000,
                max_output_tokens: 16_384,
                function_tools: true,
                custom_tools: true,
                tool_search: true,
                reasoning: true,
                default_reasoning_level: asb_core::contracts::CodexReasoningLevel::High,
                supported_reasoning_levels: vec![
                    asb_core::contracts::CodexReasoningLevel::None,
                    asb_core::contracts::CodexReasoningLevel::High,
                ],
                images: true,
                compact: true,
                display_name: None,
                description: None,
                base_instructions: None,
                supports_parallel_tool_calls: None,
            }],
            model_routes: vec![CodexModelRoute {
                client_model: "codex".to_string(),
                upstream_model: "vendor-codex".to_string(),
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
    }

    #[test]
    fn codex_preimage_restores_only_the_confirmed_specialized_revision() {
        let _paths = crate::test_client_paths::redirect_client_paths();
        let directory = tempfile::tempdir().unwrap();
        let state = LocalState::from_root(directory.path().join("state"));
        let created = state
            .configuration()
            .create_codex_provider(uuid::Uuid::new_v4().to_string(), draft("before"))
            .unwrap();
        let original = state
            .configuration()
            .find_codex_provider_file(&created.profile.id)
            .unwrap();
        let candidate = draft("after").into_file(created.profile.id.clone(), original.position);
        save_codex(&state, &original, &candidate, "before", "after").unwrap();
        let saved = state
            .configuration()
            .update_codex_provider(&created.profile.id, draft("after"), &created.file_hash)
            .unwrap();
        validate_saved_revision(&state, &created.profile.id, &saved.file_hash).unwrap();
        restore(&state, &created.profile.id, &saved.file_hash).unwrap();
        let restored = state
            .configuration()
            .find_codex_provider_file(&created.profile.id)
            .unwrap();
        assert_eq!(restored, original);
    }
}
