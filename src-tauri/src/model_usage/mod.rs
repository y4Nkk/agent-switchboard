//! Exact local call usage; context estimates never become consumption.
mod parse;
mod file_index;
use asb_core::contracts::{AppKind, ModelUsageCoverage, ModelUsageDay, ModelUsageGroup,
    ModelUsageIssue, ModelUsageRange, ModelUsageReport, ModelUsageTokens};
use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct TokenTotals {
    pub input: u64,
    pub cache_read: u64,
    pub cache_creation: u64,
    pub output: u64,
    pub total: u64,
}
impl TokenTotals {
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
    fn assignment(self, timestamp: Option<&str>) -> Option<Option<NaiveDate>> {
        match timestamp.and_then(parse::local_date) {
            Some(date) if self.start.is_none_or(|start| (start..=self.end).contains(&date)) => Some(Some(date)),
            Some(_) => None,
            None if self.start.is_none() => Some(None),
            None => None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct UsageEvent {
    pub app: AppKind,
    pub identity: String,
    pub session: String,
    pub model: Option<String>,
    pub at: Option<String>,
    pub tokens: TokenTotals,
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
    fn add(&mut self, event: UsageEvent) -> Result<(), String> {
        self.measured.insert((event.app, event.session.clone()));
        if event.tokens.total == 0 { return Ok(()); }
        if event.at.as_deref().and_then(parse::local_date).is_none() {
            self.undated_event_tokens = self.undated_event_tokens.checked_add(event.tokens.total).ok_or("无日期用量计数溢出")?;
        }
        let Some(assignment) = self.calendar.assignment(event.at.as_deref()) else { return Ok(()); };
        let key = GroupKey { app: event.app, model: event.model };
        let totals = self.groups.entry(key.clone()).or_default().checked_add(event.tokens)?;
        match assignment {
            Some(date) => { let destination = self.days.entry(date).or_default(); *destination = destination.checked_add(event.tokens)?; }
            None => self.undated = self.undated.checked_add(event.tokens)?,
        }
        self.groups.insert(key.clone(), totals);
        self.sessions.entry(key).or_default().insert(event.session);
        Ok(())
    }
}

pub(crate) fn reports(
    root: &std::path::Path,
    mut sources: Vec<crate::session_manager::SessionSource>,
    source_issues: Vec<crate::session_manager::SessionIssue>,
    now: DateTime<Local>,
) -> Result<Vec<ModelUsageReport>, String> {
    sources.retain(|source| source.converted.is_none());
    sources.sort_by(|left, right| (left.meta.app, &left.meta.session_id).cmp(&(right.meta.app, &right.meta.session_id)));
    let mut issues: Vec<ModelUsageIssue> = source_issues.into_iter().filter(|issue| issue.conversion_id.is_none())
        .map(|issue| ModelUsageIssue { app: issue.app, message: issue.message }).collect();
    let index = crate::session_manager::read_index::ReadIndex::open(root)?;
    let (events, scanned_files) = collect_events(&index, &sources, &mut issues);
    crate::session_manager::diagnostics::report(root, "模型用量来源",
        issues.iter().map(|issue| format!("{}: {}", issue.app.label(), issue.message)));
    let mut results = Vec::new();
    for range in [ModelUsageRange::Today, ModelUsageRange::Last7Days, ModelUsageRange::Last30Days, ModelUsageRange::All] {
        let mut aggregate = Accumulator::new(CalendarRange::new(range, now));
        let mut range_issues = issues.clone();
        for event in &events {
            if let Err(message) = aggregate.add(event.clone()) { range_issues.push(ModelUsageIssue { app: event.app, message }); }
        }
        let coverage = ModelUsageCoverage { source_sessions: sources.len() as u64, scanned_files,
            sessions_without_usage: sources.iter().filter(|source|
                !aggregate.measured.contains(&(source.meta.app, source.meta.session_id.clone()))).count() as u64,
            undated_event_tokens: aggregate.undated_event_tokens };
        results.push(finish(range, now, aggregate, coverage, range_issues));
    }
    Ok(results)
}

fn collect_events(
    index: &crate::session_manager::read_index::ReadIndex,
    sources: &[crate::session_manager::SessionSource], issues: &mut Vec<ModelUsageIssue>,
) -> (Vec<UsageEvent>, u64) {
    let mut codex = HashMap::new();
    let mut claude = HashMap::new();
    let mut scanned_files = 0;
    for source in sources {
        for record in source.record_sources() {
            scanned_files += 1;
            match file_index::read_source(index, &record, source.meta.app, &source.meta.session_id) {
                Ok(file) => {
                    issues.extend(file.issues.into_iter().map(|message| ModelUsageIssue { app: source.meta.app, message }));
                    match file.state {
                        file_index::UsageState::Codex(state) => for event in state.events() {
                            if let Err(message) = crate::codex_metering::merge_token_event(&mut codex, event) {
                                issues.push(ModelUsageIssue { app: AppKind::Codex, message });
                            }
                        },
                        file_index::UsageState::Claude(state) => for candidate in state.into_values() { parse::merge(&mut claude, candidate); },
                    }
                }
                Err(message) => issues.push(ModelUsageIssue { app: source.meta.app, message }),
            }
        }
    }
    let mut events: Vec<_> = codex.into_values().map(|event| {
        let tokens = event.tokens;
        UsageEvent { app: AppKind::Codex, identity: event.identity, session: event.owner,
            model: event.model, at: event.at, tokens: TokenTotals {
                input: tokens.input - tokens.cached_input - tokens.cache_write_input,
                cache_read: tokens.cached_input, cache_creation: tokens.cache_write_input, output: tokens.output,
                total: tokens.total().expect("validated receipt total") } }
    }).collect();
    events.extend(claude.into_values().map(|candidate| candidate.event));
    (events, scanned_files)
}

#[cfg(test)]
pub(crate) fn report(range: ModelUsageRange, sources: Vec<crate::session_manager::SessionSource>,
    issues: Vec<crate::session_manager::SessionIssue>, now: DateTime<Local>) -> ModelUsageReport {
    let temporary = tempfile::tempdir().unwrap();
    reports(temporary.path(), sources, issues, now).unwrap().into_iter().find(|report| report.range == range).unwrap()
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

#[cfg(test)]
mod tests;
