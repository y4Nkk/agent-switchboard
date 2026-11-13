use crate::contracts::UsageQuery;

use crate::validate::error::{ValidationError, MAX_AUTO_REFRESH_INTERVAL_MINUTES};

const MAX_USAGE_QUERY_SOURCE_LEN: usize = 65_536;

/// Owns the serializable usage-query contract for both clients. Source code
/// executes only during a query: runtime resource failures cannot determine
/// whether an unchanged provider file is valid.
pub fn validate_usage_query(query: &UsageQuery) -> Result<(), ValidationError> {
    if query.refresh_interval_minutes() > MAX_AUTO_REFRESH_INTERVAL_MINUTES {
        return Err(ValidationError::UsageQueryRefreshIntervalTooLarge(
            MAX_AUTO_REFRESH_INTERVAL_MINUTES,
        ));
    }
    match query {
        UsageQuery::Declarative {
            url,
            remaining_path,
            used_path,
            total_path,
            unit,
            ..
        } => {
            let url = url.trim();
            if url.is_empty() {
                return Err(ValidationError::EmptyUsageQueryUrl);
            }
            if !(url.starts_with("https://")
                || url.starts_with("http://")
                || url.starts_with("{{baseUrl}}"))
                || url.chars().any(char::is_control)
            {
                return Err(ValidationError::BadUsageQueryUrl);
            }
            if remaining_path.is_none() && used_path.is_none() && total_path.is_none() {
                return Err(ValidationError::UsageQueryExtractsNothing);
            }
            for (field, value) in [
                ("remainingPath", remaining_path),
                ("usedPath", used_path),
                ("totalPath", total_path),
                ("unit", unit),
            ] {
                if value.as_ref().is_some_and(|value| {
                    value.trim().is_empty() || value.chars().any(char::is_control)
                }) {
                    return Err(ValidationError::EmptyUsageQueryField { field });
                }
            }
            Ok(())
        }
        UsageQuery::Script { source, .. } => {
            if source.trim().is_empty() {
                return Err(ValidationError::EmptyUsageQueryScript);
            }
            if source.chars().count() > MAX_USAGE_QUERY_SOURCE_LEN {
                return Err(ValidationError::UsageQueryScriptTooLong(
                    MAX_USAGE_QUERY_SOURCE_LEN,
                ));
            }
            Ok(())
        }
    }
}
