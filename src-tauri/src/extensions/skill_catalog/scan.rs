use std::sync::{atomic::{AtomicI64, Ordering}, Arc};
use std::time::Instant;

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, IF_NONE_MATCH};

use super::progress::{CatalogScanMonitor, RepositoryState};
use super::recovery::{offer_recovery, RepositoryChange, RepositoryRecovery};
use super::SkillRepository;
use crate::extensions::sources::{
    http_fetch_with_progress, resolve_pinned_github_source, revalidate_github_commit, GitHubRequest,
    SkillCandidate, SourceError,
    MAX_SOURCE_BYTES, MAX_SOURCE_CANDIDATES,
};

static GITHUB_RETRY_AT: AtomicI64 = AtomicI64::new(0);

pub(super) fn github_retry_at() -> Option<i64> {
    let until = GITHUB_RETRY_AT.load(Ordering::Relaxed);
    (until > chrono::Utc::now().timestamp()).then_some(until)
}

#[derive(Debug)]
pub(crate) struct RepositoryScan {
    pub repository: SkillRepository,
    pub result: VerifiedRepository,
}

#[derive(Clone, Debug)]
pub(crate) struct VerifiedRepository {
    pub commit: String,
    pub candidates: Vec<Arc<SkillCandidate>>,
}

/// Each enabled repository is pinned independently. A failed source does not
/// discard other sources, and aggregate limits also bound a many-repo scan.
pub(crate) fn scan_repositories(
    repositories: &[SkillRepository],
    changes: &[RepositoryChange],
    deadline: Instant,
    monitor: &CatalogScanMonitor,
    cached: impl Fn(&SkillRepository) -> Option<VerifiedRepository>,
) -> Vec<RepositoryScan> {
    let mut result = Vec::new();
    let (mut count, mut bytes) = (0usize, 0u64);
    for (index, repo) in repositories.iter().filter(|repo| repo.enabled).enumerate() {
        monitor.set_state(index, RepositoryState::Running);
        let change = changes.iter().find(|change| change.original.id == repo.id);
        let source = change.map_or(repo, |change| &change.replacement);
        let fetch = |step, url: &str, headers: HeaderMap| {
            let result = http_fetch_with_progress(url, Some(deadline), headers, &|transfer| {
                if let Some(until) = transfer.github_retry_at {
                    GITHUB_RETRY_AT.fetch_max(until, Ordering::Relaxed);
                }
                monitor.update_request(index, step, transfer);
            });
            monitor.finish_request(index, step, result.is_ok());
            result
        };
        let scanned = if github_retry_at().is_some() {
            Err(SourceError::GithubRateLimited("GitHub API 限流，等待恢复后重试".into()))
        } else {
            scan_repository(source, cached(source), fetch)
        };
        let scanned = scanned
            .and_then(|verified| {
                let candidates = &verified.candidates;
                if change.is_some() && candidates.is_empty() {
                    return Err(SourceError::Rejected("调整后的仓库范围没有找到 Skill，已保留原配置".into()));
                }
                let source_bytes: u64 = candidates.iter().flat_map(|item| &item.entries)
                    .map(|entry| entry.size()).sum();
                if count + candidates.len() > MAX_SOURCE_CANDIDATES || bytes + source_bytes > MAX_SOURCE_BYTES {
                    return Err(SourceError::Rejected(format!(
                        "Catalog scan exceeds {MAX_SOURCE_CANDIDATES} candidates or {MAX_SOURCE_BYTES} bytes; disable repositories or narrow their subpaths"
                    )));
                }
                count += candidates.len();
                bytes += source_bytes;
                Ok(verified)
            });
        match scanned {
            Ok(verified) => {
                monitor.set_state(index, RepositoryState::Complete { candidate_count: verified.candidates.len() });
                result.push(RepositoryScan { repository: source.clone(), result: verified });
            }
            Err(error) => {
                if matches!(&error, SourceError::GithubRateLimited(_)) && github_retry_at().is_none() {
                    GITHUB_RETRY_AT.fetch_max(chrono::Utc::now().timestamp() + 60, Ordering::Relaxed);
                }
                let recovery = change.map(|change| change.recovery.clone())
                    .or_else(|| offer_recovery(repo, &error, repositories))
                    .or_else(|| (github_retry_at().is_some() && matches!(&error, SourceError::Http { status: 403, .. }))
                        .then_some(RepositoryRecovery::Retry));
                monitor.set_state(index, RepositoryState::Failed {
                    message: asb_core::adapter::scrub_message(error.to_string()),
                    recovery,
                });
            }
        }
    }
    result
}

fn scan_repository(
    repo: &SkillRepository,
    previous: Option<VerifiedRepository>,
    fetch: impl Fn(GitHubRequest, &str, HeaderMap) -> Result<(u16, Vec<u8>), String>,
) -> Result<VerifiedRepository, SourceError> {
    let ref_name = repo.ref_name.as_deref();
    let previous_sha = previous.as_ref().map(|snapshot| snapshot.commit.as_str());
    let commit = revalidate_github_commit(&repo.repo, ref_name.unwrap_or("HEAD"), previous_sha, &|url| {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github.sha"));
        if let Some(sha) = previous_sha {
            headers.insert(IF_NONE_MATCH, format!("\"{sha}\"").parse().expect("validated commit SHA"));
        }
        fetch(GitHubRequest::Commit, url, headers)
    })?;
    if let Some(previous) = previous.filter(|snapshot| snapshot.commit == commit) {
        return Ok(previous);
    }
    let candidates = resolve_pinned_github_source(&repo.repo, &repo.subpath, ref_name, &commit, |step, url| {
        fetch(step, url, HeaderMap::new())
    })?;
    Ok(VerifiedRepository {
        commit,
        candidates: candidates.into_iter().map(Arc::new).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "0123456789abcdef0123456789abcdef01234567";
    const NEW: &str = "89abcdef0123456789abcdef0123456789abcdef";

    fn repository() -> SkillRepository {
        SkillRepository {
            id: "test".into(), repo: "example/skills".into(), subpath: String::new(),
            ref_name: Some("main".into()), enabled: true,
        }
    }

    fn previous() -> VerifiedRepository {
        VerifiedRepository { commit: OLD.into(), candidates: Vec::new() }
    }

    #[test]
    fn unchanged_commit_skips_tree_and_archive() {
        for (status, body) in [(304, Vec::new()), (200, OLD.as_bytes().to_vec())] {
            let verified = scan_repository(&repository(), Some(previous()), |step, url, headers| {
                assert_eq!(step, GitHubRequest::Commit);
                assert!(url.ends_with("/example/skills/commits/main"));
                assert_eq!(headers[ACCEPT].to_str().unwrap(), "application/vnd.github.sha");
                assert_eq!(headers[IF_NONE_MATCH].to_str().unwrap(), format!("\"{OLD}\""));
                Ok((status, body.clone()))
            }).unwrap();
            assert_eq!(verified.commit, OLD);
        }
    }

    #[test]
    fn changed_commit_reads_the_tree_at_the_new_sha() {
        let verified = scan_repository(&repository(), Some(previous()), |step, url, headers| match step {
            GitHubRequest::Commit => Ok((200, NEW.as_bytes().to_vec())),
            GitHubRequest::Tree => {
                assert!(url.ends_with(&format!("/git/trees/{NEW}?recursive=1")));
                assert!(headers.is_empty());
                Ok((200, br#"{"truncated":false,"tree":[]}"#.to_vec()))
            }
            GitHubRequest::Archive => panic!("repository has no Skill manifest"),
        }).unwrap();
        assert_eq!(verified.commit, NEW);
    }

    #[test]
    fn network_errors_and_unexpected_not_modified_are_reported() {
        let failed = scan_repository(&repository(), Some(previous()), |_, _, _| Ok((404, Vec::new())));
        assert!(failed.unwrap_err().to_string().contains("HTTP 404"));
        let missing = scan_repository(&repository(), None, |_, _, headers| {
            assert!(!headers.contains_key(IF_NONE_MATCH));
            Ok((304, Vec::new()))
        });
        assert!(missing.is_err());
    }
}
