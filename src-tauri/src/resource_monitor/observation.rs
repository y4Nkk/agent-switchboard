use super::types::{Evidence, Group, InterfaceCoverage, Issue};
use std::{collections::HashMap, path::PathBuf, time::Instant};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProcessKey { pub pid: u32, pub birth: u128 }

impl ProcessKey {
    pub fn encoded(self) -> String { format!("{}:{:x}", self.pid, self.birth) }
}

#[derive(Clone, Debug)]
pub struct Entry { pub pid: u32, pub parent_pid: u32, pub name: String, pub user: ProcessUser }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessUser { Current, Other, Unverified }

#[derive(Clone, Debug)]
pub struct Observation {
    pub key: ProcessKey,
    pub parent_pid: u32,
    pub name: String,
    pub executable: Option<PathBuf>,
    pub cpu_ns: Result<u64, Issue>,
    pub resident_bytes: Result<u64, Issue>,
    pub at: Instant,
}

#[derive(Clone, Copy, Debug)]
pub struct Owner { pub group: Group, pub evidence: Evidence }

pub struct TrackedProcess {
    pub owner: Owner,
    pub executable: Option<PathBuf>,
    pub cpu_ns: Result<u64, Issue>,
    pub at: Instant,
}

pub struct OwnedProcess { pub observation: Observation, pub owner: Owner }

#[derive(Clone)]
pub struct InterfaceProcesses {
    pub processes: Vec<ProcessKey>,
    pub coverage: InterfaceCoverage,
    pub issues: Vec<Issue>,
}

pub struct Collection {
    pub processes: Vec<OwnedProcess>,
    pub group_issues: HashMap<Group, Vec<Issue>>,
    pub online_cpus: u32,
    pub scope_issues: Vec<Issue>,
    pub interface: InterfaceProcesses,
    pub at: Instant,
}
