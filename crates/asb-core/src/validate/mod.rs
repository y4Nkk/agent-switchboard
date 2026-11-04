//! Early, loud validation for provider profiles and general settings.
//!
//! Invalid shapes fail here with an explanation of how to fix them; the
//! switch executor refuses to receive anything that has not passed.

mod claude;
mod codex;
mod error;
mod profile;
mod settings;
pub mod approval_policy;
mod usage;

pub(crate) use codex::validate_connection as validate_codex_connection;
pub use error::{ValidationError, MAX_AUTO_REFRESH_INTERVAL_MINUTES};
pub use usage::validate_usage_query;
pub use settings::validate_setting_value;

use crate::contracts::{ProviderProfile, SettingsValues};
/// Validates the only current planning contract. Client selection comes from
/// the provider profile, so the client settings cannot carry a second,
/// conflicting `app` value.
pub fn validate_plan(
    profile: &ProviderProfile,
    client_settings: &SettingsValues,
) -> Result<(), ValidationError> {
    profile.validate()?;
    client_settings.validate_client_settings(profile.app)
}
