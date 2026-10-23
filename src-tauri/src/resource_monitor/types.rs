use serde::{Deserialize, Serialize};

pub const PAGE_SIZE: usize = 50;
pub const SAMPLE_INTERVAL_MS: u64 = 2_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Group { Switchboard, Codex, Claude }

impl Group {
    pub const ALL: [Self; 3] = [Self::Switchboard, Self::Codex, Self::Claude];
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadRequest {
    pub reset_interval: bool,
    pub details: Option<DetailRequest>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetailRequest { pub group: Group, pub page: u32 }

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Issue {
    WarmingUp, AccessDenied, ProcessExited, IdentityUnavailable, IdentityChanged,
    CounterReset, IntervalInterrupted, CpuTopologyChanged, CollectionFailed,
    UnrecognizedExecutable, WebviewUnavailable, WebviewPartial, BrowserExcluded,
    EnumerationRestricted, NamespaceBoundary, ApproximateRss, ParentUnverified,
    NotObserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MetricState { Measured, Partial, Unavailable }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metric {
    pub state: MetricState,
    pub value: Option<f64>,
    pub measured_processes: usize,
    pub missing_processes: usize,
    pub issues: Vec<Issue>,
}

impl Metric {
    pub fn unavailable(issue: Issue) -> Self {
        Self { state: MetricState::Unavailable, value: None, measured_processes: 0,
            missing_processes: 1, issues: vec![issue] }
    }

    pub fn aggregate(values: impl IntoIterator<Item = Result<f64, Issue>>, coverage: &[Issue]) -> Self {
        let mut result = Self { state: MetricState::Measured, value: Some(0.0),
            measured_processes: 0, missing_processes: 0, issues: coverage.to_vec() };
        for value in values {
            match value {
                Ok(value) if value.is_finite() && value >= 0.0 => {
                    result.measured_processes += 1;
                    *result.value.as_mut().unwrap() += value;
                }
                other => {
                    result.missing_processes += 1;
                    push_issue(&mut result.issues, other.err().unwrap_or(Issue::CounterReset));
                }
            }
        }
        if result.measured_processes == 0 {
            result.state = MetricState::Unavailable;
            result.value = None;
            if result.issues.is_empty() { result.issues.push(Issue::NotObserved); }
        } else if result.missing_processes > 0 || !coverage.is_empty() {
            result.state = MetricState::Partial;
        }
        result
    }
}

pub fn push_issue(issues: &mut Vec<Issue>, issue: Issue) {
    if !issues.contains(&issue) { issues.push(issue); }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Presence { Running, NotObserved, Indeterminate }

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Evidence { Application, Executable, Ancestor, Webview, Retained }

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Role { Application, Engine, Interface, Task }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupReading {
    pub group: Group,
    pub presence: Presence,
    pub cpu: Metric,
    pub resident_bytes: Metric,
    pub process_count: Metric,
    pub issues: Vec<Issue>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessReading {
    pub key: String,
    pub pid: u32,
    pub name: String,
    pub role: Role,
    pub evidence: Evidence,
    pub cpu: Metric,
    pub resident_bytes: Metric,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InterfaceCoverage { Verified, Partial, BrowserExcluded }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    pub platform: String,
    pub interface_coverage: InterfaceCoverage,
    pub issues: Vec<Issue>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    pub group: Group,
    pub page: u32,
    pub page_size: usize,
    pub total: usize,
    pub rows: Vec<ProcessReading>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRead {
    pub sample_id: u64,
    pub sampled_at: String,
    pub next_read_in_ms: u64,
    pub scope: Scope,
    pub groups: Vec<GroupReading>,
    pub details: Option<Details>,
}
