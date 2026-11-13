//! Profile-owned usage-balance queries.
//!
//! Declarative queries retain fixed GET, the profile-selected credential
//! header, and JSON-Pointer extraction. Script queries evaluate a small
//! JavaScript expression in a fresh, resource-bounded QuickJS context. The
//! script can calculate a request and extract JSON, but receives no host I/O;
//! the shared probe transport remains the only process that performs a
//! network request. Both query modes normalize extracted facts through one
//! business owner; exhausted or unavailable quotas are successful readings.

mod declarative;
mod response;
pub(crate) mod scheduler;
mod script;


use asb_core::contracts::{ProviderConnectionOptions, UpstreamProtocol, UsageQuery, UsageSummary};
use declarative::run_declarative_query;
use script::run_script_query;

/// Runs one usage query against the provider endpoint. The credential travels
/// only in the declarative mode's selected auth header, or into the script's
/// explicit `request` input. It is never echoed in errors or the returned
/// summary.
#[cfg(test)]
#[allow(dead_code)] // usage query harness
pub fn run_usage_query(
    query: &UsageQuery,
    api_key: &str,
    base_url: Option<&str>,
    upstream_protocol: UpstreamProtocol,
    authentication: Option<asb_core::AuthenticationScheme>,
) -> Result<UsageSummary, String> {
    run_usage_query_with_connection(
        query,
        api_key,
        base_url,
        upstream_protocol,
        authentication,
        &ProviderConnectionOptions::default(),
    )
}

/// Runs one usage query with the complete provider connection contract. The
/// query remains metadata-owned, while endpoint, auth and request overrides
/// follow the same rules as model discovery and gateway forwarding.
pub fn run_usage_query_with_connection(
    query: &UsageQuery,
    api_key: &str,
    base_url: Option<&str>,
    upstream_protocol: UpstreamProtocol,
    authentication: Option<asb_core::AuthenticationScheme>,
    connection: &ProviderConnectionOptions,
) -> Result<UsageSummary, String> {
    if api_key.chars().any(char::is_control) {
        return Err("API 密钥包含无效的请求头字符".into());
    }
    asb_core::validate::validate_usage_query(query).map_err(|error| error.to_string())?;
    let selected_base_url =
        base_url.and_then(|base| connection.endpoint_candidates(base).into_iter().next());
    match query {
        UsageQuery::Declarative {
            url,
            remaining_path,
            used_path,
            total_path,
            unit,
            ..
        } => run_declarative_query(
            url,
            remaining_path.as_deref(),
            used_path.as_deref(),
            total_path.as_deref(),
            unit.clone(),
            api_key,
            selected_base_url.as_deref(),
            upstream_protocol,
            authentication,
            connection,
        ),
        UsageQuery::Script { source, .. } => {
            run_script_query(source, api_key, selected_base_url.as_deref(), connection)
        }
    }
}
