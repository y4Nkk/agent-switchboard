use crate::contracts::{
    AuthenticationScheme, CodexSubagentRoute, ProviderConnectionOptions, ResponsesRequestMode,
    UpstreamProtocol,
};
use serde::{Deserialize, Serialize};

/// The only third-party protocols a Codex profile may target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexUpstream {
    Responses,
    ChatCompletions,
    AnthropicMessages,
}

/// How a third-party Codex provider is activated in the local client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CodexRouteMode {
    Direct,
    Gateway,
}

impl CodexRouteMode {
    pub const fn requires_gateway(self) -> bool {
        matches!(self, Self::Gateway)
    }

    /// Codex can speak a Responses provider directly only when its request
    /// shape and credential delivery both match the native client contract.
    /// `base_url` is the profile's primary service address: endpoint
    /// candidates equal to it give automatic routing nothing to choose.
    /// A sub-agent model route is a gateway capability, so a profile carrying
    /// one always routes locally. Every route-affecting profile field
    /// participates here so construction, validation, and later connection
    /// edits share one decision owner.
    pub fn for_profile(
        upstream: CodexUpstream,
        request_mode: ResponsesRequestMode,
        connection: &ProviderConnectionOptions,
        authentication: Option<AuthenticationScheme>,
        base_url: Option<&str>,
        subagent_route: Option<&CodexSubagentRoute>,
        has_model_routes: bool,
    ) -> Self {
        let default_authentication = upstream.protocol().authentication_scheme();
        if has_model_routes
            || subagent_route.is_some()
            || upstream != CodexUpstream::Responses
            || request_mode == ResponsesRequestMode::Minimal
            || connection.requires_gateway(base_url)
            || authentication.is_some_and(|selected| selected != default_authentication)
        {
            Self::Gateway
        } else {
            Self::Direct
        }
    }
}

impl CodexUpstream {
    pub const fn protocol(self) -> UpstreamProtocol {
        match self {
            Self::Responses => UpstreamProtocol::Responses,
            Self::ChatCompletions => UpstreamProtocol::ChatCompletions,
            Self::AnthropicMessages => UpstreamProtocol::AnthropicMessages,
        }
    }

    pub const fn from_protocol(protocol: UpstreamProtocol) -> Option<Self> {
        match protocol {
            UpstreamProtocol::Responses => Some(Self::Responses),
            UpstreamProtocol::ChatCompletions => Some(Self::ChatCompletions),
            UpstreamProtocol::AnthropicMessages => Some(Self::AnthropicMessages),
            UpstreamProtocol::GeminiGenerateContent => None,
        }
    }
}
