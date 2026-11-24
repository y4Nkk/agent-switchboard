use std::time::Instant;

use super::progress::{CatalogScanMonitor, RepositoryState};
use super::SkillRepository;
use crate::extensions::sources::{
    http_fetch_with_progress, resolve_github_source_with_requests, SkillCandidate, SourceError,
    MAX_SOURCE_BYTES, MAX_SOURCE_CANDIDATES,
};

#[derive(Debug)]
pub struct RepositoryScan {
    pub repository: SkillRepository,
    pub candidates: Vec<SkillCandidate>,
}

/// Each enabled repository is pinned independently. A failed source does not
/// discard other sources, and aggregate limits also bound a many-repo scan.
pub(crate) fn scan_repositories(
    repositories: &[SkillRepository],
    deadline: Instant,
    monitor: &CatalogScanMonitor,
) -> Vec<RepositoryScan> {
    let mut result = Vec::new();
    let (mut count, mut bytes) = (0usize, 0u64);
    for (index, repo) in repositories.iter().enumerate() {
        monitor.set_state(index, RepositoryState::Running);
        let fetch = |step, url: &str| {
            let result = http_fetch_with_progress(url, Some(deadline), &|transfer| {
                monitor.update_request(index, step, transfer);
            });
            monitor.finish_request(index, step, result.is_ok());
            result
        };
        let scanned = resolve_github_source_with_requests(&repo.repo, &repo.subpath, repo.ref_name.as_deref(), fetch)
            .and_then(|candidates| {
                let source_bytes: u64 = candidates.iter().flat_map(|item| &item.entries)
                    .map(|entry| entry.size()).sum();
                if count + candidates.len() > MAX_SOURCE_CANDIDATES || bytes + source_bytes > MAX_SOURCE_BYTES {
                    return Err(SourceError::Rejected(format!(
                        "Catalog scan exceeds {MAX_SOURCE_CANDIDATES} candidates or {MAX_SOURCE_BYTES} bytes; disable repositories or narrow their subpaths"
                    )));
                }
                count += candidates.len();
                bytes += source_bytes;
                Ok(candidates)
            });
        match scanned {
            Ok(candidates) => {
                monitor.set_state(index, RepositoryState::Complete { candidate_count: candidates.len() });
                result.push(RepositoryScan { repository: repo.clone(), candidates });
            }
            Err(error) => monitor.set_state(index, RepositoryState::Failed {
                message: asb_core::adapter::scrub_message(error.to_string()),
            }),
        }
    }
    result
}
