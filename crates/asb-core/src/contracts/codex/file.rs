use super::{
    CodexProviderDraft, CodexProviderFile, CodexProviderProfile, CodexProviderRecord,
    CodexRouteMode, CodexUpstream, CODEX_PROVIDER_SCHEMA_VERSION,
};
use crate::contracts::{
    CodexModelSettings, ExplicitMaxOutputTokens, ModelOptions, ProviderFile, ResponsesOptions,
    RouteMode,
};

impl CodexProviderDraft {
    pub fn into_file(self, id: String, position: u64) -> CodexProviderFile {
        CodexProviderFile {
            schema_version: CODEX_PROVIDER_SCHEMA_VERSION,
            position,
            profile: CodexProviderProfile {
                id,
                name: self.name,
                route_mode: CodexRouteMode::for_profile(
                    self.upstream,
                    self.request_mode,
                    &self.connection,
                    self.authentication,
                    Some(self.endpoint.0.as_str()),
                    self.subagent_route.as_ref(),
                    !self.model_routes.is_empty(),
                ),
                endpoint: self.endpoint,
                api_key: self.api_key,
                authentication: self.authentication,
                connection: self.connection,
                upstream: self.upstream,
                request_mode: self.request_mode,
                default_model: self.default_model,
                catalog: self.catalog,
                model_routes: self.model_routes,
                subagent_route: self.subagent_route,
                capabilities: self.capabilities,
            },
            parameters: self.parameters,
            notes: self.notes,
            website_url: self.website_url,
            usage_query: self.usage_query,
        }
    }
}

impl CodexProviderRecord {
    pub fn into_draft(self) -> CodexProviderDraft {
        CodexProviderDraft {
            name: self.profile.name,
            endpoint: self.profile.endpoint,
            api_key: self.profile.api_key,
            authentication: self.profile.authentication,
            connection: self.profile.connection,
            upstream: self.profile.upstream,
            request_mode: self.profile.request_mode,
            default_model: self.profile.default_model,
            catalog: self.profile.catalog,
            model_routes: self.profile.model_routes,
            subagent_route: self.profile.subagent_route,
            capabilities: self.profile.capabilities,
            parameters: self.parameters,
            notes: self.notes,
            website_url: self.website_url,
            usage_query: self.usage_query,
        }
    }
}

impl CodexProviderFile {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != CODEX_PROVIDER_SCHEMA_VERSION {
            return Err("Codex 供应商档案版本不受支持，请重新创建档案".to_string());
        }
        if self.position == 0 {
            return Err("Codex 供应商排序位置必须大于零".to_string());
        }
        self.profile.validate()?;
        self.parameters
            .validate_provider_parameters(crate::contracts::AppKind::Codex)
            .map_err(|error| error.to_string())?;
        if let Some(query) = &self.usage_query {
            crate::validate::validate_usage_query(query).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub fn record(&self, file_hash: String) -> CodexProviderRecord {
        CodexProviderRecord {
            profile: self.profile.clone(),
            position: self.position,
            parameters: self.parameters.clone(),
            notes: self.notes.clone(),
            website_url: self.website_url.clone(),
            usage_query: self.usage_query.clone(),
            file_hash,
        }
    }

    pub fn client_projection(&self) -> ProviderFile {
        let default = self
            .profile
            .catalog
            .iter()
            .find(|entry| entry.id == self.profile.default_model)
            .expect("validated Codex profile always contains its default model");
        ProviderFile {
            authentication: self.profile.authentication,
            id: self.profile.id.clone(),
            name: self.profile.name.clone(),
            position: self.position,
            route_mode: RouteMode::Custom,
            api_key: self.profile.api_key.clone(),
            upstream_protocol: Some(self.profile.upstream.protocol()),
            responses_options: (self.profile.upstream == CodexUpstream::Responses).then_some(
                ResponsesOptions {
                    request_mode: self.profile.request_mode,
                },
            ),
            max_output_tokens: ExplicitMaxOutputTokens::from(
                (self.profile.upstream == CodexUpstream::AnthropicMessages)
                    .then_some(default.max_output_tokens),
            ),
            base_url: Some(self.profile.endpoint.0.clone()),
            connection: self.profile.connection.clone(),
            model: Some(self.profile.default_model.clone()),
            model_options: Some(ModelOptions::Codex(CodexModelSettings {
                context_window: Some(default.context_window),
                subagent_route: self.profile.subagent_route.clone(),
            })),
            parameters: self.parameters.clone(),
            claude_fragment: Default::default(),
            notes: self.notes.clone(),
            website_url: self.website_url.clone(),
            display: None,
            usage_query: self.usage_query.clone(),
            official_quota_refresh_interval_minutes: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::contracts::{
        AppKind, CodexCatalogEntry, CodexEndpoint, CodexProviderDraft, ResponsesRequestMode,
        DEFAULT_CODEX_CAPABILITIES,
    };

    fn draft(request_mode: ResponsesRequestMode) -> CodexProviderDraft {
        CodexProviderDraft {
            name: "Relay".into(),
            endpoint: CodexEndpoint("https://relay.example/v1".into()),
            api_key: "secret".into(),
            authentication: None,
            connection: Default::default(),
            upstream: super::CodexUpstream::Responses,
            request_mode,
            default_model: "relay-model".into(),
            catalog: vec![CodexCatalogEntry::default_entry("relay-model")],
            model_routes: Vec::new(),
            subagent_route: None,
            capabilities: DEFAULT_CODEX_CAPABILITIES,
            parameters: crate::ownership::default_provider_parameters(AppKind::Codex),
            notes: None,
            website_url: None,
            usage_query: None,
        }
    }

    #[test]
    fn minimal_responses_persist_as_a_gateway_route() {
        let file = draft(ResponsesRequestMode::Minimal).into_file("profile".into(), 1);
        assert_eq!(file.profile.route_mode, super::CodexRouteMode::Gateway);
        assert!(file.validate().is_ok());
    }

    #[test]
    fn minimal_responses_reject_a_stale_direct_route() {
        let mut file = draft(ResponsesRequestMode::Minimal).into_file("profile".into(), 1);
        file.profile.route_mode = super::CodexRouteMode::Direct;
        assert!(file.validate().is_err());
    }
    #[test]
    fn a_model_mapping_cannot_bypass_the_gateway() {
        let mut mapped = draft(ResponsesRequestMode::Standard);
        mapped.model_routes = vec![crate::contracts::CodexModelRoute {
            client_model: "relay-model".into(),
            upstream_model: "vendor-model".into(),
        }];
        let mut file = mapped.into_file("profile".into(), 1);
        assert_eq!(file.profile.route_mode, super::CodexRouteMode::Gateway);
        assert!(file.validate().is_ok());
        file.profile.route_mode = super::CodexRouteMode::Direct;
        assert!(file.validate().is_err());
    }
}
