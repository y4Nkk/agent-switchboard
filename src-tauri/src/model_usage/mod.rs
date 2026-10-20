//! Exact local call usage; context estimates never become consumption.
mod parse;
use asb_core::contracts::{AppKind, ModelUsageCoverage, ModelUsageDay, ModelUsageGroup,
    ModelUsageIssue, ModelUsageRange, ModelUsageReport, ModelUsageTokens};
use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Copy, Default)]
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

pub(crate) fn report(
    range: ModelUsageRange,
    mut sources: Vec<crate::session_manager::SessionSource>,
    source_issues: Vec<crate::session_manager::SessionIssue>,
    now: DateTime<Local>,
) -> ModelUsageReport {
    sources.retain(|source| source.converted.is_none());
    let mut aggregate = Accumulator::new(CalendarRange::new(range, now));
    sources.sort_by(|left, right| (left.meta.app, &left.meta.session_id).cmp(&(right.meta.app, &right.meta.session_id)));
    let mut issues: Vec<ModelUsageIssue> = source_issues.into_iter().filter(|issue| issue.conversion_id.is_none())
        .map(|issue| ModelUsageIssue { app: issue.app, message: issue.message }).collect();
    let mut codex = HashMap::new();
    let mut claude = HashMap::new();
    let mut coverage = ModelUsageCoverage { source_sessions: sources.len() as u64, ..Default::default() };
    for source in &sources {
        for path in &source.paths {
            coverage.scanned_files += 1;
            let scan = match source.meta.app {
                AppKind::Codex => crate::codex_metering::visit_token_usage(path, &source.meta.session_id, |event| {
                    if let Err(message) = crate::codex_metering::merge_token_event(&mut codex, event) {
                        issues.push(ModelUsageIssue { app: AppKind::Codex, message });
                    }
                    Ok(())
                }),
                AppKind::Claude => parse::collect_claude(path, &source.meta.session_id, &mut claude),
            };
            match scan {
                Ok(warnings) => issues.extend(warnings.into_iter().map(|message| ModelUsageIssue { app: source.meta.app, message })),
                Err(message) => issues.push(ModelUsageIssue { app: source.meta.app, message }),
            }
        }
    }
    for event in codex.into_values() {
        let tokens = event.tokens;
        let total = tokens.total().expect("validated receipt total");
        let result = aggregate.add(UsageEvent { app: AppKind::Codex, identity: event.identity, session: event.owner,
            model: event.model, at: event.at, tokens: TokenTotals {
                input: tokens.input - tokens.cached_input - tokens.cache_write_input,
                cache_read: tokens.cached_input, cache_creation: tokens.cache_write_input, output: tokens.output, total } });
        if let Err(message) = result { issues.push(ModelUsageIssue { app: AppKind::Codex, message }); }
    }
    for candidate in claude.into_values() {
        if let Err(message) = aggregate.add(candidate.event) { issues.push(ModelUsageIssue { app: AppKind::Claude, message }); }
    }
    coverage.sessions_without_usage = sources.iter()
        .filter(|source| !aggregate.measured.contains(&(source.meta.app, source.meta.session_id.clone()))).count() as u64;
    coverage.undated_event_tokens = aggregate.undated_event_tokens;
    finish(range, now, aggregate, coverage, issues)
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
