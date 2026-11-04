//! Codex `config.toml` adapter.
//!
//! Pure parse / preview / render over configuration text. Host-owned keys,
//! comments and layout are preserved by editing the document through
//! `toml_edit` instead of re-serializing from a typed mirror.

mod common;
mod permissions;
pub use permissions::{permission_profile_status, PermissionProfileStatus};
mod common_fragment;
mod document;
pub use common::extract_client_settings;
pub use common::toml_value;
pub use common_fragment::{declared_mcp_server_keys, fragment_is_applied, validate_fragment};
mod overlay;
mod preview;
mod render;
mod restore;
mod state;
mod subagents;
mod unmanaged;

pub(crate) use document::{check_syntax, parse};
pub use restore::validate_restore_configuration;
pub(crate) use preview::preview;
pub(crate) use render::{render, render_client_settings_into_file, render_gateway_base_url};
pub(crate) use state::{full_diff, matches_provider_settings, owned_diff};
pub use state::{route_state, OFFICIAL_PROVIDER};
pub use subagents::{read_subagent_settings, render_subagent_settings};
pub(crate) use unmanaged::remove_unmanaged;

fn validate_projection(
    plan: &crate::contracts::SwitchPlan,
) -> Result<(), crate::adapter::AdapterError> {
    if plan.is_gateway() {
        if !plan.client_base_url().is_some_and(is_gateway_base_url) {
            return Err(crate::adapter::AdapterError {
                message: "Codex 网关投影地址无效".into(),
                line: None,
            });
        }
        return Ok(());
    }
    if plan.profile.route_mode != crate::contracts::RouteMode::Custom {
        return Ok(());
    }
    if plan.profile.upstream_protocol != Some(crate::contracts::UpstreamProtocol::Responses) {
        return Err(crate::adapter::AdapterError {
            message: "Codex Chat Completions 与 Anthropic 上游必须通过本机网关投影".into(),
            line: None,
        });
    }
    if plan.client_base_url().is_none() {
        return Err(crate::adapter::AdapterError {
            message: "Codex 直连投影缺少 Responses 服务地址".into(),
            line: None,
        });
    }
    Ok(())
}

/// The only endpoint shape a Codex projection may write; it must not send
/// ambient OAuth to an upstream address supplied as a gateway projection.
pub fn is_gateway_base_url(value: &str) -> bool {
    let Ok(url) = url::Url::parse(value) else {
        return false;
    };
    let segments: Vec<_> = url.path().split('/').collect();
    url.scheme() == "http"
        && url.host_str() == Some("127.0.0.1")
        && url.port().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && matches!(segments.as_slice(), ["", "codex", capability, "v1"]
        if capability.strip_prefix("asb_codex_").is_some_and(|secret| {
            secret.len() == 64 && secret.bytes().all(|c| c.is_ascii_hexdigit())
        }))
}
