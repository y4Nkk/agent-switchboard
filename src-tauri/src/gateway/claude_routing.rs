//! Claude projection and request candidate ownership, separate from Codex.

use super::controller::projection_warning;
use super::*;

impl GatewayController {
    /// Projects a Claude route and captures the current custom-provider order
    /// for request-level failover. The client projection is identical to
    /// `project`; the extra snapshot never crosses the client-file boundary.
    pub(crate) fn project_with_candidates(
        &self,
        local: &LocalState,
        plan: &SwitchPlan,
        replacement: Option<&ProviderProfile>,
    ) -> Result<GatewayProjection, String> {
        let force_claude_gateway =
            plan.app() == AppKind::Claude && plan.profile.route_mode == RouteMode::Custom && {
                let policy = failover::load(local.root())?;
                policy.enabled || policy.takeover
            };
        if force_claude_gateway && plan.profile.connection.claude_native.is_some() {
            return Err("Claude 原生云 SDK 不能接管到普通 Anthropic HTTP 网关；请关闭 Claude 接管和 Failover 后重试".into());
        }
        let mut projection = if force_claude_gateway {
            self.project_claude_for_failover(plan)?
        } else {
            self.project(plan)?
        };
        if plan.app() == AppKind::Claude
            && matches!(&projection.activation, GatewayActivation::Routed(_))
        {
            let route = match &projection.activation {
                GatewayActivation::Routed(route) => route,
                GatewayActivation::Direct { .. } => unreachable!(),
            };
            projection.candidate_routes = self.claude_candidate_routes(
                local, &route.profile_id, replacement.or(Some(&plan.profile)),
            )?;
        }
        Ok(projection)
    }

    fn project_claude_for_failover(&self, plan: &SwitchPlan) -> Result<GatewayProjection, String> {
        validate_plan(&plan.profile, &plan.client_settings).map_err(|error| error.to_string())?;
        if self.blocked_recovery().is_some() {
            return Err(
                "存在未完成的端口修改恢复，已暂停经本机协议网关的切换；请先在网关页处理恢复状态"
                    .to_string(),
            );
        }
        let base_url = self
            .listening_base_url()
            .ok_or_else(|| "本机协议网关当前未在监听，无法启用 Claude 故障转移".to_string())?;
        let route = self.route_for_profile(&plan.profile)?;
        let projected = SwitchPlan::through_gateway(
            plan.profile.clone(),
            plan.client_settings.clone(),
            route.client_endpoint(&base_url),
            route.client_token.clone(),
        )
        .with_claude_route_revision(route.claude_revision());
        let warning = format!(
            "Claude 本机接管已启用：请求将经过本机协议网关，故障转移仅在策略明确启用后尝试队列。{}",
            projection_warning(plan, &projected)
        );
        Ok(GatewayProjection {
            plan: projected,
            activation: GatewayActivation::Routed(route),
            warning: Some(warning),
            codex_catalog: None,
            candidate_routes: Vec::new(),
        })
    }

    pub(crate) fn claude_candidate_routes(
        &self,
        local: &LocalState,
        selected_profile_id: &str,
        replacement: Option<&ProviderProfile>,
    ) -> Result<Vec<ActiveRoute>, String> {
        let policy = failover::load(local.root())?;
        let profiles = local
            .configuration()
            .list_providers()
            .map_err(|_| "无法读取 Claude 供应商配置，已取消本次切换".to_string())?
            .into_iter()
            .filter_map(|record| {
                let profile = replacement.filter(|profile| profile.id == record.profile.id)
                    .cloned().unwrap_or(record.profile);
                (profile.app == AppKind::Claude
                    && profile.route_mode == RouteMode::Custom
                    && profile.connection.claude_native.is_none()
                    && profile.base_url.is_some()
                    && profile.upstream_protocol.is_some())
                .then_some(profile)
            })
            .collect::<Vec<_>>();
        let selected = profiles
            .iter()
            .position(|profile| profile.id == selected_profile_id)
            .ok_or_else(|| "当前 Claude 供应商不在可切换候选列表中".to_string())?;
        let selected_id = profiles[selected].id.clone();
        let available_ids = profiles
            .iter()
            .map(|profile| profile.id.clone())
            .collect::<Vec<_>>();
        let ordered_ids = policy.candidate_provider_ids(&selected_id, &available_ids);
        let routes = ordered_ids
            .into_iter()
            .filter_map(|id| profiles.iter().find(|profile| profile.id == id))
            .map(|profile| self.route_for_profile(profile))
            .collect::<Result<Vec<_>, _>>()
            .map(|routes| {
                routes
                    .into_iter()
                    .flat_map(endpoint_variants)
                    .collect::<Vec<_>>()
            })?;
        Ok(routes)
    }
}

fn endpoint_variants(route: ActiveRoute) -> Vec<ActiveRoute> {
    route
        .connection
        .endpoint_candidates(&route.upstream_base_url)
        .into_iter()
        .map(|endpoint| {
            let mut candidate = route.clone();
            candidate.upstream_base_url = endpoint;
            candidate
        })
        .collect()
}

impl ActiveRoute {
    pub(super) fn claude_revision(&self) -> String {
        format!("{}:{}", self.profile_id, self.fingerprint)
    }
}

pub(crate) fn claude_uses_gateway(configuration: &str) -> bool {
    routing::config_points_at_gateway(AppKind::Claude, configuration)
}
