use super::*;

fn observation(pid: u32, at: Instant, cpu: u64) -> Observation {
    Observation { key: ProcessKey { pid, birth: u128::from(pid) }, parent_pid: 0,
        name: format!("test-{pid}"), executable: None, cpu_ns: Ok(cpu), resident_bytes: Ok(4096), at }
}

#[test]
fn cpu_uses_actual_monotonic_interval_and_all_logical_processors() {
    let at = Instant::now();
    let previous = TrackedProcess { cpu_ns: Ok(1_000_000_000), at, executable: None,
        owner: Owner { group: Group::Codex, evidence: Evidence::Executable } };
    let current = observation(1, at + Duration::from_secs(2), 3_000_000_000);
    assert_eq!(cpu_percent(&current, Some(&previous), 4).unwrap(), 25.0);
    assert_eq!(cpu_percent(&current, None, 4), Err(Issue::WarmingUp));
    let zero = observation(1, at + Duration::from_secs(2), 1_000_000_000);
    assert_eq!(cpu_percent(&zero, Some(&previous), 4).unwrap(), 0.0);
}

#[test]
fn counter_reset_and_invalid_intervals_require_new_baselines() {
    let at = Instant::now();
    let previous = TrackedProcess { cpu_ns: Ok(100), at, executable: None,
        owner: Owner { group: Group::Codex, evidence: Evidence::Executable } };
    assert_eq!(cpu_percent(&observation(1, at + Duration::from_secs(2), 50), Some(&previous), 4), Err(Issue::CounterReset));
    assert_eq!(cpu_percent(&observation(1, at + Duration::from_secs(60), 100), Some(&previous), 4), Err(Issue::IntervalInterrupted));
    assert_eq!(cpu_percent(&observation(1, at, 100), Some(&previous), 4), Err(Issue::IntervalInterrupted));
}

#[test]
fn changing_software_owner_does_not_charge_the_previous_interval_to_the_new_group() {
    let at = Instant::now();
    let old = observation(1, at, 0);
    let mut sampler = Sampler::default();
    sampler.previous.insert(old.key, TrackedProcess { cpu_ns: old.cpu_ns, at,
        executable: None, owner: Owner { group: Group::Codex, evidence: Evidence::Executable } });
    let collection = Collection { processes: vec![OwnedProcess {
        observation: observation(1, at + Duration::from_secs(2), 2_000_000_000),
        owner: Owner { group: Group::Claude, evidence: Evidence::Executable } }],
        group_issues: HashMap::new(), online_cpus: 4, scope_issues: Vec::new(),
        interface: InterfaceProcesses { processes: Vec::new(), coverage: InterfaceCoverage::Verified, issues: Vec::new() },
        at: at + Duration::from_secs(2) };
    let snapshot = sampler.sample(collection, false, 1);
    let read = snapshot.read(&ReadRequest { reset_interval: false, details: None });
    let claude = read.groups.iter().find(|row| row.group == Group::Claude).unwrap();
    assert_eq!(claude.cpu.value, None);
    assert!(claude.cpu.issues.contains(&Issue::IntervalInterrupted));
    assert_eq!(claude.resident_bytes.value, Some(4096.0));
}

#[test]
fn sleep_or_wall_clock_changes_interrupt_the_interval() {
    let at = Instant::now();
    let wall = SystemTime::now();
    assert!(!clock_interrupted(Some((at, wall)), at + Duration::from_secs(2), wall + Duration::from_secs(2)));
    assert!(clock_interrupted(Some((at, wall)), at + Duration::from_secs(2), wall + Duration::from_secs(3600)));
    assert!(clock_interrupted(Some((at, wall)), at + Duration::from_secs(2), wall - Duration::from_secs(1)));
}

#[test]
fn warm_cpu_does_not_invalidate_measured_memory_or_count_and_details_share_one_sample() {
    let at = Instant::now();
    let processes = (1..=51).map(|pid| OwnedProcess { observation: observation(pid, at, 0),
        owner: Owner { group: Group::Codex, evidence: Evidence::Executable } }).collect();
    let collection = Collection { processes, group_issues: HashMap::new(), online_cpus: 4,
        scope_issues: Vec::new(), interface: InterfaceProcesses { processes: Vec::new(),
            coverage: InterfaceCoverage::Verified, issues: Vec::new() }, at };
    let snapshot = Sampler::default().sample(collection, true, 7);
    let request = ReadRequest { reset_interval: false, details: Some(DetailRequest { group: Group::Codex, page: 2 }) };
    let read = snapshot.read(&request);
    assert_eq!(read.sample_id, 7);
    let codex = read.groups.iter().find(|row| row.group == Group::Codex).unwrap();
    assert_eq!(codex.cpu.state, MetricState::Unavailable);
    assert_eq!(codex.resident_bytes.state, MetricState::Measured);
    assert_eq!(codex.process_count.value, Some(51.0));
    let details = read.details.unwrap();
    assert_eq!((details.page, details.page_size, details.total, details.rows.len()), (2, 50, 51, 1));
    assert_eq!(details.rows[0].pid, 51);
}
