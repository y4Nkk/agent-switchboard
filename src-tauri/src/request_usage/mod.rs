//! Bounded projections of native OTel facts, independent of gateway billing and session consumption.
mod aggregate;
mod contracts;
pub(crate) use contracts::*;
use aggregate::Accumulator;
use asb_core::contracts::AppKind;
use std::{collections::BTreeMap, path::Path, sync::{Arc, Mutex}};

const PAGE_SIZE: usize = 25;
const GROUP_PAGE_SIZE: usize = 20;

#[derive(Clone, Default)]
pub(crate) struct RequestUsageReader {
    current: Arc<Mutex<Option<Arc<Snapshot>>>>,
}

impl RequestUsageReader {
    pub(crate) fn read(&self, root: &Path, request: RequestUsageRead) -> Result<RequestUsageResult, String> {
        match request {
            RequestUsageRead::Refresh { query } => {
                let snapshot = Arc::new(build(root, &query)?);
                let report = snapshot.report(root, query.page, query.group_page)?;
                *self.current.lock().map_err(|_| "请求性能读取状态不可用")? = Some(snapshot);
                Ok(RequestUsageResult::Report(report))
            }
            RequestUsageRead::Page { snapshot_id, page, group_page } => {
                validate_pages(page, group_page)?;
                let snapshot = self.current.lock().map_err(|_| "请求性能读取状态不可用")?
                    .as_ref().filter(|snapshot| snapshot.id == snapshot_id).cloned()
                    .ok_or("请求性能快照已更新，请刷新报告")?;
                snapshot.pages(root, page, group_page).map(RequestUsageResult::Page)
            }
        }
    }
}

struct Snapshot {
    id: String,
    start: u64,
    end: u64,
    bucket_ms: u64,
    scope: MeasurementScope,
    summary: RequestUsageMetrics,
    sources: Vec<RequestUsageSource>,
    groups: Vec<RequestUsageGroup>,
    trend: Vec<RequestUsageBucket>,
    ids: Vec<String>,
}

impl Snapshot {
    fn report(&self, root: &Path, page: usize, group_page: usize) -> Result<RequestUsageReport, String> {
        Ok(RequestUsageReport { snapshot_id: self.id.clone(), generated_at_ms: self.end,
            start_at_ms: self.start, end_at_ms: self.end, bucket_ms: self.bucket_ms, scope: self.scope,
            summary: self.summary.clone(), sources: self.sources.clone(), trend: self.trend.clone(),
            pages: self.pages(root, page, group_page)? })
    }

    fn pages(&self, root: &Path, page: usize, group_page: usize) -> Result<RequestUsagePages, String> {
        let page = page.min(self.ids.len().saturating_sub(1) / PAGE_SIZE + 1);
        let offset = (page - 1) * PAGE_SIZE;
        let rows = crate::native_telemetry::store::read_rows(root,
            &self.ids[offset..(offset + PAGE_SIZE).min(self.ids.len())])?;
        let group_page = group_page.min(self.groups.len().saturating_sub(1) / GROUP_PAGE_SIZE + 1);
        let offset = (group_page - 1) * GROUP_PAGE_SIZE;
        Ok(RequestUsagePages {
            groups: self.groups[offset..(offset + GROUP_PAGE_SIZE).min(self.groups.len())].to_vec(),
            total_groups: self.groups.len(), group_page, group_page_size: GROUP_PAGE_SIZE,
            rows, page, page_size: PAGE_SIZE })
    }
}

struct Projection {
    summary: Accumulator,
    groups: BTreeMap<(AppKind, Option<String>, String), Accumulator>,
    buckets: BTreeMap<u64, Accumulator>,
    sources: Vec<RequestUsageSource>,
    ids: Vec<String>,
}

impl Projection {
    fn new(app: Option<AppKind>) -> Self {
        Self { summary: Accumulator::default(), groups: BTreeMap::new(), buckets: BTreeMap::new(),
            sources: [AppKind::Codex, AppKind::Claude].into_iter().filter(|item| app.is_none_or(|app| app == *item))
                .map(|app| RequestUsageSource { app, verified_records: 0, earliest_at_ms: None, latest_at_ms: None }).collect(),
            ids: Vec::new() }
    }

    fn add(&mut self, row: RequestUsageRow, model: Option<&str>, bucket_ms: u64) {
        if let Some(source) = self.sources.iter_mut().find(|source| source.app == row.app) {
            source.verified_records += 1;
            source.earliest_at_ms = Some(source.earliest_at_ms.unwrap_or(row.at_ms).min(row.at_ms));
            source.latest_at_ms = Some(source.latest_at_ms.unwrap_or(row.at_ms).max(row.at_ms));
        }
        if model.is_some_and(|model| row.model.as_ref().is_none_or(|value|
            !value.to_lowercase().contains(model))) { return; }
        self.summary.add(&row);
        self.groups.entry((row.app, row.model.clone(), row.event_name.clone())).or_default().add(&row);
        self.buckets.entry(row.at_ms / bucket_ms * bucket_ms).or_default().add(&row);
        self.ids.push(row.id);
    }
}

fn build(root: &Path, query: &RequestUsageQuery) -> Result<Snapshot, String> {
    query.validate()?;
    let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let start = query.range.milliseconds().map(|span| now.saturating_sub(span)).unwrap_or(0);
    let bucket_ms = query.range.milliseconds().map(|span| (span / 48 / 60_000).max(1) * 60_000).unwrap_or(86_400_000);
    let model = query.model.as_ref().map(|value| value.trim().to_lowercase());
    let mut result = Projection::new(query.app);
    crate::native_telemetry::store::visit(root, query.app, query.scope, start, now,
        |row| result.add(row, model.as_deref(), bucket_ms))?;
    let start = if start == 0 {
        result.sources.iter().filter_map(|source| source.earliest_at_ms).min().unwrap_or(now)
    } else { start };
    let mut groups: Vec<_> = result.groups.into_iter().map(|((app, model, event_name), accumulator)|
        RequestUsageGroup { app, model, event_name, metrics: accumulator.finish() }).collect();
    groups.sort_by(|a, b| b.metrics.records.cmp(&a.metrics.records)
        .then(a.model.cmp(&b.model)).then(a.event_name.cmp(&b.event_name))
        .then_with(|| (a.app == AppKind::Codex).cmp(&(b.app == AppKind::Codex))));
    let (bucket_ms, trend) = finish_trend(result.buckets, bucket_ms);
    Ok(Snapshot { id: uuid::Uuid::new_v4().to_string(), start, end: now, bucket_ms,
        scope: query.scope, summary: result.summary.finish(), sources: result.sources, groups, trend, ids: result.ids })
}

fn finish_trend(buckets: BTreeMap<u64, Accumulator>, step: u64) -> (u64, Vec<RequestUsageBucket>) {
    let first = buckets.first_key_value().map(|(at, _)| *at).unwrap_or(0);
    let last = buckets.last_key_value().map(|(at, _)| *at).unwrap_or(first);
    let width = (((last - first) / step / 95) + 1) * step;
    let mut merged = BTreeMap::<u64, Accumulator>::new();
    for (at, accumulator) in buckets { merged.entry(at / width * width).or_default().merge(accumulator); }
    let trend = merged.into_iter().map(|(at_ms, acc)| {
        let metrics = acc.finish();
        RequestUsageBucket { at_ms, speed_p50: metrics.output_rate_p50,
            first_token_p50_ms: metrics.first_token_p50_ms,
            duration_p50_ms: metrics.duration_p50_ms,
            failure_rate: metrics.success_ratio.map(|ratio| 100.0 - ratio) }
    }).collect();
    (width, trend)
}

#[cfg(test)]
mod tests;
