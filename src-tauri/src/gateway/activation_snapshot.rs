use super::*;

/// Recovery intent references a profile revision without copying credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GatewayActivationSnapshot {
    pub(crate) app: AppKind,
    pub(crate) route: Option<PersistedRoute>,
}

impl GatewayProjection {
    pub(crate) fn candidate_revision(&self) -> String {
        let revisions = self.candidate_routes.iter().map(|route| {
            (&route.profile_id, &route.fingerprint, &route.upstream_base_url)
        }).collect::<Vec<_>>();
        asb_switch::sha256_hex(
            &serde_json::to_string(&revisions).expect("route identity serializes"),
        )
    }
}

impl GatewayController {
    pub(crate) fn snapshot_activation(
        &self,
        app: AppKind,
    ) -> Result<GatewayActivationSnapshot, String> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| "网关状态锁不可用".to_string())?;
        Ok(GatewayActivationSnapshot {
            app,
            route: state.route(app).cloned(),
        })
    }

    /// Hash only the effective Codex routes under the activation lock. Client
    /// files alone do not change when the stable loopback route switches.
    pub(crate) fn probe_route_revision(&self, configuration: &str) -> Result<String, String> {
        if !routing::config_points_at_gateway(AppKind::Codex, configuration) { return Ok(String::new()); }
        let _activation = self.inner.activation_lock.lock().map_err(|_| "网关激活锁不可用")?;
        let routes = self.inner.routes.read().map_err(|_| "网关路由锁不可用")?;
        let candidates = self.inner.candidate_routes.read().map_err(|_| "网关候选路由锁不可用")?;
        let active = routes.get(&AppKind::Codex).map(|route| (&route.profile_id, &route.fingerprint));
        let mut revisions = candidates.get(&AppKind::Codex).into_iter().flatten()
            .map(|route| (&route.profile_id, &route.fingerprint)).collect::<Vec<_>>();
        revisions.sort();
        let data = serde_json::to_string(&(active, revisions, self.inner.codex_activation_generation.load(Ordering::SeqCst))).map_err(|error| error.to_string())?;
        Ok(asb_switch::sha256_hex(&data))
    }

    pub(crate) fn restore_activation_snapshot(
        &self,
        local: &LocalState,
        snapshot: &GatewayActivationSnapshot,
    ) -> Result<(), String> {
        let mut candidate_routes = Vec::new();
        let activation = match &snapshot.route {
            None => GatewayActivation::Direct { app: snapshot.app },
            Some(saved) => {
                let route = match snapshot.app {
                    AppKind::Codex => {
                        let file = local
                            .configuration()
                            .find_codex_provider_file(&saved.profile_id)
                            .map_err(|_| "恢复所需的 Codex 供应商档案已不存在".to_string())?;
                        if codex_route_fingerprint(&file)? != saved.revision {
                            return Err("恢复所需的 Codex 供应商连接已改变，保留恢复记录等待处理"
                                .to_string());
                        }
                        let (policy, _) = codex::policy::load(local.root())?;
                        candidate_routes = self.codex_candidate_routes(&file, &policy, None)?;
                        self.route_for_codex_file(&file, None)?
                    }
                    AppKind::Claude => {
                        let profile = local
                            .configuration()
                            .find_provider(&saved.profile_id)
                            .map_err(|_| "恢复所需的供应商档案已不存在".to_string())?;
                        if profile.app != AppKind::Claude
                            || route_fingerprint(&profile)? != saved.revision
                        {
                            return Err(
                                "恢复所需的供应商连接已改变，保留恢复记录等待处理".to_string()
                            );
                        }
                        let route = self.route_for_profile(&profile)?;
                        candidate_routes = self
                            .claude_candidate_routes(local, &route.profile_id, None)
                            .unwrap_or_else(|_| vec![route.clone()]);
                        route
                    }
                };
                GatewayActivation::Routed(route)
            }
        };
        self.commit_activation(&activation, &candidate_routes, || Ok(()))
    }
}
