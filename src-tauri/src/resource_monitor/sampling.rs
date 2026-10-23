use super::{observation::*, types::*};
use std::{collections::HashMap, time::{Duration, Instant, SystemTime}};

#[derive(Default)]
pub struct Sampler {
    pub previous: HashMap<ProcessKey, TrackedProcess>,
    online_cpus: u32,
    last_clock: Option<(Instant, SystemTime)>,
}

pub struct Snapshot {
    pub at: Instant,
    sample_id: u64,
    sampled_at: String,
    scope: Scope,
    groups: Vec<GroupReading>,
    processes: HashMap<Group, Vec<ProcessReading>>,
}

impl Sampler {
    pub fn sample(&mut self, collection: Collection, reset: bool, sample_id: u64) -> Snapshot {
        let wall = SystemTime::now();
        let interrupted = clock_interrupted(self.last_clock, collection.at, wall);
        let topology_changed = self.online_cpus != 0 && self.online_cpus != collection.online_cpus;
        let mut processes: HashMap<Group, Vec<ProcessReading>> = HashMap::new();
        let mut baselines = HashMap::new();
        for process in collection.processes {
            let observation = process.observation;
            let cpu = if reset { Err(Issue::WarmingUp) } else if interrupted { Err(Issue::IntervalInterrupted) }
                else if topology_changed || collection.group_issues.get(&process.owner.group)
                    .is_some_and(|issues| issues.contains(&Issue::CpuTopologyChanged)) { Err(Issue::CpuTopologyChanged) }
                else if self.previous.get(&observation.key).is_some_and(|previous| previous.owner.group != process.owner.group) {
                    Err(Issue::IntervalInterrupted)
                } else { cpu_percent(&observation, self.previous.get(&observation.key), collection.online_cpus) };
            processes.entry(process.owner.group).or_default().push(process_reading(&observation, process.owner, cpu));
            baselines.insert(observation.key, TrackedProcess { owner: process.owner,
                executable: observation.executable, cpu_ns: observation.cpu_ns, at: observation.at });
        }
        for rows in processes.values_mut() { rows.sort_by_key(|process| process.pid); }
        self.previous = baselines;
        self.online_cpus = collection.online_cpus;
        self.last_clock = Some((collection.at, wall));
        let groups = Group::ALL.into_iter().map(|group| {
            let issues = collection.group_issues.get(&group).cloned().unwrap_or_default();
            group_reading(group, processes.get(&group).map(Vec::as_slice).unwrap_or(&[]), issues)
        }).collect();
        let mut scope_issues = collection.scope_issues;
        if cfg!(target_os = "linux") { push_issue(&mut scope_issues, Issue::ApproximateRss); }
        Snapshot { at: collection.at, sample_id,
            sampled_at: chrono::Utc::now().to_rfc3339(),
            scope: Scope { platform: std::env::consts::OS.to_string(),
                interface_coverage: collection.interface.coverage, issues: scope_issues }, groups, processes }
    }
}

fn cpu_percent(process: &Observation, previous: Option<&TrackedProcess>, cpus: u32) -> Result<f64, Issue> {
    let now = process.cpu_ns?;
    let previous = previous.ok_or(Issue::WarmingUp)?;
    let before = previous.cpu_ns.map_err(|_| Issue::WarmingUp)?;
    let elapsed = process.at.checked_duration_since(previous.at).ok_or(Issue::IntervalInterrupted)?;
    if elapsed < Duration::from_millis(200) || elapsed > Duration::from_secs(5) || cpus == 0 {
        return Err(Issue::IntervalInterrupted);
    }
    let delta = now.checked_sub(before).ok_or(Issue::CounterReset)?;
    let value = delta as f64 / elapsed.as_nanos() as f64 / f64::from(cpus) * 100.0;
    if !value.is_finite() || value > 101.0 { return Err(Issue::CounterReset); }
    Ok(value.min(100.0))
}

fn clock_interrupted(previous: Option<(Instant, SystemTime)>, at: Instant, wall: SystemTime) -> bool {
    let Some((old_at, old_wall)) = previous else { return false; };
    let (Some(monotonic), Ok(wall)) = (at.checked_duration_since(old_at), wall.duration_since(old_wall)) else { return true; };
    monotonic.abs_diff(wall) > Duration::from_millis(500)
}

fn process_reading(process: &Observation, owner: Owner, cpu: Result<f64, Issue>) -> ProcessReading {
    let role = match owner.evidence {
        Evidence::Application => Role::Application, Evidence::Executable => Role::Engine,
        Evidence::Webview => Role::Interface, Evidence::Ancestor | Evidence::Retained => Role::Task,
    };
    let memory = process.resident_bytes.and_then(|bytes| {
        if bytes > 9_007_199_254_740_991 { Err(Issue::CounterReset) } else { Ok(bytes as f64) }
    });
    ProcessReading { key: process.key.encoded(), pid: process.key.pid,
        name: process.name.chars().take(256).collect(), role, evidence: owner.evidence,
        cpu: Metric::aggregate([cpu], &[]), resident_bytes: Metric::aggregate([memory], &[]) }
}

fn group_reading(group: Group, rows: &[ProcessReading], issues: Vec<Issue>) -> GroupReading {
    let coverage: Vec<Issue> = issues.into_iter().filter(|issue| *issue != Issue::CpuTopologyChanged).collect();
    let presence = if !rows.is_empty() { Presence::Running }
        else if coverage.is_empty() { Presence::NotObserved } else { Presence::Indeterminate };
    let mut cpu = aggregate_readings(rows.iter().map(|row| &row.cpu), &coverage);
    if let Some(value) = cpu.value {
        // Kernel scheduler ticks may round a fully busy interval slightly above 100%.
        if value > 101.0 { cpu = Metric::unavailable(Issue::CounterReset); }
        else { cpu.value = Some(value.min(100.0)); }
    }
    let resident_bytes = aggregate_readings(rows.iter().map(|row| &row.resident_bytes), &coverage);
    let process_count = if rows.is_empty() && coverage.is_empty() {
        Metric { state: MetricState::Measured, value: Some(0.0), measured_processes: 0, missing_processes: 0, issues: Vec::new() }
    } else { Metric::aggregate(rows.iter().map(|_| Ok(1.0)), &coverage) };
    GroupReading { group, presence, cpu, resident_bytes, process_count, issues: coverage }
}

fn aggregate_readings<'a>(readings: impl Iterator<Item = &'a Metric>, coverage: &[Issue]) -> Metric {
    let readings: Vec<&Metric> = readings.collect();
    let mut result = Metric::aggregate(readings.iter().map(|reading| reading.value
        .ok_or_else(|| reading.issues.first().copied().unwrap_or(Issue::CollectionFailed))), coverage);
    for reading in readings {
        for &issue in &reading.issues { push_issue(&mut result.issues, issue); }
    }
    result
}

impl Snapshot {
    pub fn read(&self, request: &ReadRequest) -> ResourceRead {
        let mut groups = self.groups.clone();
        if request.reset_interval {
            for group in &mut groups {
                if group.presence == Presence::Running { group.cpu = Metric::unavailable(Issue::WarmingUp); }
            }
        }
        let details = request.details.as_ref().map(|detail| {
            let rows = self.processes.get(&detail.group).map(Vec::as_slice).unwrap_or(&[]);
            let page = (detail.page as usize).min(rows.len().div_ceil(PAGE_SIZE).max(1));
            let start = (page - 1) * PAGE_SIZE;
            let mut selected: Vec<ProcessReading> = rows.iter().skip(start).take(PAGE_SIZE).cloned().collect();
            if request.reset_interval {
                for process in &mut selected { process.cpu = Metric::unavailable(Issue::WarmingUp); }
            }
            Details { group: detail.group, page: page as u32, page_size: PAGE_SIZE, total: rows.len(), rows: selected }
        });
        ResourceRead { sample_id: self.sample_id, sampled_at: self.sampled_at.clone(),
            next_read_in_ms: SAMPLE_INTERVAL_MS.saturating_sub(self.at.elapsed().as_millis() as u64),
            scope: self.scope.clone(), groups, details }
    }
}

#[cfg(test)]
mod tests;
