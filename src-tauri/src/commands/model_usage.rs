//! Read-only local model-token usage command.

use super::error::{blocking, CommandError};
use asb_core::contracts::{ModelUsageRange, ModelUsageReport};

/// Aggregates token consumption from the fixed local Codex and Claude Code
/// session roots. The renderer supplies only a calendar range; no profile,
/// provider credential, session path, or raw session record crosses this API.
#[tauri::command]
pub async fn get_model_usage_report(
    range: ModelUsageRange,
) -> Result<ModelUsageReport, CommandError> {
    blocking(move || Ok(crate::model_usage::get_model_usage_report(range))).await
}
