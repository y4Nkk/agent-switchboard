use super::*;
use std::collections::{BTreeMap, HashSet};
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

struct Accumulator {
    calendar: CalendarRange,
    groups: BTreeMap<GroupKey, TokenTotals>,
    sessions: BTreeMap<GroupKey, HashSet<String>>,
    days: BTreeMap<NaiveDate, TokenTotals>,
    undated: TokenTotals,
    measured: HashSet<(AppKind, String)>,
    undated_event_tokens: u64,
}
impl Accumulator {
    fn new(calendar: CalendarRange) -> Self {
        Self { calendar, groups: BTreeMap::new(), sessions: BTreeMap::new(), days: BTreeMap::new(),
            undated: TokenTotals::default(), measured: HashSet::new(), undated_event_tokens: 0 }
    }
    fn add(&mut self, event: &buckets::Bucket) -> Result<(), String> {
        self.measured.insert((event.app, event.session.clone()));
        if event.tokens.total == 0 { return Ok(()); }
        if event.day.is_none() {
            self.undated_event_tokens = self.undated_event_tokens.checked_add(event.tokens.total).ok_or("无日期用量计数溢出")?;
        }
        let Some(assignment) = self.calendar.assignment(event.day) else { return Ok(()); };
        let key = GroupKey { app: event.app, model: event.model.clone() };
        let totals = self.groups.entry(key.clone()).or_default().checked_add(event.tokens)?;
        match assignment {
            Some(date) => { let destination = self.days.entry(date).or_default(); *destination = destination.checked_add(event.tokens)?; }
            None => self.undated = self.undated.checked_add(event.tokens)?,
        }
        self.groups.insert(key.clone(), totals);
        self.sessions.entry(key).or_default().insert(event.session.clone());
        Ok(())
    }
}

fn finish(
    range: ModelUsageRange, now: DateTime<Local>, aggregate: Accumulator,
    coverage: ModelUsageCoverage, issues: Vec<ModelUsageIssue>,
) -> ModelUsageReport {
    let mut seen_issues = HashSet::new();
    ModelUsageReport { range, generated_at: now.with_timezone(&Utc).to_rfc3339(),
        groups: aggregate.groups.into_iter().map(|(key, totals)| ModelUsageGroup {
            session_count: aggregate.sessions.get(&key).map_or(0, |sessions| sessions.len()) as u64,
            app: key.app, model: key.model, input_tokens: totals.input, cache_read_input_tokens: totals.cache_read,
            cache_creation_input_tokens: totals.cache_creation, output_tokens: totals.output, total_tokens: totals.total }).collect(),
        days: aggregate.days.into_iter().map(|(date, totals)| ModelUsageDay { date: date.to_string(),
            input_tokens: totals.input, cache_read_input_tokens: totals.cache_read,
            cache_creation_input_tokens: totals.cache_creation, output_tokens: totals.output, total_tokens: totals.total }).collect(),
        unassigned_tokens: aggregate.undated.into_contract(), coverage,
        issues: issues.into_iter().filter(|issue| seen_issues.insert((issue.app, issue.message.clone()))).collect() }
}


pub(super) fn build(connection: &rusqlite::Connection, scope: &str,
    sources: &[crate::session_manager::SessionSource], scanned_files: u64,
    issues: Vec<ModelUsageIssue>, now: DateTime<Local>) -> Result<Vec<ModelUsageReport>, String> {
    let buckets = buckets::read(connection, scope)?;
    let mut results = Vec::new();
    for range in [ModelUsageRange::Today, ModelUsageRange::Last7Days, ModelUsageRange::Last30Days, ModelUsageRange::All] {
        let mut aggregate = Accumulator::new(CalendarRange::new(range, now));
        for bucket in &buckets { aggregate.add(bucket)?; }
        let coverage = ModelUsageCoverage { source_sessions: sources.len() as u64, scanned_files,
            sessions_without_usage: sources.iter().filter(|source|
                !aggregate.measured.contains(&(source.meta.app, source.meta.session_id.clone()))).count() as u64,
            undated_event_tokens: aggregate.undated_event_tokens };
        results.push(finish(range, now, aggregate, coverage, issues.clone()));
    }
    Ok(results)
}
