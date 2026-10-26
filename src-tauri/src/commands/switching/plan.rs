use crate::commands::error::{operation_error, store_error, CommandError};
use asb_core::contracts::{AppKind, ProviderProfile, SwitchPlan};
use asb_switch::io::FsIo;
use asb_switch::read_preview;
use std::fs;
use std::path::Path;

/// Official Codex is not a third-party profile: switching to it targets the
/// stored official-login record, whose direct plan is built by
/// `build_plan_for_profile` exactly like any other profile.
pub(in crate::commands) fn build_plan(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    profile_id: &str,
) -> Result<crate::gateway::GatewayProjection, CommandError> {
    let saved = state.configuration().resolve_saved_provider(profile_id)
        .map_err(|error| operation_error("profile-not-found", error))?;
    match saved {
        crate::config_store::SavedProvider::Codex { file, .. } => build_codex_plan(state, gateway, file),
        crate::config_store::SavedProvider::Client(record) => build_plan_for_profile(state, gateway, record.profile, None),
    }
}

pub(super) fn build_codex_plan(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    file: asb_core::contracts::CodexProviderFile,
) -> Result<crate::gateway::GatewayProjection, CommandError> {
    let client_settings = crate::codex_common::resolve(state, &file.profile.id)
        .map_err(|error| CommandError::new("codex-common-config-invalid", error))?;
    gateway
        .project_codex(&file, client_settings, None)
        .map_err(|error| CommandError::new("gateway-projection-invalid", error))
}

pub(super) fn build_plan_for_profile(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    profile: ProviderProfile,
    replacement: Option<&ProviderProfile>,
) -> Result<crate::gateway::GatewayProjection, CommandError> {
    let client_settings = if profile.app == AppKind::Codex {
        crate::codex_common::resolve(state, &profile.id)
            .map_err(|error| CommandError::new("codex-common-config-invalid", error))?
    } else {
        state
            .configuration()
            .get_client_settings(profile.app)
            .map_err(store_error)?
            .settings
    };
    let mut plan = SwitchPlan::direct(profile, client_settings);
    if plan.app() == AppKind::Codex {
        // 片段随计划携带：停用档也要带文本，投影才能剥离已合并的键。
        if let Some(fragment) = crate::codex_common::resolve_fragment(state.root(), &plan.profile.id)
            .map_err(|error| CommandError::new("codex-common-config-invalid", error))?
        {
            plan = plan.with_codex_common_fragment(fragment);
        }
    }
    if plan.app() == AppKind::Codex && plan.profile.route_mode == asb_core::RouteMode::Official {
        plan = crate::codex_auth::projection::official_plan(state, plan)
            .map_err(|error| CommandError::new("codex-official-login-required", error))?;
        let (policy, _) = crate::gateway::codex::policy::load(state.root())
            .map_err(|error| CommandError::new("codex-policy-invalid", error))?;
        if policy.takeover && plan.codex_managed_auth().is_some() {
            return gateway
                .project_codex_official_takeover(plan, &policy)
                .map_err(|error| CommandError::new("gateway-projection-invalid", error));
        }
    }
    asb_core::validate_plan(&plan.profile, &plan.client_settings)
        .map_err(|error| CommandError::new("invalid-plan", error.to_string()))?;
    gateway
        .project_with_candidates(state, &plan, replacement)
        .map_err(|error| CommandError::new("gateway-projection-invalid", error))
}

pub(in crate::commands) fn preview_projection(
    state: &crate::local_state::LocalState,
    projection: &crate::gateway::GatewayProjection,
) -> Result<asb_switch::FilePreview, CommandError> {
    let plan = &projection.plan;
    let target = state
        .target(plan.app())
        .map_err(|error| CommandError::new("config-path-unavailable", error))?;
    let backup_dir = state.backup_dir();
    let mut preview = read_preview(&FsIo, &target, plan, &backup_dir.to_string_lossy())
        .map_err(CommandError::from)?;
    if plan.app() == AppKind::Codex && plan.profile.route_mode == asb_core::RouteMode::Custom {
        preview
            .preview
            .warnings
            .push(if plan.codex_preserve_official_login() {
                asb_core::contracts::LocalizedMessage::new(
                    "warnings.codex.keepOfficialLogin",
                    serde_json::json!({}),
                    "第三方认证与 ChatGPT 登录分开；保留现有官方令牌，缺少登录时网关使用本机专用凭据",
                )
            } else {
                asb_core::contracts::LocalizedMessage::new(
                    "warnings.codex.dropOfficialLogin",
                    serde_json::json!({}),
                    "已选择不保留官方登录：本次切换会备份并移除 auth.json 中的 OAuth 令牌",
                )
            });
    }
    if plan.codex_managed_auth().is_some() {
        preview.preview.warnings.push(asb_core::contracts::LocalizedMessage::new(
            "warnings.codex.managedAuth",
            serde_json::json!({}),
            "将使用绑定的 Codex 托管账号；认证文件独立备份、校验并可恢复，令牌不会显示在预览中",
        ));
    }
    if let Some(warning) = projection.warning() {
        preview.preview.warnings.push(asb_core::contracts::LocalizedMessage::new(
            "warnings.gateway.freeform",
            serde_json::json!({ "detail": warning }),
            warning.to_string(),
        ));
    }
    if let Some(catalog) = &projection.codex_catalog {
        preview.preview.warnings.push(asb_core::contracts::LocalizedMessage::new(
            "warnings.codex.catalogCreated",
            serde_json::json!({ "fileName": catalog.file_name }),
            format!(
                "将生成并写入 ASB 管理的 Codex 模型目录 {}，并由 model_catalog_json 指向该文件。",
                catalog.file_name
            ),
        ));
    } else if plan.app() == AppKind::Codex
        && plan.profile.route_mode == asb_core::RouteMode::Official
        && official_catalog_artifact(&target)?.is_some()
    {
        preview.preview.warnings.push(asb_core::contracts::LocalizedMessage::new(
            "warnings.codex.catalogCleanup",
            serde_json::json!({}),
            "将清理当前 model_catalog_json 指向的 ASB 管理模型目录；用户自己的目录文件不会被删除。",
        ));
    }
    preview.preview.target = target.to_string_lossy().to_string();
    Ok(preview)
}

pub(super) fn execute_projection(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    projection: &crate::gateway::GatewayProjection,
    expected_hash: &str,
    expected_rendered_hash: &str,
) -> Result<asb_switch::SwitchOutcome, CommandError> {
    let auth_preview = if projection.plan.app() == AppKind::Codex {
        let target = state
            .target(AppKind::Codex)
            .map_err(|error| CommandError::new("config-path-unavailable", error))?;
        let preview = read_preview(
            &FsIo,
            &target,
            &projection.plan,
            &state.backup_dir().to_string_lossy(),
        )
        .map_err(CommandError::from)?;
        if preview.content_hash != expected_hash || preview.rendered_hash != expected_rendered_hash
        {
            return Err(CommandError::keyed(
                "switch-stale",
                "errors.sw.codexPreviewStale",
                "Codex 配置预览已失效，请重新查看差异",
            ));
        }
        Some(preview)
    } else {
        None
    };
    execute_projection_with_auth(
        state,
        gateway,
        projection,
        expected_hash,
        expected_rendered_hash,
        auth_preview
            .as_ref()
            .and_then(|preview| preview.auth_hash.as_deref()),
        auth_preview
            .as_ref()
            .and_then(|preview| preview.auth_existed),
        auth_preview
            .as_ref()
            .and_then(|preview| preview.auth_rendered_hash.as_deref()),
    )
}

pub(in crate::commands) fn execute_projection_with_auth(
    state: &crate::local_state::LocalState,
    gateway: &crate::gateway::GatewayController,
    projection: &crate::gateway::GatewayProjection,
    expected_hash: &str,
    expected_rendered_hash: &str,
    expected_auth_hash: Option<&str>,
    expected_auth_existed: Option<bool>,
    expected_auth_rendered_hash: Option<&str>,
) -> Result<asb_switch::SwitchOutcome, CommandError> {
    let plan = &projection.plan;
    let _account_guard = plan
        .codex_managed_auth()
        .map(|auth| {
            crate::codex_auth::projection::commit_guard(state.root(), &plan.profile.id, auth)
                .map_err(|error| CommandError::new("codex-account-preview-stale", error))
        })
        .transpose()?;
    let target = state
        .target(plan.app())
        .map_err(|error| CommandError::new("config-path-unavailable", error))?;
    if plan.app() == AppKind::Codex
        && plan.profile.route_mode == asb_core::RouteMode::Official
        && plan.codex_managed_auth().is_none()
    {
        crate::codex_auth::projection::require_native(&target)
            .map_err(|error| CommandError::new("codex-official-login-required", error))?;
    }
    let auth = super::projection_transaction::auth_intent(
        expected_auth_hash,
        expected_auth_existed,
        expected_auth_rendered_hash,
    )?;
    let preservation = super::projection_transaction::begin(
        state,
        gateway,
        projection,
        &target,
        expected_hash,
        expected_rendered_hash,
        auth,
    )?;
    let request = asb_switch::SwitchRequest {
        target: &target,
        plan,
        backup_dir: &state.backup_dir(),
        expected_hash,
        expected_rendered_hash,
    };
    let commit = |outcome: &asb_switch::SwitchOutcome| {
        super::projection_transaction::commit(state, gateway, projection, outcome)
    };
    let execution = if plan.app() == AppKind::Codex {
        asb_switch::execute_codex(
            &FsIo,
            &request,
            expected_auth_hash,
            expected_auth_existed,
            expected_auth_rendered_hash,
            commit,
        )
    } else {
        asb_switch::execute(&FsIo, &request, commit)
    };
    let mut outcome = super::transaction::finish(state, gateway, execution)?;
    outcome.warnings.extend(preservation);
    outcome.preview.target = target.to_string_lossy().into_owned();
    Ok(outcome)
}

pub(super) fn official_catalog_artifact(
    config_target: &Path,
) -> Result<Option<super::transaction::CatalogArtifact>, CommandError> {
    let text = match fs::read_to_string(config_target) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => {
            return Err(CommandError::keyed(
                "codex-catalog-unreadable",
                "errors.sw.codexConfigUnreadableForCatalogCleanup",
                "无法读取 Codex 配置以清理模型目录",
            ));
        }
    };
    let document = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| {
            CommandError::keyed(
                "codex-catalog-pointer-invalid",
                "errors.sw.codexConfigUnparseable",
                "Codex 配置无法解析",
            )
        })?;
    let Some(pointer) = document
        .get("model_catalog_json")
        .and_then(|item| item.as_str())
    else {
        return Ok(None);
    };
    let pointer_path = Path::new(pointer);
    let valid_name = pointer_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| name.starts_with("agent-switchboard-codex-") && name.ends_with(".json"));
    if valid_name.is_none() || pointer_path.components().count() != 1 {
        return Ok(None);
    }
    let directory = config_target
        .parent()
        .ok_or_else(|| {
            CommandError::keyed(
                "codex-catalog-path-invalid",
                "errors.sw.codexConfigDirInvalid",
                "Codex 配置目录无效",
            )
        })?;
    let path = directory.join(pointer);
    let before = match fs::read_to_string(&path) {
        Ok(content) => Some(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => {
            return Err(CommandError::keyed(
                "codex-catalog-unreadable",
                "errors.sw.ownedCodexCatalogUnreadable",
                "无法读取 ASB 管理的 Codex 模型目录",
            ));
        }
    };
    Ok(Some(super::transaction::CatalogArtifact::new(
        path, before, None,
    )))
}

pub(super) fn catalog_artifact(
    config_target: &Path,
    catalog: Option<&crate::gateway::CodexCatalogProjection>,
) -> Result<Option<super::transaction::CatalogArtifact>, CommandError> {
    let Some(catalog) = catalog else {
        return Ok(None);
    };
    let directory = config_target
        .parent()
        .ok_or_else(|| {
            CommandError::keyed(
                "codex-catalog-path-invalid",
                "errors.sw.codexConfigDirInvalid",
                "Codex 配置目录无效",
            )
        })?;
    let path = directory.join(&catalog.file_name);
    let before = match fs::read_to_string(&path) {
        Ok(content) => Some(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => {
            return Err(CommandError::keyed(
                "codex-catalog-unreadable",
                "errors.sw.codexCatalogFileUnreadable",
                "无法读取 Codex 模型目录文件",
            ));
        }
    };
    if before
        .as_deref()
        .is_some_and(|content| content != catalog.content)
    {
        return Err(CommandError::keyed(
            "codex-catalog-revision-conflict",
            "errors.sw.codexCatalogRevisionConflict",
            "同一 Codex 路由修订的模型目录内容不一致，拒绝覆盖",
        ));
    }
    Ok(Some(super::transaction::CatalogArtifact::new(
        path,
        before,
        Some(catalog.content.clone()),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use asb_core::contracts::{
        CodexCapabilities, CodexCatalogEntry, CodexEndpoint, CodexModelRoute, CodexProviderDraft,
        CodexUpstream,
    };

    fn codex_file() -> asb_core::contracts::CodexProviderFile {
        CodexProviderDraft {
            name: "relay".to_string(),
            endpoint: CodexEndpoint("https://relay.example/v1".to_string()),
            api_key: "fixture-key".to_string(),
            authentication: None,
            connection: Default::default(),
            upstream: CodexUpstream::Responses,
            request_mode: asb_core::contracts::ResponsesRequestMode::Standard,
            default_model: "fixture-model".to_string(),
            catalog: vec![CodexCatalogEntry {
                id: "fixture-model".to_string(),
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
                client_model: "fixture-model".to_string(),
                upstream_model: "vendor-model".to_string(),
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
        .into_file("00000000-0000-0000-0000-000000000001".to_string(), 100)
    }

    #[test]
    fn third_party_preflight_does_not_require_official_login_or_write_auth() {
        let _paths = crate::test_client_paths::redirect_client_paths();
        let directory = tempfile::tempdir().unwrap();
        let state = crate::local_state::LocalState::from_root(directory.path().join("state"));
        let gateway = crate::gateway::GatewayController::start(&state);
        let target = state.target(AppKind::Codex).unwrap();
        let mut file = codex_file();
        file.profile.upstream = CodexUpstream::ChatCompletions;
        file.profile.route_mode = asb_core::contracts::CodexRouteMode::Gateway;
        let projection = build_codex_plan(&state, &gateway, file.clone()).unwrap();
        assert!(projection.plan.is_gateway());
        assert!(!target.exists());
        let auth = target.parent().unwrap().join("auth.json");
        for value in [
            "broken",
            r#"{"auth_mode":"apikey","tokens":{"access_token":"residue"}}"#,
        ] {
            std::fs::write(&auth, value).unwrap();
            let projection = build_codex_plan(&state, &gateway, file.clone()).unwrap();
            assert!(projection.plan.is_gateway());
            assert_eq!(std::fs::read_to_string(&auth).unwrap(), value);
            assert!(!target.exists());
        }
        let preview = preview_projection(&state, &projection).unwrap();
        assert!(!preview.content.contains("fixture-key"));
        assert!(!target.exists());
        assert_eq!(
            std::fs::read_to_string(&auth).unwrap(),
            r#"{"auth_mode":"apikey","tokens":{"access_token":"residue"}}"#
        );
        gateway.shutdown();
    }

    #[test]
    fn official_cleanup_only_targets_the_asb_catalog_pointer() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("config.toml");
        let owned = directory.path().join("agent-switchboard-codex-a.json");
        std::fs::write(&owned, "catalog").unwrap();
        std::fs::write(
            &target,
            "model_catalog_json = \"agent-switchboard-codex-a.json\"\n",
        )
        .unwrap();
        let artifact = official_catalog_artifact(&target).unwrap().unwrap();
        crate::commands::switching::transaction::apply_catalog_artifact(&artifact).unwrap();
        assert!(!owned.exists());

        let user_catalog = directory.path().join("my-models.json");
        std::fs::write(&user_catalog, "user catalog").unwrap();
        std::fs::write(&target, "model_catalog_json = \"my-models.json\"\n").unwrap();
        assert!(official_catalog_artifact(&target).unwrap().is_none());
        assert_eq!(
            std::fs::read_to_string(&user_catalog).unwrap(),
            "user catalog"
        );
    }
}
