use super::{observation::*, ownership::resolve as resolve_with_boundaries, types::*, ReadAction, ResourceMonitor};
use std::{collections::{HashMap, HashSet}, time::Instant};

fn resolve(process: &Observation, observed: &HashMap<u32, Observation>, roots: &HashMap<u32, Owner>,
    previous: &HashMap<ProcessKey, TrackedProcess>) -> Option<Owner> {
    resolve_with_boundaries(process, observed, roots, &HashSet::new(), previous)
}

fn process(pid: u32, parent_pid: u32, birth: u128) -> Observation {
    Observation { key: ProcessKey { pid, birth }, parent_pid, name: format!("process-{pid}"),
        executable: Some(format!("/fixture/process-{pid}").into()), cpu_ns: Ok(0), resident_bytes: Ok(1024), at: Instant::now() }
}

fn owner(group: Group) -> Owner { Owner { group, evidence: Evidence::Executable } }

fn tracked(process: &Observation, group: Group) -> TrackedProcess {
    TrackedProcess { owner: owner(group), executable: process.executable.clone(), cpu_ns: process.cpu_ns, at: process.at }
}

#[test]
fn nested_coding_engine_shadows_switchboard_and_owns_its_tasks() {
    let processes = HashMap::from([(1, process(1, 0, 1)), (2, process(2, 1, 2)), (3, process(3, 2, 3))]);
    let roots = HashMap::from([(1, owner(Group::Switchboard)), (2, owner(Group::Codex))]);
    let previous = HashMap::new();
    assert_eq!(resolve(&processes[&1], &processes, &roots, &previous).unwrap().group, Group::Switchboard);
    assert_eq!(resolve(&processes[&2], &processes, &roots, &previous).unwrap().group, Group::Codex);
    assert_eq!(resolve(&processes[&3], &processes, &roots, &previous).unwrap().group, Group::Codex);
}

#[test]
fn unverified_coding_root_is_not_charged_to_its_switchboard_parent() {
    let processes = HashMap::from([(1, process(1, 0, 1)), (2, process(2, 1, 2)), (3, process(3, 2, 3))]);
    let roots = HashMap::from([(1, owner(Group::Switchboard))]);
    let boundaries = HashSet::from([2]);
    assert!(resolve_with_boundaries(&processes[&2], &processes, &roots, &boundaries, &HashMap::new()).is_none());
    assert!(resolve_with_boundaries(&processes[&3], &processes, &roots, &boundaries, &HashMap::new()).is_none());
}

#[test]
fn reused_parent_pid_cannot_capture_an_older_child() {
    let processes = HashMap::from([(2, process(2, 0, 20)), (3, process(3, 2, 5))]);
    let roots = HashMap::from([(2, owner(Group::Claude))]);
    assert!(resolve(&processes[&3], &processes, &roots, &HashMap::new()).is_none());
    let previous = HashMap::from([(processes[&3].key, tracked(&processes[&3], Group::Codex))]);
    let retained = resolve(&processes[&3], &processes, &roots, &previous).unwrap();
    assert_eq!((retained.group, retained.evidence), (Group::Codex, Evidence::Retained));
}

#[test]
fn retained_ownership_requires_the_same_birth_identity() {
    let old = process(3, 2, 3);
    let previous = HashMap::from([(old.key, tracked(&old, Group::Codex))]);
    let new = process(3, 0, 30);
    assert!(resolve(&new, &HashMap::new(), &HashMap::new(), &previous).is_none());
    assert_eq!(resolve(&old, &HashMap::new(), &HashMap::new(), &previous).unwrap().evidence, Evidence::Retained);
    let mut replaced = old.clone();
    replaced.executable = Some("/fixture/different-image".into());
    assert!(resolve(&replaced, &HashMap::new(), &HashMap::new(), &previous).is_none());
}

#[test]
fn unrooted_cycles_and_broken_parent_chains_are_not_attributed() {
    let processes = HashMap::from([(2, process(2, 3, 1)), (3, process(3, 2, 1))]);
    assert!(resolve(&processes[&2], &processes, &HashMap::new(), &HashMap::new()).is_none());
    assert!(resolve(&process(4, 999, 4), &processes, &HashMap::new(), &HashMap::new()).is_none());
}

#[test]
fn unknown_metrics_never_become_a_successful_zero() {
    let unavailable = Metric::aggregate([Err(Issue::AccessDenied)], &[]);
    assert_eq!(unavailable.state, MetricState::Unavailable);
    assert_eq!(unavailable.value, None);
    let partial = Metric::aggregate([Ok(0.0), Err(Issue::ProcessExited)], &[]);
    assert_eq!(partial.state, MetricState::Partial);
    assert_eq!(partial.value, Some(0.0));
    assert_eq!((partial.measured_processes, partial.missing_processes), (1, 1));
}

#[test]
fn wire_contract_rejects_unknown_fields_and_unsupported_groups() {
    assert!(serde_json::from_str::<ReadRequest>(r#"{"resetInterval":false,"details":null,"interval":1}"#).is_err());
    assert!(serde_json::from_str::<ReadRequest>(r#"{"resetInterval":false,"details":{"group":"node","page":1}}"#).is_err());
    assert!(serde_json::from_str::<ReadRequest>(r#"{"resetInterval":false,"details":{"group":"codex","page":1,"limit":1000}}"#).is_err());
    assert_eq!(serde_json::to_value(Metric::unavailable(Issue::WarmingUp)).unwrap()["value"], serde_json::Value::Null);
}

#[test]
fn overlapping_readers_share_one_inflight_sampling_slot() {
    let monitor = ResourceMonitor::default();
    let ReadAction::Start { receiver, sender, id, .. } = monitor.begin(false).unwrap() else { panic!("expected first sampling task"); };
    assert_eq!(id, 1);
    assert!(matches!(monitor.begin(false).unwrap(), ReadAction::Waiting(_)));
    assert!(matches!(monitor.begin(true).unwrap(), ReadAction::Waiting(_)));
    assert_eq!(monitor.state.lock().unwrap().sequence, 1);
    drop((receiver, sender));
}

#[test]
fn native_reader_measures_the_current_process_with_a_stable_identity() {
    let reader = super::platform::Reader::new().unwrap();
    assert!(reader.online_cpus().unwrap() > 0);
    let (entries, _) = reader.entries().unwrap();
    let entry = entries.iter().find(|entry| entry.pid == std::process::id()).unwrap();
    assert_eq!(entry.user, ProcessUser::Current);
    let first = reader.read(entry).unwrap().unwrap();
    let second = reader.read(entry).unwrap().unwrap();
    assert_eq!(first.key.pid, std::process::id());
    assert!(first.key.birth > 0);
    assert_eq!(first.key, second.key);
    assert_eq!(first.parent_pid, entry.parent_pid);
    assert_eq!(std::fs::canonicalize(first.executable.unwrap()).unwrap(),
        std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap());
    assert!(first.resident_bytes.unwrap() > 0);
    assert!(second.resident_bytes.unwrap() > 0);
    assert!(second.cpu_ns.unwrap() >= first.cpu_ns.unwrap());
}
