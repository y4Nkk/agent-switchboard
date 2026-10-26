use super::*;
use crate::resource_monitor::types::InterfaceCoverage;

fn entry(pid: u32, parent_pid: u32, name: &str, user: ProcessUser) -> Entry {
    Entry { pid, parent_pid, name: name.into(), user }
}

fn observation(entry: &Entry) -> Observation {
    Observation { key: ProcessKey { pid: entry.pid, birth: u128::from(entry.pid) }, parent_pid: entry.parent_pid,
        name: entry.name.clone(), executable: Some(format!("/fixture/{}", entry.name).into()),
        cpu_ns: Ok(0), resident_bytes: Ok(1024), at: Instant::now() }
}

fn scan(entries: &HashMap<u32, Entry>, observed: Vec<Observation>) -> Scan<'_> {
    Scan { reader: Reader::new().unwrap(), entries, roots: HashMap::new(),
        visited: observed.iter().map(|process| process.key.pid).collect(),
        observed: observed.into_iter().map(|process| (process.key.pid, process)).collect(),
        failures: HashMap::new(), issues: HashMap::new(), scope_issues: Vec::new(), boundaries: HashSet::new() }
}

fn interface() -> InterfaceProcesses {
    InterfaceProcesses { processes: Vec::new(), coverage: InterfaceCoverage::Partial, issues: Vec::new() }
}

#[test]
fn foreign_engines_and_unscoped_shared_helpers_do_not_pollute_client_status() {
    let entries: HashMap<_, _> = [entry(u32::MAX, 0, "codex.exe", ProcessUser::Other),
        entry(u32::MAX - 1, 0, "claude.exe", ProcessUser::Other),
        entry(u32::MAX - 2, 0, "codex-windows-sandbox-service.exe", ProcessUser::Unverified)]
        .into_iter().map(|entry| (entry.pid, entry)).collect();
    let mut scan = scan(&entries, Vec::new());
    scan.identify(&interface(), &HashMap::new());
    assert!(scan.roots.is_empty());
    assert!(scan.issues.is_empty());
    assert!(scan.scope_issues.is_empty());
    assert!(!scan.visited.contains(&u32::MAX));
    assert!(!scan.visited.contains(&(u32::MAX - 1)));
    assert!(!scan.visited.contains(&(u32::MAX - 2)));
}

#[test]
fn unknown_user_is_an_enumeration_limit_rather_than_a_client_access_error() {
    let entries: HashMap<_, _> = [entry(u32::MAX, 0, "codex.exe", ProcessUser::Unverified),
        entry(u32::MAX - 1, 0, "claude.exe", ProcessUser::Unverified)]
        .into_iter().map(|entry| (entry.pid, entry)).collect();
    let mut scan = scan(&entries, Vec::new());
    scan.identify(&interface(), &HashMap::new());
    assert_eq!(scan.scope_issues, [Issue::EnumerationRestricted]);
    assert!(scan.issues.is_empty());
    assert_eq!(scan.boundaries.len(), 2);
    assert!(!scan.visited.contains(&u32::MAX));
}

#[test]
fn verified_current_user_read_failures_remain_specific_to_the_client() {
    let entries = HashMap::from([(u32::MAX, entry(u32::MAX, 0, "claude.exe", ProcessUser::Current))]);
    let mut scan = scan(&entries, Vec::new());
    scan.visited.insert(u32::MAX);
    scan.failures.insert(u32::MAX, Issue::AccessDenied);
    scan.identify(&interface(), &HashMap::new());
    assert_eq!(scan.issues[&Group::Claude], [Issue::AccessDenied]);
    assert!(!scan.issues.contains_key(&Group::Codex));
    assert!(scan.scope_issues.is_empty());
}

#[test]
fn both_engines_keep_their_helpers_and_tasks_without_double_charging_switchboard() {
    let own_pid = std::process::id();
    let ids = [u32::MAX - 10, u32::MAX - 9, u32::MAX - 8, u32::MAX - 7, u32::MAX - 6];
    let entries: HashMap<_, _> = [entry(own_pid, 0, "switchboard", ProcessUser::Current),
        entry(ids[0], own_pid, "codex.exe", ProcessUser::Current),
        entry(ids[1], ids[0], "codex-code-mode-host.exe", ProcessUser::Current),
        entry(ids[2], ids[1], "build-task", ProcessUser::Current),
        entry(ids[3], own_pid, "claude.exe", ProcessUser::Current),
        entry(ids[4], ids[3], "claude-code-helper", ProcessUser::Current)]
        .into_iter().map(|entry| (entry.pid, entry)).collect();
    let observed: Vec<_> = entries.values().map(observation).collect();
    let previous = observed.iter().filter_map(|process| {
        let group = executables::named_group(&process.name)?;
        Some((process.key, TrackedProcess { owner: Owner { group, evidence: Evidence::Executable },
            executable: process.executable.clone(), cpu_ns: process.cpu_ns, at: process.at }))
    }).collect();
    let mut scan = scan(&entries, observed);
    scan.identify(&interface(), &previous);
    scan.descendants(&previous);
    for (pid, group, evidence) in [(own_pid, Group::Switchboard, Evidence::Application),
        (ids[0], Group::Codex, Evidence::Executable), (ids[1], Group::Codex, Evidence::Ancestor),
        (ids[2], Group::Codex, Evidence::Ancestor), (ids[3], Group::Claude, Evidence::Executable),
        (ids[4], Group::Claude, Evidence::Ancestor)] {
        let owner = ownership::resolve(&scan.observed[&pid], &scan.observed, &scan.roots, &scan.boundaries, &previous).unwrap();
        assert_eq!((owner.group, owner.evidence), (group, evidence));
    }
    assert!(scan.issues.is_empty());
    assert!(scan.boundaries.is_empty());
}

#[test]
fn coverage_errors_stop_at_an_excluded_or_unverified_engine_boundary() {
    let entries = HashMap::from([(2, entry(2, 1, "codex.exe", ProcessUser::Current)),
        (3, entry(3, 2, "task", ProcessUser::Current)), (4, entry(4, 1, "task", ProcessUser::Current)),
        (5, entry(5, 1, "foreign-task", ProcessUser::Other)), (6, entry(6, 5, "task", ProcessUser::Current))]);
    let roots = HashMap::from([(1, Owner { group: Group::Switchboard, evidence: Evidence::Application })]);
    let boundaries = HashSet::from([2]);
    assert_eq!(affected_group(2, &entries, &roots, &boundaries), None);
    assert_eq!(affected_group(3, &entries, &roots, &boundaries), None);
    assert_eq!(affected_group(4, &entries, &roots, &boundaries), Some(Group::Switchboard));
    assert_eq!(affected_group(5, &entries, &roots, &boundaries), None);
    assert_eq!(affected_group(6, &entries, &roots, &boundaries), None);
}
