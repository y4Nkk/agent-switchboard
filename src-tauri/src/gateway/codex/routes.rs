use super::policy::CodexGatewayPolicy;
use crate::gateway::{ActiveRoute, GatewayController, GatewayInner};
use asb_core::contracts::{AppKind, CodexProviderFile};

impl GatewayInner {
    /// Resolves the target route of a subagent wire model at request time.
    /// The store is read live so an edited or deleted target profile fails
    /// loudly instead of forwarding with stale credentials. Endpoint
    /// variants of the target replace the whole candidate list: a subagent
    /// route never falls over to another provider.
    pub(crate) fn subagent_route_candidates(
        &self,
        wire: &asb_core::contracts::CodexSubagentRoute,
    ) -> Result<(ActiveRoute, Vec<ActiveRoute>), String> {
        let store = crate::config_store::ConfigStore::new(self.state_root.clone());
        let file = store
            .find_codex_provider_file(&wire.profile_id)
            .map_err(|_| {
                format!(
                    "子代理路由引用的 Codex 供应商不存在：{}（模型 {}）",
                    wire.profile_id, wire.model
                )
            })?;
        crate::gateway::validate_subagent_target(wire, &file)?;
        let identity = self
            .state
            .lock()
            .map_err(|_| "本机协议网关状态锁不可用".to_string())?
            .identity
            .clone();
        let route =
            crate::gateway::codex_profile::codex_route_from_file(&self.state_root, &file, &identity, None)?;
        let variants = route
            .connection
            .endpoint_candidates(&route.upstream_base_url)
            .into_iter()
            .map(|endpoint| {
                let mut variant = route.clone();
                variant.upstream_base_url = endpoint;
                variant
            })
            .collect();
        Ok((route, variants))
    }

    /// Records a successful Codex custom endpoint and moves it to the front
    /// of its provider's candidates. Metadata only: the client configuration
    /// and the route identity are untouched, and the shared endpoint lock
    /// keeps the write from racing a confirmed provider save.
    pub(crate) fn record_codex_endpoint_success(&self, route: &ActiveRoute) {
        if route.app != AppKind::Codex {
            return;
        }
        let Ok(_guard) = self.endpoint_write_lock.lock() else {
            log::warn!("Codex 供应商端点使用记录锁不可用");
            return;
        };
        let store = crate::config_store::ConfigStore::new(self.state_root.clone());
        match store.mark_codex_endpoint_used(&route.profile_id, &route.upstream_base_url) {
            Ok(true) => self.prioritize_endpoint(route),
            Ok(false) => {}
            Err(error) => log::warn!("无法记录 Codex 供应商端点使用时间: {error}"),
        }
    }
}

impl GatewayController {
    pub(crate) fn codex_candidate_routes(
        &self,
        file: &CodexProviderFile,
        policy: &CodexGatewayPolicy,
        replacement: Option<&CodexProviderFile>,
    ) -> Result<Vec<ActiveRoute>, String> {
        policy.validate()?;
        let store = crate::config_store::ConfigStore::new(self.inner.state_root.clone());
        let mut routes = Vec::new();
        for id in policy.candidate_ids(&file.profile.id) {
            let candidate = if id == file.profile.id {
                file.clone()
            } else if let Some(candidate) = replacement.filter(|file| file.profile.id == id) {
                candidate.clone()
            } else {
                store
                    .find_codex_provider_file(&id)
                    .map_err(|_| format!("Codex 队列供应商不存在：{id}"))?
            };
            if policy.enabled && candidate.profile.connection.auth_binding.is_some() {
                return Err("托管账号不能参与 Codex 自动故障转移".into());
            }
            let route = self.route_for_codex_file(&candidate, replacement)?;
            for endpoint in route
                .connection
                .endpoint_candidates(&route.upstream_base_url)
            {
                let mut variant = route.clone();
                variant.upstream_base_url = endpoint;
                routes.push(variant);
            }
        }
        Ok(routes)
    }

    pub(crate) fn rehydrate_codex_candidates(
        &self,
        profile_id: &str,
    ) -> Result<Vec<ActiveRoute>, String> {
        let store = crate::config_store::ConfigStore::new(self.inner.state_root.clone());
        let file = store
            .find_codex_provider_file(profile_id)
            .map_err(|error| error.to_string())?;
        let (policy, _) = super::policy::load(&self.inner.state_root)?;
        self.codex_candidate_routes(&file, &policy, None)
    }

    pub(crate) fn reset_codex_provider_health(&self, profile_id: &str) -> Result<(), String> {
        let store = crate::config_store::ConfigStore::new(self.inner.state_root.clone());
        let file = store
            .find_codex_provider_file(profile_id)
            .map_err(|error| error.to_string())?;
        let route = self.route_for_codex_file(&file, None)?;
        for endpoint in route
            .connection
            .endpoint_candidates(&route.upstream_base_url)
        {
            let mut variant = route.clone();
            variant.upstream_base_url = endpoint;
            self.inner.codex_health.reset(&variant)?;
        }
        Ok(())
    }

    pub(crate) fn codex_health_warning(&self) -> Option<String> {
        self.inner.codex_health.warning()
    }
}

impl crate::gateway::GatewayProjection {
    pub(crate) fn activation_snapshot(&self) -> crate::gateway::GatewayActivationSnapshot {
        use crate::gateway::GatewayActivation;
        let route = match &self.activation {
            GatewayActivation::Direct { .. } => None,
            GatewayActivation::Routed(route) => Some(crate::gateway::state::PersistedRoute {
                profile_id: route.profile_id.clone(),
                revision: route.fingerprint.clone(),
            }),
        };
        crate::gateway::GatewayActivationSnapshot {
            app: self.plan.app(),
            route,
        }
    }
}

impl GatewayController {
    pub(crate) fn clear_codex_activation(&self) -> Result<(), String> {
        self.commit_activation(
            &crate::gateway::GatewayActivation::Direct {
                app: AppKind::Codex,
            },
            &[],
            || Ok(()),
        )
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexEndpointHealth {
    endpoint: String,
    circuit_state: crate::gateway::ProviderHealthState,
    consecutive_failures: u32,
}
impl GatewayController {
    pub(crate) fn codex_endpoint_health(
        &self,
        file: &CodexProviderFile,
        policy: &CodexGatewayPolicy,
    ) -> Result<Vec<CodexEndpointHealth>, String> {
        let route = self.route_for_codex_file(file, None)?;
        Ok(route
            .connection
            .endpoint_candidates(&route.upstream_base_url)
            .into_iter()
            .map(|endpoint| {
                let mut candidate = route.clone();
                candidate.upstream_base_url = endpoint.clone();
                let health = self
                    .inner
                    .codex_health
                    .health_for(&candidate, policy.traffic.health_config());
                CodexEndpointHealth {
                    endpoint,
                    circuit_state: health.state(),
                    consecutive_failures: health.consecutive_failures(),
                }
            })
            .collect())
    }
}
