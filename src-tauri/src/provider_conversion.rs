//! Source revision checks for read-only cross-client conversion.

use crate::commands::error::{operation_error, store_error, CommandError};
use crate::local_state::LocalState;
use asb_core::contracts::AppKind;
use asb_core::provider_conversion::{self, ProviderConversion, ProviderConversionRequest};

pub(crate) fn prepare(state: &LocalState, request: ProviderConversionRequest) -> Result<ProviderConversion, CommandError> {
    if request.source_app == request.target_app {
        return Err(CommandError::keyed("provider-conversion-same-client", "providers.conversion.error.sameClient", "转换目标必须是另一客户端"));
    }
    let store = state.configuration();
    if request.source_app == AppKind::Codex {
        let codex = store.list_codex_providers().map_err(store_error)?;
        if let Some(record) = codex.iter().find(|record| record.profile.id == request.source_profile_id) {
            check_revision(&record.file_hash, &request.expected_source_file_hash)?;
            return provider_conversion::from_codex(record).map_err(invalid);
        }
    }
    let record = store.find_provider_record(&request.source_profile_id)
        .map_err(|error| match error {
            crate::config_store::StoreOperationError::Store(_) => operation_error("provider-conversion-source-unavailable", error),
            _ => source_missing(),
        })?;
    if record.profile.app != request.source_app { return Err(source_missing()); }
    check_revision(&record.file_hash, &request.expected_source_file_hash)?;
    provider_conversion::from_provider(&record).map_err(invalid)
}

fn check_revision(actual: &str, expected: &str) -> Result<(), CommandError> {
    if actual != expected {
        return Err(CommandError::keyed("provider-conversion-stale", "providers.conversion.error.stale", "来源供应商已变化，请刷新后重新转换"));
    }
    Ok(())
}

fn source_missing() -> CommandError {
    CommandError::keyed("provider-conversion-source-missing", "providers.conversion.error.sourceMissing", "来源供应商不存在或不属于所选客户端")
}

fn invalid(error: String) -> CommandError {
    CommandError::new("provider-conversion-invalid", error)
}

#[cfg(test)]
#[path = "provider_conversion_tests.rs"]
mod tests;
