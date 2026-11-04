//! Read-only projections of gateway-owned request facts, separate from session consumption.
mod aggregate;
mod contracts;
mod sources;
pub(crate) use contracts::*;
use aggregate::Accumulator;
use asb_core::contracts::AppKind;
use std::{collections::{BTreeMap, VecDeque}, path::Path};

const PAGE_SIZE: usize = 25;
const GROUP_PAGE_SIZE: usize = 20;

pub(crate) fn read(root: &Path, claude_path: &Path, query: RequestUsageQuery)
    -> Result<RequestUsageReport, String>
{
    query.validate()?;
    let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let start = query.range.milliseconds().map(|span| now.saturating_sub(span)).unwrap_or(0);
    let mut summary = Accumulator::default();
    let mut groups = BTreeMap::<String, (RequestUsageGroup, Accumulator)>::new();
    let mut buckets = BTreeMap::<u64, Accumulator>::new();
    let bucket_ms = query.range.milliseconds().map(bucket_size).unwrap_or(86_400_000);
    let mut rows = Vec::new();
    let mut matched = 0_usize;
    let mut sources = Vec::new();
    let mut tail = VecDeque::new();
    // Claude's existing bounded snapshot is merged with the ordered SQLite cursor.
    // Only the requested page and a final-page tail are retained from the merged stream.
    let mut claude_rows = Vec::new();
    if query.app.is_none_or(|app| app == AppKind::Claude) {
        sources.push(sources::visit(root, claude_path, AppKind::Claude, start, now, |row| claude_rows.push(row)));
    }
    claude_rows.sort_by(|a, b| (a.at_ms, &a.id).cmp(&(b.at_ms, &b.id)));
    let mut consume = |row: RequestUsageRow| {
        if !matches_query(&row, &query) { return; }
        summary.add(&row);
        let key = serde_json::to_string(&(row.app, &row.profile_id, &row.model, row.model_source))
            .expect("serializable group identity");
        let entry = groups.entry(key).or_insert_with(|| (RequestUsageGroup {
            app: row.app, profile_id: row.profile_id.clone(), model: row.model.clone(),
            model_source: row.model_source, metrics: Default::default(),
        }, Accumulator::default()));
        entry.1.add(&row);
        buckets.entry(row.at_ms / bucket_ms * bucket_ms).or_default().add(&row);
        matched += 1;
        if matched > (query.page - 1) * PAGE_SIZE && rows.len() < PAGE_SIZE { rows.push(row.clone()); }
        if tail.len() == PAGE_SIZE { tail.pop_front(); }
        tail.push_back(row);
    };
    if query.app.is_none_or(|app| app == AppKind::Codex) {
        sources.push(sources::visit(root, claude_path, AppKind::Codex, start, now, |row| {
            while claude_rows.last().is_some_and(|other| (other.at_ms, &other.id) > (row.at_ms, &row.id)) {
                consume(claude_rows.pop().expect("existing Claude row"));
            }
            consume(row);
        }));
    }
    while let Some(row) = claude_rows.pop() { consume(row); }
    let summary = summary.finish();
    let start = if start == 0 {
        sources.iter().filter_map(|source| source.earliest_at_ms).min().unwrap_or(now)
    } else { start };
    let mut groups: Vec<_> = groups.into_values().map(|(mut group, accumulator)| {
        group.metrics = accumulator.finish(); group
    }).collect();
    groups.sort_by(|a, b| b.metrics.requests.cmp(&a.metrics.requests)
        .then(a.profile_id.cmp(&b.profile_id)).then(a.model.cmp(&b.model)));
    let total_groups = groups.len();
    let group_page = query.group_page.min(total_groups.saturating_sub(1) / GROUP_PAGE_SIZE + 1);
    let groups = groups.into_iter().skip((group_page - 1) * GROUP_PAGE_SIZE).take(GROUP_PAGE_SIZE).collect();
    let page = query.page.min(matched.saturating_sub(1) / PAGE_SIZE + 1);
    if page != query.page {
        let count = matched.saturating_sub((page - 1) * PAGE_SIZE);
        let skip = tail.len().saturating_sub(count);
        rows = tail.into_iter().skip(skip).collect();
    }
    let (bucket_ms, trend) = finish_trend(buckets, bucket_ms);
    Ok(RequestUsageReport { generated_at_ms: now, start_at_ms: start, end_at_ms: now,
        bucket_ms, summary, sources, groups, total_groups, group_page, group_page_size: GROUP_PAGE_SIZE,
        trend, rows, page, page_size: PAGE_SIZE })
}

fn matches_query(row: &RequestUsageRow, query: &RequestUsageQuery) -> bool {
    query.profile_id.as_ref().is_none_or(|profile| profile == &row.profile_id)
        && query.model.as_ref().is_none_or(|model| row.model.as_ref()
            .is_some_and(|value| value.to_lowercase().contains(&model.trim().to_lowercase())))
}

fn bucket_size(span: u64) -> u64 { (span / 48 / 60_000).max(1) * 60_000 }

fn finish_trend(buckets: BTreeMap<u64, Accumulator>, step: u64) -> (u64, Vec<RequestUsageBucket>) {
    // All-history can span years. Merge accumulators before percentiles, never merge percentiles.
    let first = buckets.first_key_value().map(|(at, _)| *at).unwrap_or(0);
    let last = buckets.last_key_value().map(|(at, _)| *at).unwrap_or(first);
    let width = (((last - first) / step / 95) + 1) * step;
    let mut merged = BTreeMap::<u64, Accumulator>::new();
    for (at, accumulator) in buckets {
        merged.entry(at / width * width).or_default().merge(accumulator);
    }
    let trend = merged.into_iter().map(|(at_ms, acc)| {
        let metrics = acc.finish();
        RequestUsageBucket { at_ms, requests: metrics.requests, speed_p50: metrics.output_rate_p50,
            first_output_p50_ms: metrics.first_output_p50_ms,
            failure_rate: aggregate::percentage(metrics.failed, metrics.completed + metrics.failed) }
    }).collect();
    (width, trend)
}

#[cfg(test)]
mod tests;
