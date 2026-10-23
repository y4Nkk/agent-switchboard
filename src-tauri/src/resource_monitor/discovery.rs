use super::{executables::{self, Executables}, observation::*, ownership, platform::Reader,
    types::{push_issue, Evidence, Group, Issue}};
use std::{collections::{HashMap, HashSet, VecDeque}, time::Instant};

pub fn collect(interface: InterfaceProcesses, previous: &HashMap<ProcessKey, TrackedProcess>) -> Result<Collection, String> {
    let reader = Reader::new()?;
    let cpus = reader.online_cpus()?;
    let (entries, scope_issues) = reader.entries()?;
    let entries: HashMap<u32, Entry> = entries.into_iter().map(|entry| (entry.pid, entry)).collect();
    let mut scan = Scan { reader, entries: &entries, observed: HashMap::new(), roots: HashMap::new(),
        failures: HashMap::new(), issues: HashMap::new(), visited: HashSet::new(), boundaries: HashSet::new() };
    scan.identify(&interface, previous);
    scan.descendants(previous);
    let mut processes = Vec::new();
    for observation in scan.observed.values() {
        if let Some(owner) = ownership::resolve(observation, &scan.observed, &scan.roots, &scan.boundaries, previous) {
            processes.push(OwnedProcess { observation: observation.clone(), owner });
        } else if let Some(group) = affected_group(observation.key.pid, &entries, &scan.roots) {
            push_issue(scan.issues.entry(group).or_default(), Issue::ParentUnverified);
        }
    }
    for (&pid, &issue) in &scan.failures {
        if let Some(group) = affected_group(pid, &entries, &scan.roots) {
            push_issue(scan.issues.entry(group).or_default(), issue);
        }
    }
    if scan.reader.online_cpus()? != cpus {
        for group in Group::ALL { push_issue(scan.issues.entry(group).or_default(), Issue::CpuTopologyChanged); }
    }
    for group in Group::ALL {
        if scope_issues.contains(&Issue::EnumerationRestricted) {
            push_issue(scan.issues.entry(group).or_default(), Issue::EnumerationRestricted);
        }
    }
    for &issue in &interface.issues { push_issue(scan.issues.entry(Group::Switchboard).or_default(), issue); }
    if !scan.roots.contains_key(&std::process::id()) {
        push_issue(scan.issues.entry(Group::Switchboard).or_default(), Issue::IdentityUnavailable);
    }
    Ok(Collection { processes, group_issues: scan.issues, online_cpus: cpus,
        scope_issues, interface, at: Instant::now() })
}

struct Scan<'a> {
    reader: Reader,
    entries: &'a HashMap<u32, Entry>,
    observed: HashMap<u32, Observation>,
    roots: HashMap<u32, Owner>,
    failures: HashMap<u32, Issue>,
    issues: HashMap<Group, Vec<Issue>>,
    visited: HashSet<u32>,
    boundaries: HashSet<u32>,
}

impl Scan<'_> {
    fn inspect(&mut self, pid: u32) {
        if !self.visited.insert(pid) { return; }
        let Some(entry) = self.entries.get(&pid) else { self.failures.insert(pid, Issue::ProcessExited); return; };
        match self.reader.read(entry) {
            Ok(Some(process)) => { self.observed.insert(pid, process); }
            Ok(None) => {}
            Err(issue) => { self.failures.insert(pid, issue); }
        }
    }

    fn identify(&mut self, interface: &InterfaceProcesses, previous: &HashMap<ProcessKey, TrackedProcess>) {
        let mut catalog: Option<Executables> = None;
        let own_pid = std::process::id();
        let candidates: Vec<u32> = self.entries.values().filter(|entry| executables::candidate(&entry.name).is_some())
            .map(|entry| entry.pid).collect();
        for pid in candidates {
            self.inspect(pid);
            if let Some(observation) = self.observed.get(&pid) {
                if executables::outside_scope(observation) { self.boundaries.insert(pid); continue; }
                let retained_root = previous.get(&observation.key).filter(|previous|
                    previous.owner.evidence == Evidence::Executable && observation.executable.is_some()
                    && previous.executable == observation.executable).map(|previous| previous.owner.group);
                let classification = retained_root.map(|group| Ok(Some(group))).unwrap_or_else(||
                    catalog.get_or_insert_with(Executables::new).classify(observation));
                match classification {
                    Ok(Some(group)) => { self.roots.insert(pid, Owner { group, evidence: Evidence::Executable }); }
                    Ok(None) => {}
                    Err(issue) => {
                        self.boundaries.insert(pid);
                        if let Some(group) = executables::candidate(&observation.name) {
                            push_issue(self.issues.entry(group).or_default(), issue);
                        }
                    }
                }
            } else if let Some(&issue) = self.failures.get(&pid) {
                self.boundaries.insert(pid);
                if let Some(group) = self.entries.get(&pid).and_then(|entry| executables::candidate(&entry.name)) {
                    push_issue(self.issues.entry(group).or_default(), issue);
                }
            }
        }
        self.inspect(own_pid);
        if self.observed.contains_key(&own_pid) {
            self.roots.insert(own_pid, Owner { group: Group::Switchboard, evidence: Evidence::Application });
        }
        for key in &interface.processes {
            self.inspect(key.pid);
            let verified = self.observed.get(&key.pid).is_some_and(|process| process.key == *key
                && process.executable.as_ref().and_then(|path| path.file_name()).is_some_and(|name|
                    name.to_string_lossy().eq_ignore_ascii_case("msedgewebview2.exe")));
            if verified {
                self.roots.entry(key.pid).or_insert(Owner { group: Group::Switchboard, evidence: Evidence::Webview });
            } else { push_issue(self.issues.entry(Group::Switchboard).or_default(), Issue::IdentityChanged); }
        }
        for key in previous.keys() { self.inspect(key.pid); }
    }

    fn descendants(&mut self, previous: &HashMap<ProcessKey, TrackedProcess>) {
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for entry in self.entries.values() { children.entry(entry.parent_pid).or_default().push(entry.pid); }
        let retained = self.observed.values().filter(|process| ownership::retained(process, previous).is_some())
            .map(|process| process.key.pid);
        let mut queue: VecDeque<u32> = self.roots.keys().copied().chain(retained).collect();
        let mut expanded = HashSet::new();
        while let Some(pid) = queue.pop_front() {
            if !expanded.insert(pid) { continue; }
            for &child in children.get(&pid).into_iter().flatten() {
                self.inspect(child);
                if self.observed.contains_key(&child) { queue.push_back(child); }
            }
        }
    }
}

fn affected_group(pid: u32, entries: &HashMap<u32, Entry>, roots: &HashMap<u32, Owner>) -> Option<Group> {
    let mut current = pid;
    let mut visited = HashSet::new();
    while visited.insert(current) {
        if let Some(owner) = roots.get(&current) { return Some(owner.group); }
        current = entries.get(&current)?.parent_pid;
    }
    None
}
