//! Bounded projections of native OTel facts, independent of gateway billing and session consumption.
mod aggregate;
mod contracts;
pub(crate) use contracts::*;
use aggregate::Accumulator;
use asb_core::contracts::AppKind;
use std::{collections::{BTreeMap, VecDeque}, path::Path};

const PAGE_SIZE: usize = 25;
const GROUP_PAGE_SIZE: usize = 20;

struct Projection {
    summary: Accumulator,
    groups: BTreeMap<String, (RequestUsageGroup, Accumulator)>,
    buckets: BTreeMap<u64, Accumulator>,
    sources: Vec<RequestUsageSource>,
    rows: Vec<RequestUsageRow>,
    tail: VecDeque<RequestUsageRow>,
    matched: usize,
}

impl Projection {
    fn new(app: Option<AppKind>) -> Self {
        Self { summary: Accumulator::default(), groups: BTreeMap::new(), buckets: BTreeMap::new(),
            sources: [AppKind::Codex, AppKind::Claude].into_iter().filter(|item| app.is_none_or(|app| app == *item))
                .map(|app| RequestUsageSource { app, verified_records: 0, earliest_at_ms: None, latest_at_ms: None }).collect(),
            rows: Vec::new(), tail: VecDeque::new(), matched: 0 }
    }

    fn add(&mut self, row: RequestUsageRow, query: &RequestUsageQuery, bucket_ms: u64) {
        if let Some(source) = self.sources.iter_mut().find(|source| source.app == row.app) {
            source.verified_records += 1;
            source.earliest_at_ms = Some(source.earliest_at_ms.unwrap_or(row.at_ms).min(row.at_ms));
            source.latest_at_ms = Some(source.latest_at_ms.unwrap_or(row.at_ms).max(row.at_ms));
        }
        if query.model.as_ref().is_some_and(|model| row.model.as_ref().is_none_or(|value|
            !value.to_lowercase().contains(&model.trim().to_lowercase()))) { return; }
        self.summary.add(&row);
        let key = serde_json::to_string(&(row.app, &row.model, &row.event_name)).expect("group identity");
        let group = self.groups.entry(key).or_insert_with(|| (RequestUsageGroup {
            app: row.app, model: row.model.clone(), event_name: row.event_name.clone(), metrics: Default::default(),
        }, Accumulator::default()));
        group.1.add(&row);
        self.buckets.entry(row.at_ms / bucket_ms * bucket_ms).or_default().add(&row);
        self.matched += 1;
        if self.matched > (query.page - 1) * PAGE_SIZE && self.rows.len() < PAGE_SIZE { self.rows.push(row.clone()); }
        if self.tail.len() == PAGE_SIZE { self.tail.pop_front(); }
        self.tail.push_back(row);
    }
}

pub(crate) fn read(root: &Path, query: RequestUsageQuery) -> Result<RequestUsageReport, String> {
    query.validate()?;
    let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let start = query.range.milliseconds().map(|span| now.saturating_sub(span)).unwrap_or(0);
    let bucket_ms = query.range.milliseconds().map(|span| (span / 48 / 60_000).max(1) * 60_000).unwrap_or(86_400_000);
    let mut result = Projection::new(query.app);
    crate::native_telemetry::store::visit(root, query.app, query.scope, start, now,
        |row| result.add(row, &query, bucket_ms))?;
    let start = if start == 0 {
        result.sources.iter().filter_map(|source| source.earliest_at_ms).min().unwrap_or(now)
    } else { start };
    let mut groups: Vec<_> = result.groups.into_values().map(|(mut group, accumulator)| {
        group.metrics = accumulator.finish(); group
    }).collect();
    groups.sort_by(|a, b| b.metrics.records.cmp(&a.metrics.records)
        .then(a.model.cmp(&b.model)).then(a.event_name.cmp(&b.event_name)));
    let total_groups = groups.len();
    let group_page = query.group_page.min(total_groups.saturating_sub(1) / GROUP_PAGE_SIZE + 1);
    let groups = groups.into_iter().skip((group_page - 1) * GROUP_PAGE_SIZE).take(GROUP_PAGE_SIZE).collect();
    let page = query.page.min(result.matched.saturating_sub(1) / PAGE_SIZE + 1);
    if page != query.page {
        let count = result.matched.saturating_sub((page - 1) * PAGE_SIZE);
        let skip = result.tail.len().saturating_sub(count);
        result.rows = result.tail.into_iter().skip(skip).collect();
    }
    let (bucket_ms, trend) = finish_trend(result.buckets, bucket_ms);
    Ok(RequestUsageReport { generated_at_ms: now, start_at_ms: start, end_at_ms: now, bucket_ms,
        scope: query.scope, summary: result.summary.finish(), sources: result.sources, groups,
        total_groups, group_page, group_page_size: GROUP_PAGE_SIZE, trend, rows: result.rows, page, page_size: PAGE_SIZE })
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
