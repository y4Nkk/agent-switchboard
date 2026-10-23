use super::{observation::{Observation, Owner, ProcessKey, TrackedProcess}, types::Evidence};
use std::collections::{HashMap, HashSet};

pub fn resolve(
    process: &Observation,
    observed: &HashMap<u32, Observation>,
    roots: &HashMap<u32, Owner>,
    boundaries: &HashSet<u32>,
    previous: &HashMap<ProcessKey, TrackedProcess>,
) -> Option<Owner> {
    let mut current = process;
    let mut visited = HashSet::new();
    while visited.insert(current.key.pid) {
        if let Some(owner) = roots.get(&current.key.pid) {
            return Some(Owner { group: owner.group, evidence: if current.key == process.key {
                owner.evidence
            } else { Evidence::Ancestor } });
        }
        if boundaries.contains(&current.key.pid) || boundaries.contains(&current.parent_pid) { return None; }
        let Some(parent) = observed.get(&current.parent_pid) else { break; };
        if parent.key.birth > current.key.birth { break; }
        current = parent;
    }
    retained(process, previous)
}

pub fn retained(process: &Observation, previous: &HashMap<ProcessKey, TrackedProcess>) -> Option<Owner> {
    previous.get(&process.key).filter(|previous| process.executable.is_some()
        && previous.executable == process.executable)
        .map(|previous| Owner { group: previous.owner.group, evidence: Evidence::Retained })
}
