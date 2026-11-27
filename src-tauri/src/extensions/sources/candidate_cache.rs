use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, LazyLock, Mutex};

use asb_switch::sha256_hex;
use crate::extensions::skill_catalog::{RepositoryScan, SkillRepository, VerifiedRepository, MAX_REPOSITORIES};
use super::{SkillCandidate, MAX_SOURCE_BYTES, MAX_SOURCE_CANDIDATES};

const MAX_CACHED_BYTES: u64 = 2 * MAX_SOURCE_BYTES;
const MAX_CACHED_CANDIDATES: usize = 2 * MAX_SOURCE_CANDIDATES;
const MAX_CACHED_REPOSITORIES: usize = 2 * MAX_REPOSITORIES;

static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(|| Mutex::new(Cache::default()));

struct CachedCandidate {
    candidate: Arc<SkillCandidate>,
    bytes: u64,
}

struct CachedRepository {
    source: SkillRepository,
    commit: String,
    candidate_ids: Vec<String>,
}

#[derive(Default)]
struct Cache {
    candidates: BTreeMap<String, CachedCandidate>,
    publication_order: VecDeque<String>,
    repositories: VecDeque<CachedRepository>,
    bytes: u64,
}

impl Cache {
    fn publish(&mut self, candidates: impl IntoIterator<Item = (String, Arc<SkillCandidate>)>) {
        let mut published = BTreeSet::new();
        for (id, candidate) in candidates {
            published.insert(id.clone());
            self.candidates.entry(id).or_insert_with(|| {
                let bytes = candidate.entries.iter().map(|entry| entry.size()).sum();
                self.bytes += bytes;
                CachedCandidate { candidate, bytes }
            });
        }
        self.publication_order.retain(|id| !published.contains(id));
        self.publication_order.extend(published);
        while self.bytes > MAX_CACHED_BYTES || self.candidates.len() > MAX_CACHED_CANDIDATES {
            let id = self.publication_order.pop_front().expect("cached candidate order");
            self.bytes -= self.candidates.remove(&id).expect("ordered cached candidate").bytes;
        }
        self.repositories.retain(|repo| repo.candidate_ids.iter().all(|id| self.candidates.contains_key(id)));
    }
}

fn same_source(a: &SkillRepository, b: &SkillRepository) -> bool {
    a.repo == b.repo && a.subpath == b.subpath && a.ref_name == b.ref_name
}

pub(crate) fn candidate_id(candidate: &SkillCandidate) -> String {
    let key = serde_json::to_string(&(candidate.source_ref(), &candidate.content_digest))
        .expect("serializable Skill candidate identity");
    sha256_hex(&key)
}

pub(crate) fn cache_candidates(list: Vec<SkillCandidate>) {
    CACHE.lock().expect("candidates")
        .publish(list.into_iter().map(|candidate| (candidate_id(&candidate), Arc::new(candidate))));
}

pub(crate) fn cached_candidate(id: &str) -> Option<SkillCandidate> {
    let candidate = Arc::clone(&CACHE.lock().expect("candidates").candidates.get(id)?.candidate);
    Some((*candidate).clone())
}

pub(crate) fn cached_repository(source: &SkillRepository) -> Option<VerifiedRepository> {
    let cache = CACHE.lock().expect("candidates");
    let repository = cache.repositories.iter().find(|repo| same_source(&repo.source, source))?;
    let candidates = repository.candidate_ids.iter()
        .map(|id| cache.candidates.get(id).map(|value| Arc::clone(&value.candidate)))
        .collect::<Option<Vec<_>>>()?;
    Some(VerifiedRepository { commit: repository.commit.clone(), candidates })
}

pub(crate) fn cache_catalog_repositories(repositories: &[RepositoryScan]) {
    let mut candidates = Vec::new();
    let snapshots = repositories.iter().map(|scan| {
        let candidate_ids = scan.result.candidates.iter().map(|candidate| {
            let id = candidate_id(candidate);
            candidates.push((id.clone(), Arc::clone(candidate)));
            id
        }).collect();
        CachedRepository { source: scan.repository.clone(), commit: scan.result.commit.clone(), candidate_ids }
    }).collect::<Vec<_>>();
    let mut cache = CACHE.lock().expect("candidates");
    // Publish the full accepted scan together so one repository cannot evict a peer in this result.
    cache.publish(candidates);
    for snapshot in snapshots {
        cache.repositories.retain(|existing| !same_source(&existing.source, &snapshot.source));
        cache.repositories.push_back(snapshot);
    }
    while cache.repositories.len() > MAX_CACHED_REPOSITORIES {
        cache.repositories.pop_front();
    }
}
