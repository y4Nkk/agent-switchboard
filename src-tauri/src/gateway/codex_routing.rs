//! Codex catalog projections, separate from Claude provider routing.

use super::*;

impl GatewayController {
    /// Resolves the stored common-file fragment for one profile. Disabled
    /// profiles still carry the text so the projection can strip previously
    /// merged keys; an empty fragment resolves to `None`.
    fn resolve_common_fragment(
        &self,
        profile_id: &str,
    ) -> Result<Option<asb_core::contracts::CodexCommonFragment>, String> {
        crate::codex_common::resolve_fragment(&self.inner.state_root, profile_id)
            .map_err(|error| format!("Codex 通用配置片段无效：{error}"))
    }

    /// Projects a current-format Codex profile through the local capability
    /// gateway. Official Codex has no profile and therefore never enters this
    /// path.
    pub(crate) fn project_codex(
        &self,
        file: &asb_core::contracts::CodexProviderFile,
        client_settings: asb_core::contracts::SettingsValues,
        replacement: Option<&asb_core::contracts::CodexProviderFile>,
    ) -> Result<GatewayProjection, String> {
        let (policy, _) = codex::policy::load(&self.inner.state_root)?;
        self.project_codex_with_policy(file, client_settings, &policy, replacement)
    }

    pub(crate) fn project_codex_with_policy(
        &self,
        file: &asb_core::contracts::CodexProviderFile,
        client_settings: asb_core::contracts::SettingsValues,
        policy: &codex::policy::CodexGatewayPolicy,
        replacement: Option<&asb_core::contracts::CodexProviderFile>,
    ) -> Result<GatewayProjection, String> {
        policy.validate()?;
        let fragment = self.resolve_common_fragment(&file.profile.id)?;
        let dependency_revision = self.codex_dependency_revision(file, replacement)?;
        let preserve = crate::codex_auth::policy::load(&self.inner.state_root)?
            .policy
            .preserve_official_login;
        file.validate()?;
        let profile = file.client_projection().into_profile(AppKind::Codex);
        let catalog = CodexCatalogProjection::from_file(
            &self.inner.state_root,
            file,
            &codex_route_fingerprint(file)?,
            replacement,
        )?;
        if file.profile.route_mode == asb_core::contracts::CodexRouteMode::Direct
            && !policy.takeover
        {
            let plan = attach_fragment(
                SwitchPlan::direct(profile, client_settings)
                    .with_codex_model_catalog(catalog.file_name.clone())
                    .with_codex_preserve_official_login(preserve),
                fragment,
            );
            asb_core::validate_plan(&plan.profile, &plan.client_settings)
                .map_err(|error| error.to_string())?;
            return Ok(GatewayProjection {
                plan,
                activation: GatewayActivation::Direct { app: AppKind::Codex },
                warning: Some(
                    "Codex Responses 将直连供应商；API 密钥仅写入 Codex auth.json，不写入 config.toml，也不会启动本机网关路由"
                        .to_string(),
                ),
                codex_catalog: Some(catalog),
                candidate_routes: Vec::new(),
            dependency_revision: Some(dependency_revision),
            });
        }
        if self.blocked_recovery().is_some() {
            return Err("存在未完成的端口修改恢复，无法切换 Codex 供应商".to_string());
        }
        let base_url = self
            .listening_base_url()
            .ok_or_else(|| "本机协议网关当前未在监听，无法写入 Codex 第三方配置".to_string())?;
        let route = self.route_for_codex_file(file, replacement)?;
        let plan = attach_fragment(
            SwitchPlan::through_gateway(
                profile,
                client_settings,
                route.client_endpoint(&base_url),
                route.client_token.clone(),
            )
            .with_codex_model_catalog(catalog.file_name.clone()),
            fragment,
        );
        asb_core::validate_plan(&plan.profile, &plan.client_settings)
            .map_err(|error| error.to_string())?;
        let warning = if file.profile.request_mode
            == asb_core::contracts::ResponsesRequestMode::Minimal
        {
            "Codex 第三方模型请求将经过本机协议网关；最小模式省略 reasoning、service_tier、store、include、metadata 与客户端缓存扩展字段，非空 previous_response_id 必须改为完整 input".to_string()
        } else {
            "Codex 第三方模型请求将经过本机协议网关".to_string()
        };
        Ok(GatewayProjection {
            plan,
            candidate_routes: self.codex_candidate_routes(file, policy, replacement)?,
            dependency_revision: Some(dependency_revision),
            activation: GatewayActivation::Routed(route),
            warning: Some(warning),
            codex_catalog: Some(catalog),
        })
    }

    /// Projects an official Codex switch through the local gateway while the
    /// policy enables takeover and a managed account is bound. The client
    /// keeps the built-in `openai` provider with the gateway entry in
    /// `openai_base_url`; the route resolves fresh managed tokens per request
    /// and never joins the third-party failover queue. A native CLI login
    /// without a binding stays direct: the gateway must never rewrite
    /// `auth.json` outside the switch executor.
    pub(crate) fn project_codex_official_takeover(
        &self,
        plan: SwitchPlan,
        policy: &codex::policy::CodexGatewayPolicy,
    ) -> Result<GatewayProjection, String> {
        policy.validate()?;
        if !policy.takeover {
            return Err("Codex 官方接管需要先在网关策略中启用接管".to_string());
        }
        let auth = plan
            .codex_managed_auth()
            .cloned()
            .ok_or_else(|| "Codex 官方接管需要绑定一个托管账号；原生登录请保持直连".to_string())?;
        if self.blocked_recovery().is_some() {
            return Err("存在未完成的端口修改恢复，无法切换 Codex 供应商".to_string());
        }
        let base_url = self
            .listening_base_url()
            .ok_or_else(|| "本机协议网关当前未在监听，无法接管 Codex 官方登录".to_string())?;
        let identity = self
            .inner
            .state
            .lock()
            .map_err(|_| "本机协议网关状态锁不可用".to_string())?
            .identity
            .clone();
        let fingerprint = hex_digest(
            serde_json::json!({
                "codexOfficial": true,
                "profileId": plan.profile.id,
                "managedId": auth.managed_id,
                "accountId": auth.account_id,
            })
            .to_string()
            .as_bytes(),
        );
        let snapshot = asb_core::contracts::CodexRouteSnapshot::official(
            &plan.profile.id,
            crate::codex_auth::OFFICIAL_CODEX_BASE,
            fingerprint.clone(),
        )?;
        let route = ActiveRoute {
            app: AppKind::Codex,
            profile_id: plan.profile.id.clone(),
            client_token: route_token(&identity, AppKind::Codex, &plan.profile.id, &fingerprint),
            continuation_key: codex_continuation_key(&identity, &plan.profile.id, &fingerprint),
            fingerprint,
            upstream_base_url: crate::codex_auth::OFFICIAL_CODEX_BASE.to_string(),
            connection: Default::default(),
            upstream_protocol: UpstreamProtocol::Responses,
            responses_options: plan.profile.responses_options.clone(),
            max_output_tokens: None,
            api_key: String::new(),
            authentication: asb_core::AuthenticationScheme::Bearer,
            claude_fragment: Default::default(),
            claude_primary_model: None,
            claude_model_options: None,
            claude_account: None,
            codex_account: Some(auth),
            codex: Some(snapshot),
        };
        let projected = official_takeover_plan(plan, &route, &base_url)?;
        Ok(GatewayProjection {
            plan: projected,
            activation: GatewayActivation::Routed(route),
            warning: Some(
                "官方 ChatGPT 账号将经过本机协议网关（仅限绑定的托管账号）；本地登录与绑定账号身份不一致的请求将被拒绝；官方账号不参与自动故障转移"
                    .to_string(),
            ),
            codex_catalog: None,
            candidate_routes: Vec::new(),
            dependency_revision: None,
        })
    }
}

fn official_takeover_plan(
    plan: SwitchPlan,
    route: &ActiveRoute,
    base_url: &str,
) -> Result<SwitchPlan, String> {
    let projected = attach_fragment(
        SwitchPlan::through_gateway(
            plan.profile.clone(),
            plan.client_settings.clone(),
            route.client_endpoint(base_url),
            route.client_token.clone(),
        )
        .with_codex_managed_auth(
            route
                .codex_account
                .clone()
                .expect("official takeover carries its bound account"),
        ),
        plan.codex_common_fragment().cloned(),
    );
    asb_core::validate_plan(&projected.profile, &projected.client_settings)
        .map_err(|error| error.to_string())?;
    Ok(projected)
}

/// Attaches the resolved common-file fragment to a Codex plan. `None` (no
/// stored fragment) leaves the plan untouched; the fragment itself carries
/// its per-profile enable decision.
fn attach_fragment(
    plan: SwitchPlan,
    fragment: Option<asb_core::contracts::CodexCommonFragment>,
) -> SwitchPlan {
    match fragment {
        Some(fragment) => plan.with_codex_common_fragment(fragment),
        None => plan,
    }
}

#[cfg(test)]
mod subagent_catalog_tests {
    use crate::gateway::CodexCatalogProjection;

    fn draft_with_route(
        route: Option<asb_core::contracts::CodexSubagentRoute>,
    ) -> asb_core::contracts::CodexProviderDraft {
        let mut draft = crate::codex_common::test_draft();
        draft.subagent_route = route;
        draft
    }

    #[test]
    fn a_subagent_route_merges_its_wire_entry_into_route_and_catalog() {
        let _paths = crate::test_client_paths::redirect_client_paths();
        let directory = tempfile::tempdir().unwrap();
        let state = crate::local_state::LocalState::from_root(directory.path().join("state"));
        let gateway = crate::gateway::GatewayController::start(&state);
        let target = state
            .configuration()
            .create_codex_provider(uuid::Uuid::new_v4().to_string(), draft_with_route(None))
            .unwrap();
        let routed = state
            .configuration()
            .create_codex_provider(uuid::Uuid::new_v4().to_string(), draft_with_route(Some(
                asb_core::contracts::CodexSubagentRoute {
                    profile_id: target.profile.id.clone(),
                    model: "relay-model".to_string(),
                },
            )))
            .unwrap();

        let file = state
            .configuration()
            .find_codex_provider_file(&routed.profile.id)
            .unwrap();
        let route = gateway.route_for_codex_file(&file, None).unwrap();
        let wire_id = format!("asb:{}/relay-model", target.profile.id);
        let snapshot = route.codex.as_ref().unwrap();
        assert!(snapshot.catalog.iter().any(|entry| entry.id == wire_id));
        let merged = snapshot
            .catalog
            .iter()
            .find(|entry| entry.id == wire_id)
            .unwrap();
        assert!(merged.display_name.as_deref().is_some_and(|name| name.contains("Relay")));

        let revision = crate::gateway::codex_route_fingerprint(&file).unwrap();
        let catalog =
            CodexCatalogProjection::from_file(&state.root(), &file, &revision, None).unwrap();
        assert!(catalog.content.contains(&wire_id));
        gateway.shutdown();
    }
}
