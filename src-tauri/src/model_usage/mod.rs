//! Persistent source observations own call deduplication and incremental totals.
mod parse;
mod ingest;
mod stream;
mod store;
mod resolve;
mod buckets;
mod reports;
mod refresh;
pub(crate) use refresh::refresh;
pub(crate) mod progress;
use asb_core::contracts::{AppKind, ModelUsageCoverage, ModelUsageDay, ModelUsageGroup,
    ModelUsageIssue, ModelUsageRange, ModelUsageReport, ModelUsageTokens};
use chrono::{DateTime, Local, Utc};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct TokenTotals {
    pub input: u64,
    pub cache_read: u64,
    pub cache_creation: u64,
    pub output: u64,
    pub total: u64,
}
impl TokenTotals {
    fn checked_sub(self, other: Self) -> Result<Self, String> {
        Ok(Self { input: self.input.checked_sub(other.input).ok_or("输入计数不一致")?,
            cache_read: self.cache_read.checked_sub(other.cache_read).ok_or("缓存计数不一致")?,
            cache_creation: self.cache_creation.checked_sub(other.cache_creation).ok_or("缓存写入计数不一致")?,
            output: self.output.checked_sub(other.output).ok_or("输出计数不一致")?,
            total: self.total.checked_sub(other.total).ok_or("总计数不一致")? })
    }
    fn checked_add(self, other: Self) -> Result<Self, String> {
        Ok(Self { input: self.input.checked_add(other.input).ok_or("输入计数溢出")?,
            cache_read: self.cache_read.checked_add(other.cache_read).ok_or("缓存计数溢出")?,
            cache_creation: self.cache_creation.checked_add(other.cache_creation).ok_or("缓存写入计数溢出")?,
            output: self.output.checked_add(other.output).ok_or("输出计数溢出")?,
            total: self.total.checked_add(other.total).ok_or("总计数溢出")? })
    }
    fn into_contract(self) -> ModelUsageTokens {
        ModelUsageTokens { input_tokens: self.input, cache_read_input_tokens: self.cache_read,
            cache_creation_input_tokens: self.cache_creation, output_tokens: self.output, total_tokens: self.total }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct UsageEvent {
    pub app: AppKind,
    pub identity: String,
    pub session: String,
    pub model: Option<String>,
    pub at: Option<String>,
    pub tokens: TokenTotals,
}


#[cfg(test)]
pub(crate) fn report(range: ModelUsageRange, sources: Vec<crate::session_manager::SessionSource>,
    issues: Vec<crate::session_manager::SessionIssue>, now: DateTime<Local>) -> ModelUsageReport {
    let temporary = tempfile::tempdir().unwrap();
    let index = crate::session_manager::read_index::ReadIndex::open(temporary.path()).unwrap();
    let progress = progress::Progress::new(&|_| {});
    let manifest = crate::session_manager::usage_sources::Manifest { sources, issues, complete: true };
    refresh(&index, "fixture", manifest, || now, &progress).unwrap().reports.into_iter().find(|report| report.range == range).unwrap()
}

#[cfg(test)]
pub(crate) fn fixture_reports(root: &std::path::Path, sources: Vec<crate::session_manager::SessionSource>,
    issues: Vec<crate::session_manager::SessionIssue>, now: DateTime<Local>) -> Result<Vec<ModelUsageReport>, String> {
    let index = crate::session_manager::read_index::ReadIndex::open(root)?;
    let progress = progress::Progress::new(&|_| {});
    let manifest = crate::session_manager::usage_sources::Manifest { sources, issues, complete: true };
    refresh(&index, "fixture", manifest, || now, &progress).map(|batch| batch.reports)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod incremental_tests;
