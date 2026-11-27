//! Claude Code `settings.json` adapter.
//!
//! Pure parse / preview / render over JSON text. Key order is preserved via
//! `serde_json` with the `preserve_order` feature; secrets are never written.
//! Claude Code keeps ownership of its existing login and credential
//! environment.

mod common;
mod document;
mod gateway;
pub use gateway::gateway_model_source;
mod native;
mod model_preferences;
mod takeover;
pub use takeover::restore_gateway_overlay;
mod overlay;
mod preview;
mod render;
mod state;
mod unmanaged;

pub(crate) use document::{check_syntax, parse};
pub use common::extract_client_settings;
pub(crate) use common::json_value;
pub(crate) use preview::preview;
pub(crate) use render::render_gateway_base_url;
pub(crate) use render::{render, render_entries};
pub use state::route_state;
pub(crate) use state::{full_diff, matches_provider_credentials, owned_diff};
pub(crate) use unmanaged::select_unmanaged;
