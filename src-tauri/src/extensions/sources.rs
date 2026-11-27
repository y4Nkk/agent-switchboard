//! Explicit source reads produce immutable candidates; this module never
//! installs files in a client or executes anything found in a repository.

mod archive;
mod candidate_cache;
mod github;
mod local;
mod transport;
mod zip;
mod zip_header;

use asb_core::extensions::contracts::SourceRef;
use asb_core::extensions::skill::{
    content_digest, extract_manifest, ContentEntry, ManifestExtraction,
};
use asb_core::extensions::validate::{validate_content_paths, validate_skill_name};

pub(crate) use candidate_cache::{cache_candidates, cache_catalog_repositories, cached_candidate,
    cached_repository, candidate_id};
pub use github::{fetch_github_subtree, resolve_github_commit, resolve_github_source};
pub(crate) use github::{
    repository as normalize_github_repository, resolve_pinned_github_source,
    revalidate_github_commit,
    validate_ref_name as validate_source_ref, validate_subpath as validate_source_subpath,
    GitHubRequest,
};
pub use local::scan_local_source;
pub use transport::http_fetch;
pub(crate) use transport::{
    fetch_bytes as fetch_source_bytes, http_fetch_with_progress, HttpTransfer, MAX_DIRECTORY_DOWNLOAD,
};
pub use zip::scan_zip_source;

pub(super) const MAX_SOURCE_ENTRIES: usize = 20_000;
pub(super) const MAX_SOURCE_BYTES: u64 = 128 * 1024 * 1024;
pub(super) const MAX_SOURCE_CANDIDATES: usize = 256;

#[derive(Debug, Clone, PartialEq)]
pub struct SkillCandidate {
    pub source_identity: String,
    pub subpath: String,
    pub name: String,
    pub description: Option<String>,
    pub ref_name: Option<String>,
    pub resolved_commit: Option<String>,
    pub content_digest: String,
    pub entries: Vec<ContentEntry>,
    pub diagnostics: Vec<String>,
}

impl SkillCandidate {
    pub fn source_ref(&self) -> SourceRef {
        SourceRef {
            source_id: self.source_identity.clone(),
            subpath: self.subpath.clone(),
            ref_name: self.ref_name.clone(),
            resolved_commit: self.resolved_commit.clone(),
        }
    }

    pub fn matches_source(&self, source: &SourceRef) -> bool {
        source.source_id == self.source_identity
            && source.subpath == self.subpath
            && source.ref_name == self.ref_name
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceError {
    #[error("来源不可达：{0}")]
    Unreachable(String),
    #[error("来源不可达：{0}")]
    Transport(String),
    #[error("来源不可达：{message}")]
    Http { status: u16, message: String },
    #[error("{0}")]
    GithubRateLimited(String),
    #[error("来源内容被拒绝：{0}")]
    Rejected(String),
    #[error("来源内容被拒绝：来源超过 {} 个 Skill；请指定子目录", MAX_SOURCE_CANDIDATES)]
    TooManySkills,
}

impl SourceError {
    pub(crate) fn is_rejected(&self) -> bool {
        matches!(self, Self::Rejected(_) | Self::TooManySkills)
    }
}

/// Tests inject the full HTTP boundary and never request a live repository.
pub type HttpFetch<'a> = &'a dyn Fn(&str) -> Result<(u16, Vec<u8>), String>;

fn validate_entries(entries: &[ContentEntry]) -> Result<(), SourceError> {
    let paths: Vec<_> = entries
        .iter()
        .map(|entry| (entry.relative_path.clone(), entry.kind, entry.size()))
        .collect();
    validate_content_paths(&paths).map_err(|errors| SourceError::Rejected(errors.join("；")))
}

fn finish_candidate(
    source_identity: String,
    subpath: String,
    resolved_commit: Option<String>,
    entries: Vec<ContentEntry>,
) -> Result<SkillCandidate, SourceError> {
    validate_entries(&entries)?;
    let manifest = entries
        .iter()
        .find(|entry| entry.relative_path == "SKILL.md")
        .ok_or_else(|| SourceError::Rejected("Skill 目录缺少 SKILL.md".into()))?;
    let fallback_name = subpath
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("SKILL.md")
        .to_string();
    let (name, description, diagnostics) = read_manifest(&manifest.bytes, fallback_name);
    Ok(SkillCandidate {
        source_identity,
        subpath,
        name,
        description,
        ref_name: None,
        resolved_commit,
        content_digest: content_digest(&entries),
        entries,
        diagnostics,
    })
}

fn read_manifest(bytes: &[u8], fallback: String) -> (String, Option<String>, Vec<String>) {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            return (
                fallback,
                None,
                vec![format!("SKILL.md 不是有效 UTF-8：{error}")],
            )
        }
    };
    match extract_manifest(text) {
        ManifestExtraction::Parsed(manifest) => {
            let diagnostics = validate_skill_name(&manifest.name)
                .err()
                .map(|error| vec![error.message])
                .unwrap_or_default();
            (manifest.name, manifest.description, diagnostics)
        }
        ManifestExtraction::Unreadable(failure) => (fallback, None, vec![failure.message]),
        ManifestExtraction::MissingManifest => {
            (fallback, None, vec!["SKILL.md 缺少 frontmatter".into()])
        }
    }
}

fn check_candidate_totals(candidates: &[SkillCandidate]) -> Result<(), SourceError> {
    let bytes: u64 = candidates
        .iter()
        .flat_map(|candidate| &candidate.entries)
        .map(ContentEntry::size)
        .sum();
    if candidates.len() > MAX_SOURCE_CANDIDATES || bytes > MAX_SOURCE_BYTES {
        return Err(SourceError::Rejected(format!(
            "候选超过扫描上限（{MAX_SOURCE_CANDIDATES} 个 Skill / {MAX_SOURCE_BYTES} 字节）；请缩小来源子目录"
        )));
    }
    // The import contract identifies content by digest. Ambiguous duplicate
    // subtrees must not silently replace one another's source provenance.
    let mut digests = std::collections::BTreeMap::new();
    for candidate in candidates {
        if let Some(previous) = digests.insert(&candidate.content_digest, &candidate.subpath) {
            return Err(SourceError::Rejected(format!(
                "{previous} 与 {} 的内容完全相同；请指定其中一个 Skill 子目录后扫描",
                candidate.subpath
            )));
        }
    }
    Ok(())
}

