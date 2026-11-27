use super::*;
use asb_core::contracts::ModelUsageDayModel;
use std::collections::{BTreeMap, HashMap, HashSet};
use chrono::{Duration, NaiveDate};
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct GroupKey { app: AppKind, model: Option<String> }

#[derive(Debug, Clone, Copy)]
struct CalendarRange { start: Option<NaiveDate>, end: NaiveDate }
impl CalendarRange {
    fn new(range: ModelUsageRange, now: DateTime<Local>) -> Self {
        let end = now.date_naive();
        let start = match range { ModelUsageRange::Today => Some(end),
            ModelUsageRange::Last7Days => Some(end - Duration::days(6)),
            ModelUsageRange::Last30Days => Some(end - Duration::days(29)), ModelUsageRange::All => None };
        Self { start, end }
    }
    fn assignment(self, date: Option<NaiveDate>) -> Option<Option<NaiveDate>> {
        match date {
            Some(date) if self.start.is_none_or(|start| (start..=self.end).contains(&date)) => Some(Some(date)),
            Some(_) => None,
            None if self.start.is_none() => Some(None),
            None => None,
        }
    }
}

#[derive(Default)]
struct DayTotals {
    tokens: TokenTotals,
    models: BTreeMap<Option<String>, u64>,
}

struct Accumulator {
    calendar: CalendarRange,
    groups: BTreeMap<GroupKey, TokenTotals>,
    sessions: BTreeMap<GroupKey, HashSet<String>>,
    days: BTreeMap<NaiveDate, DayTotals>,
    undated: TokenTotals,
}
impl Accumulator {
    fn new(calendar: CalendarRange) -> Self {
        Self { calendar, groups: BTreeMap::new(), sessions: BTreeMap::new(), days: BTreeMap::new(),
            undated: TokenTotals::default() }
    }
    fn add(&mut self, event: &buckets::Bucket) -> Result<(), String> {
        let Some(assignment) = self.calendar.assignment(event.day) else { return Ok(()); };
        let key = GroupKey { app: event.app, model: event.model.clone() };
        let totals = self.groups.entry(key.clone()).or_default();
        *totals = totals.checked_add(event.tokens)?;
        match assignment {
            Some(date) => {
                let day = self.days.entry(date).or_default();
                day.tokens = day.tokens.checked_add(event.tokens)?;
                *day.models.entry(event.model.clone()).or_default() += event.tokens.total;
            }
            None => self.undated = self.undated.checked_add(event.tokens)?,
        }
        self.sessions.entry(key).or_default().insert(event.session.clone());
        Ok(())
    }
}

fn finish(
    range: ModelUsageRange, now: DateTime<Local>, aggregate: Accumulator,
    coverage: ModelUsageCoverage, issues: Vec<ModelUsageIssue>, longest_session_span_seconds: Option<u64>,
) -> ModelUsageReport {
    let mut seen_issues = HashSet::new();
    ModelUsageReport { range, generated_at: now.with_timezone(&Utc).to_rfc3339(),
        groups: aggregate.groups.into_iter().map(|(key, totals)| ModelUsageGroup {
            session_count: aggregate.sessions.get(&key).map_or(0, |sessions| sessions.len()) as u64,
            app: key.app, model: key.model, input_tokens: totals.input, cache_read_input_tokens: totals.cache_read,
            cache_creation_input_tokens: totals.cache_creation, output_tokens: totals.output, total_tokens: totals.total }).collect(),
        days: aggregate.days.into_iter().map(|(date, day)| ModelUsageDay { date: date.to_string(),
            input_tokens: day.tokens.input, cache_read_input_tokens: day.tokens.cache_read,
            cache_creation_input_tokens: day.tokens.cache_creation, output_tokens: day.tokens.output,
            total_tokens: day.tokens.total,
            models: Some(day.models.into_iter().map(|(model, total_tokens)| ModelUsageDayModel { model, total_tokens }).collect()),
        }).collect(),
        unassigned_tokens: aggregate.undated.into_contract(), coverage,
        longest_session_span_seconds: if range == ModelUsageRange::All { longest_session_span_seconds } else { None },
        issues: issues.into_iter().filter(|issue| seen_issues.insert((issue.app, issue.message.clone()))).collect() }
}

fn longest_session_span(connection: &rusqlite::Connection, scope: &str, now: DateTime<Local>) -> Result<Option<u64>, String> {
    let mut statement = connection.prepare("SELECT data FROM usage_calls WHERE scope=?1").map_err(store::error)?;
    let mut rows = statement.query([scope]).map_err(store::error)?;
    let mut sessions = HashMap::new();
    let mut longest = None;
    while let Some(row) = rows.next().map_err(store::error)? {
        let raw: String = row.get(0).map_err(store::error)?;
        let call: UsageEvent = serde_json::from_str(&raw).map_err(store::error)?;
        if call.tokens.total == 0 { continue; }
        let Some(at) = call.at.as_deref().and_then(|at| DateTime::parse_from_rfc3339(at).ok()) else { continue; };
        if at > now { continue; }
        match sessions.entry((call.app, call.session)) {
            std::collections::hash_map::Entry::Vacant(entry) => { entry.insert((at, at)); }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let (first, last) = entry.get_mut();
                *first = (*first).min(at);
                *last = (*last).max(at);
                let span = last.signed_duration_since(*first).num_seconds() as u64;
                longest = longest.max(Some(span));
            }
        }
    }
    Ok(longest)
}

pub(super) fn build(connection: &rusqlite::Connection, scope: &str,
    sources: &[crate::session_manager::SessionSource], scanned_files: u64,
    issues: Vec<ModelUsageIssue>, now: DateTime<Local>) -> Result<Vec<ModelUsageReport>, String> {
    let mut ranges = [ModelUsageRange::Today, ModelUsageRange::Last7Days, ModelUsageRange::Last30Days, ModelUsageRange::All]
        .map(|range| (range, Accumulator::new(CalendarRange::new(range, now))));
    let mut measured = HashSet::new();
    let mut undated_event_tokens = 0u64;
    buckets::visit(connection, scope, |bucket| {
        if bucket.tokens.total > 0 {
            if bucket.day.is_none() {
                undated_event_tokens = undated_event_tokens.checked_add(bucket.tokens.total).ok_or("无日期用量计数溢出")?;
            }
            for (_, aggregate) in &mut ranges { aggregate.add(&bucket)?; }
        }
        measured.insert((bucket.app, bucket.session));
        Ok(())
    })?;
    let coverage = ModelUsageCoverage { source_sessions: sources.len() as u64, scanned_files,
        sessions_without_usage: sources.iter().filter(|source|
            !measured.contains(&(source.meta.app, source.meta.session_id.clone()))).count() as u64,
        undated_event_tokens };
    let span = longest_session_span(connection, scope, now)?;
    Ok(ranges.into_iter().map(|(range, aggregate)|
        finish(range, now, aggregate, coverage.clone(), issues.clone(), span)).collect())
}
