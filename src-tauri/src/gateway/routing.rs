//! Route resolution, activation commit, and restore reconciliation.

use super::*;

impl GatewayController {
    pub(crate) fn active_profile_id_for(&self, app: AppKind) -> Option<String> {
        self.inner
            .routes
            .read()
            .ok()
            .and_then(|routes| routes.get(&app).map(|route| route.profile_id.clone()))
    }

    /// Rebuilds the active Claude candidate snapshot after a policy write.
    /// The client-facing route and configuration are left untouched.
    pub(crate) fn refresh_claude_candidates(&self, local: &LocalState) -> Result<(), String> {
        let _activation = self
            .inner
            .activation_lock
            .lock()
            .map_err(|_| "本机协议网关切换锁不可用".to_string())?;
        let active = self
            .inner
            .routes
            .read()
            .map_err(|_| "本机协议网关路由锁不可用".to_string())?
            .get(&AppKind::Claude)
            .cloned();
        let Some(active) = active else {
            self.inner
                .candidate_routes
                .write()
                .map_err(|_| "本机协议网关候选路由锁不可用".to_string())?
                .remove(&AppKind::Claude);
            return Ok(());
        };
        let candidates = self.claude_candidate_routes(local, &active.profile_id, None)?;
        self.inner
            .candidate_routes
            .write()
            .map_err(|_| "本机协议网关候选路由锁不可用".to_string())?
            .insert(AppKind::Claude, candidates);
        Ok(())
    }

    pub(super) fn commit_activation<F>(
        &self,
        activation: &GatewayActivation,
        candidate_routes: &[ActiveRoute],
        persist: F,
    ) -> Result<(), String>
    where
        F: FnOnce() -> Result<(), String>,
    {
        let _activation = self
            .inner
            .activation_lock
            .lock()
            .map_err(|_| "本机协议网关切换锁不可用".to_string())?;
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| "本机协议网关状态锁不可用".to_string())?;
        let mut routes = self
            .inner
            .routes
            .write()
            .map_err(|_| "本机协议网关路由锁不可用".to_string())?;
        let mut candidates = self
            .inner
            .candidate_routes
            .write()
            .map_err(|_| "本机协议网关候选路由锁不可用".to_string())?;
        let app = activation.app();
        let mut next = state.clone();
        next.set_route(
            app,
            match activation {
                GatewayActivation::Direct { .. } => None,
                GatewayActivation::Routed(route) => Some(persisted_route(route)),
            },
        );
        write_state(&self.inner.state_path, &next)?;
        // Do not publish a provisional route while its write record can fail.
        // Accepted requests keep their prior immutable route snapshot.
        if let Err(error) = persist() {
            write_state(&self.inner.state_path, &state)
                .map_err(|rollback| format!("{error}；且无法恢复网关状态：{rollback}"))?;
            return Err(error);
        }
        *state = next;
        match activation {
            GatewayActivation::Direct { .. } => {
                routes.remove(&app);
                candidates.remove(&app);
            }
            GatewayActivation::Routed(route) => {
                routes.insert(app, route.clone());
                candidates.insert(
                    app,
                    if candidate_routes.is_empty() {
                        vec![route.clone()]
                    } else {
                        candidate_routes.to_vec()
                    },
                );
            }
        }
        if app == AppKind::Codex {
            self.inner.codex_activation_generation.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }

    pub(super) fn rehydrate(&self, local: &LocalState) {
        let Some(valid) = self.recoverable_routes(local) else {
            return;
        };
        if let Ok(mut state) = self.inner.state.lock() {
            let codex = valid.get(&AppKind::Codex).map(persisted_route);
            let claude = valid.get(&AppKind::Claude).map(persisted_route);
            if state.codex_route != codex || state.claude_route != claude {
                let mut next = state.clone();
                next.codex_route = codex;
                next.claude_route = claude;
                if write_state(&self.inner.state_path, &next).is_ok() {
                    *state = next;
                } else {
                    // Refuse to publish a route whose persistence repair
                    // failed. The local client configuration will never be
                    // silently rebound to an unrecorded provider.
                    return;
                }
            }
        } else {
            return;
        }
        if let Ok(mut routes) = self.inner.routes.write() {
            *routes = valid;
            let mut candidates = BTreeMap::new();
            for (app, route) in routes.iter() {
                let snapshot = if *app == AppKind::Claude {
                    self.claude_candidate_routes(local, &route.profile_id, None)
                        .unwrap_or_else(|_| vec![route.clone()])
                } else {
                    self.rehydrate_codex_candidates(&route.profile_id)
                        .unwrap_or_else(|error| {
                            log::warn!("无法恢复 Codex 故障转移队列：{error}");
                            vec![route.clone()]
                        })
                };
                candidates.insert(*app, snapshot);
            }
            if let Ok(mut stored) = self.inner.candidate_routes.write() {
                *stored = candidates;
            }
        }
    }

    fn recoverable_routes(&self, local: &LocalState) -> Option<BTreeMap<AppKind, ActiveRoute>> {
        let persisted = self.inner.state.lock().ok()?.clone();
        let mut valid = BTreeMap::new();
        for app in [AppKind::Codex, AppKind::Claude] {
            let Some(saved) = persisted.route(app).cloned() else {
                continue;
            };
            let route = match app {
                AppKind::Codex => local
                    .configuration()
                    .find_codex_provider_file(&saved.profile_id)
                    .map_err(|error| error.to_string())
                    .and_then(|file| {
                        (codex_route_fingerprint(&file)? == saved.revision)
                            .then_some(file)
                            .ok_or_else(|| "Codex 路由已改变".to_string())
                    })
                    .and_then(|file| self.route_for_codex_file(&file, None)),
                AppKind::Claude => local
                    .configuration()
                    .find_provider(&saved.profile_id)
                    .map_err(|error| error.to_string())
                    .and_then(|profile| {
                        (profile.app == app && route_fingerprint(&profile)? == saved.revision)
                            .then_some(profile)
                            .ok_or_else(|| "Claude 路由已改变".to_string())
                    })
                    .and_then(|profile| self.route_for_profile(&profile)),
            };
            let Ok(route) = route else {
                log::warn!("本机协议网关拒绝恢复已删除或已变更的供应商路由");
                continue;
            };
            let Ok(target) = local.target(app) else {
                log::warn!("本机协议网关无法读取客户端配置路径");
                continue;
            };
            let Ok(text) = fs::read_to_string(target) else {
                log::warn!("本机协议网关拒绝恢复未指向本机端口的供应商路由");
                continue;
            };
            if !self.route_matches_config(&route, &text).unwrap_or(false) {
                log::warn!("本机协议网关拒绝恢复未匹配当前客户端配置的供应商路由");
                continue;
            }
            valid.insert(app, route);
        }
        Some(valid)
    }

    /// A client that carries this gateway's token but does not exactly match
    /// the currently recoverable route needs an explicit re-apply. This is
    /// derived from current files rather than a boot-only flag, so the repair
    /// state survives an application restart.
    pub(super) fn has_unreconciled_gateway_files(&self, local: &LocalState) -> bool {
        for app in [AppKind::Codex, AppKind::Claude] {
            let Ok(target) = local.target(app) else {
                continue;
            };
            let Ok(configuration) = fs::read_to_string(target) else {
                continue;
            };
            if !config_points_at_gateway(app, &configuration) {
                continue;
            }
            let route = match self.inner.routes.read() {
                Ok(routes) => routes.get(&app).cloned(),
                Err(_) => return true,
            };
            if !route.is_some_and(|route| {
                self.route_matches_config(&route, &configuration)
                    .unwrap_or(false)
            }) {
                return true;
            }
        }
        false
    }

    pub(super) fn route_for_profile(
        &self,
        profile: &ProviderProfile,
    ) -> Result<ActiveRoute, String> {
        if profile.connection.claude_native.is_some() {
            return Err("Claude 原生云 SDK 不使用本机 HTTP 网关".into());
        }
        if profile.app == AppKind::Codex {
            return Err("Codex 必须从专用档案创建网关路由".to_string());
        }
        if profile.route_mode != RouteMode::Custom {
            return Err("官方登录不应创建本机协议网关路由".to_string());
        }
        let upstream_protocol = profile
            .upstream_protocol
            .ok_or_else(|| "供应商缺少上游 API 格式".to_string())?;
        let upstream_base_url = profile
            .base_url
            .clone()
            .ok_or_else(|| "供应商缺少服务地址".to_string())?;
        let fingerprint = route_fingerprint(profile)?;
        let identity = self
            .inner
            .state
            .lock()
            .map_err(|_| "本机协议网关状态锁不可用".to_string())?
            .identity
            .clone();
        Ok(ActiveRoute {
            app: profile.app,
            profile_id: profile.id.clone(),
            client_token: route_token(&identity, profile.app, &profile.id, &fingerprint),
            continuation_key: continuation_key(&identity, profile, &fingerprint),
            fingerprint,
            upstream_base_url,
            connection: profile.connection.clone(),
            upstream_protocol,
            responses_options: profile.responses_options.clone(),
            max_output_tokens: profile.max_output_tokens.value(),
            api_key: profile.api_key.clone(),
            authentication: profile
                .upstream_authentication()
                .expect("custom route authentication"),
            claude_fragment: profile.claude_fragment.clone(),
            claude_primary_model: (profile.app == AppKind::Claude)
                .then(|| profile.model.clone())
                .flatten(),
            claude_model_options: match &profile.model_options {
                Some(ModelOptions::Claude(settings)) if profile.app == AppKind::Claude => {
                    Some(settings.clone())
                }
                _ => None,
            },
            claude_account: None,
            codex_account: None,
            codex: None,
        })
    }

    pub(super) fn route_for_codex_file(
        &self,
        file: &asb_core::contracts::CodexProviderFile,
        replacement: Option<&asb_core::contracts::CodexProviderFile>,
    ) -> Result<ActiveRoute, String> {
        let identity = self
            .inner
            .state
            .lock()
            .map_err(|_| "本机协议网关状态锁不可用".to_string())?
            .identity
            .clone();
        super::codex_profile::codex_route_from_file(&self.inner.state_root, file, &identity, replacement)
    }
    pub(super) fn route_matches_config(
        &self,
        route: &ActiveRoute,
        configuration: &str,
    ) -> Result<bool, String> {
        // Official takeover keeps the built-in `openai` provider and the
        // gateway entry in `openai_base_url`; there is no catalog pointer,
        // so the client endpoint itself is the route revision marker.
        if route.app == AppKind::Codex && route.codex_account.is_some() {
            let document = configuration
                .parse::<toml_edit::DocumentMut>()
                .map_err(|_| "无法验证 Codex 路由修订".to_string())?;
            let expected = route.client_endpoint(&self.configured_base_url());
            return Ok(document
                .get(asb_core::ownership::CODEX_PROVIDER_BASE_URL_KEY)
                .and_then(|value| value.as_str())
                == Some(expected.as_str()));
        }
        let matches =
            adapter::matches_provider_identity(configuration, &self.client_identity_plan(route))
                .map_err(|_| "无法验证本机协议网关客户端凭据".to_string())?;
        if !matches {
            return Ok(false);
        }
        if route.app == AppKind::Claude {
            let value: serde_json::Value = serde_json::from_str(configuration)
                .map_err(|_| "无法验证 Claude 路由修订".to_string())?;
            return Ok(value
                .pointer("/env/ASB_CLAUDE_ROUTE_REVISION")
                .and_then(serde_json::Value::as_str)
                == Some(route.claude_revision().as_str()));
        }
        // The capability authorizes the installation, not a provider. The
        // catalog pointer is the persisted provider/revision binding.
        let document = configuration
            .parse::<toml_edit::DocumentMut>()
            .map_err(|_| "无法验证 Codex 路由修订".to_string())?;
        let catalog = &route.codex.as_ref().ok_or("Codex 路由缺少模型目录")?.catalog;
        let expected = codex_catalog_file_name(&route.profile_id, &route.fingerprint, catalog);
        Ok(document
            .get(asb_core::ownership::CODEX_MODEL_CATALOG_KEY)
            .and_then(|value| value.as_str())
            == Some(expected.as_str()))
    }

    pub(super) fn client_identity_plan(&self, route: &ActiveRoute) -> SwitchPlan {
        let base_url = self.configured_base_url();
        SwitchPlan::through_gateway(
            ProviderProfile {
                authentication: (route.app == AppKind::Claude).then_some(route.authentication),
                id: route.profile_id.clone(),
                parameters: asb_core::ownership::default_provider_parameters(route.app),
                app: route.app,
                route_mode: RouteMode::Custom,
                name: "本机协议网关".to_string(),
                model: None,
                claude_fragment: route.claude_fragment.clone(),
                base_url: Some(route.upstream_base_url.clone()),
                connection: route.connection.clone(),
                api_key: route.api_key.clone(),
                upstream_protocol: Some(route.upstream_protocol),
                responses_options: route.responses_options,
                max_output_tokens: None.into(),
                model_options: None,
                notes: None,
                website_url: None,
                display: None,
                usage_query: None,
                official_quota_refresh_interval_minutes: None,
            },
            asb_core::ownership::default_client_settings(route.app),
            route.client_endpoint(&base_url),
            route.client_token.clone(),
        )
        .with_claude_route_revision(route.claude_revision())
    }
}

fn persisted_route(route: &ActiveRoute) -> PersistedRoute {
    PersistedRoute {
        profile_id: route.profile_id.clone(),
        revision: route.fingerprint.clone(),
    }
}

/// The capability-token prefix this application writes into client
/// configurations. Combined with a loopback endpoint it identifies a client
/// that depends on this gateway without needing the listener to be up.
pub(super) const CLIENT_TOKEN_PREFIX: &str = "asb_local_";

/// Coarse, listener-independent ownership check: the client configuration
/// points at a loopback gateway endpoint carrying this application's
/// capability token. Used for repair detection, exit decisions, and restore
/// rejection — never to *authorize* a rewrite, which always requires the
/// exact identity match.
pub(super) fn config_points_at_gateway(app: AppKind, configuration: &str) -> bool {
    if adapter::validate_syntax(app, configuration).is_err() {
        return false;
    }
    let Some(base_url) = adapter::route_state(app, configuration).base_url else {
        return false;
    };
    if !base_url.starts_with("http://127.0.0.1:") {
        return false;
    }
    match app {
        AppKind::Codex => asb_core::adapter::codex::is_gateway_base_url(&base_url),
        AppKind::Claude => configuration.contains(CLIENT_TOKEN_PREFIX),
    }
}

/// Read-only routing fact extracted before renderer-visible URL redaction.
pub(crate) fn client_gateway_port(app: AppKind, configuration: &str) -> Option<u16> {
    if !config_points_at_gateway(app, configuration) { return None; }
    adapter::route_state(app, configuration).base_url
        .and_then(|base_url| reqwest::Url::parse(&base_url).ok())
        .and_then(|url| url.port())
}
