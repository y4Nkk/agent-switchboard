//! Manual endpoint probing and the shared outbound HTTP transport.
//!
//! One probe answers exactly one question: does this URL answer HTTP(S)
//! requests, with which status, and how fast. It never selects or switches
//! anything. The transport is [`reqwest`] with rustls verifying against the
//! OS root store, so the system trust and proxy settings apply the way the
//! native stack did. The shared [`http_get`] helper carries that same stack
//! to other outbound reads (model fetch, update check, quota, OAuth).

mod claude_gemini;
mod models;
mod codex_operations;
mod transport;



pub(crate) use models::{parse_models_value, provider_auth_headers};
pub(crate) use models::{fetch_models_with_operations, ProviderModelDiscovery};
pub use models::{fetch_models, ProviderModel};
pub use transport::{
    http_get, http_get_with_options, http_request, http_request_with_options, probe, ProbeResult,
    RequestFailure,
};
#[expect(unused_imports)] // reserved: warmed probe surface
pub use transport::probe_warmed;
