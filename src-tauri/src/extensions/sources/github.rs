use std::collections::BTreeMap;

use asb_core::extensions::skill::ContentEntry;
use asb_core::extensions::validate::{validate_content_paths, ContentEntryKind};
use serde::Deserialize;

use super::archive::extract_tar_gz_rooted;
use super::transport::{fetch_bytes, MAX_API_DOWNLOAD, MAX_ARCHIVE_DOWNLOAD};
use super::{check_candidate_totals, finish_candidate, HttpFetch, SkillCandidate, SourceError};
use super::{MAX_SOURCE_BYTES, MAX_SOURCE_CANDIDATES, MAX_SOURCE_ENTRIES};

pub(crate) fn repository(input: &str) -> Result<String, SourceError> {
    let input = input.trim();
    let path = if input.contains("://") {
        let url = reqwest::Url::parse(input).map_err(|_| invalid_repository())?;
        if !matches!(url.scheme(), "https" | "http")
            || url.host_str() != Some("github.com")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(invalid_repository());
        }
        url.path().trim_start_matches('/').to_string()
    } else {
        input.to_string()
    };
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, name) = path.split_once('/').ok_or_else(invalid_repository)?;
    let owner_valid = !owner.is_empty()
        && owner.len() <= 39
        && !owner.starts_with('-')
        && !owner.ends_with('-')
        && owner
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    let name_valid = !name.is_empty()
        && name.len() <= 100
        && !matches!(name, "." | "..")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte));
    if !owner_valid || !name_valid {
        return Err(invalid_repository());
    }
    Ok(format!("{owner}/{name}").to_lowercase())
}

fn invalid_repository() -> SourceError {
    SourceError::Rejected(
        "请输入 owner/repository 或 GitHub 仓库 URL；分支和子目录请在高级选项填写".into(),
    )
}

pub(crate) fn validate_subpath(subpath: &str) -> Result<(), SourceError> {
    if subpath.is_empty() {
        return Ok(());
    }
    validate_content_paths(&[(subpath.into(), ContentEntryKind::Dir, 0)])
        .map_err(|errors| SourceError::Rejected(errors.join("；")))
}

fn validate_commit(commit: &str) -> Result<(), SourceError> {
    if commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(SourceError::Rejected("GitHub 返回了无效的提交 SHA".into()))
    }
}

pub(crate) fn validate_ref_name(ref_name: &str) -> Result<(), SourceError> {
    if ref_name.is_empty() || ref_name.len() > 256 || ref_name.chars().any(char::is_control) {
        return Err(SourceError::Rejected("分支或提交无效".into()));
    }
    Ok(())
}

pub fn resolve_github_commit(
    repo: &str,
    ref_name: &str,
    fetch: HttpFetch,
) -> Result<String, SourceError> {
    let repo = repository(repo)?;
    validate_ref_name(ref_name)?;
    let mut url = reqwest::Url::parse(&format!("https://api.github.com/repos/{repo}/commits/"))
        .expect("validated GitHub repository");
    url.path_segments_mut()
        .expect("hierarchical URL")
        .pop_if_empty()
        .push(ref_name);
    let bytes = fetch_bytes(url.as_str(), MAX_API_DOWNLOAD, fetch)?;
    #[derive(Deserialize)]
    struct Commit {
        sha: String,
    }
    let response: Commit = serde_json::from_slice(&bytes)
        .map_err(|error| SourceError::Unreachable(format!("GitHub 提交响应无法解析：{error}")))?;
    validate_commit(&response.sha)?;
    Ok(response.sha.to_lowercase())
}

#[derive(Deserialize)]
struct TreeEntry {
    path: String,
    mode: String,
    #[serde(rename = "type")]
    kind: String,
    size: Option<u64>,
}

#[derive(Deserialize)]
struct Tree {
    truncated: bool,
    tree: Vec<TreeEntry>,
}

fn tree_files(
    repo: &str,
    commit: &str,
    subpath: &str,
    fetch: HttpFetch,
) -> Result<BTreeMap<String, u64>, SourceError> {
    let url = format!("https://api.github.com/repos/{repo}/git/trees/{commit}?recursive=1");
    let bytes = fetch_bytes(&url, MAX_API_DOWNLOAD, fetch)?;
    let tree: Tree = serde_json::from_slice(&bytes)
        .map_err(|error| SourceError::Unreachable(format!("GitHub 目录树无法解析：{error}")))?;
    if tree.truncated {
        return Err(SourceError::Rejected(
            "GitHub 目录树被截断，未接受不完整的扫描结果".into(),
        ));
    }
    if tree.tree.len() > MAX_SOURCE_ENTRIES {
        return Err(SourceError::Rejected(format!(
            "仓库目录树超过 {MAX_SOURCE_ENTRIES} 个条目"
        )));
    }
    let mut files = BTreeMap::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut total = 0u64;
    for entry in tree.tree {
        if entry.path.is_empty() {
            return Err(SourceError::Rejected("GitHub 目录树包含空路径".into()));
        }
        validate_subpath(&entry.path)?;
        if !seen.insert(entry.path.to_lowercase()) {
            return Err(SourceError::Rejected(format!(
                "{} 存在重复或大小写冲突",
                entry.path
            )));
        }
        match (entry.kind.as_str(), entry.mode.as_str()) {
            ("tree", "040000") => continue,
            ("blob", "100644" | "100755") => (),
            _ => {
                return Err(SourceError::Rejected(format!(
                    "{} 是链接、子模块或特殊条目",
                    entry.path
                )))
            }
        }
        let size = entry
            .size
            .ok_or_else(|| SourceError::Rejected(format!("{} 缺少文件大小", entry.path)))?;
        total = total
            .checked_add(size)
            .ok_or_else(|| SourceError::Rejected("仓库大小溢出".into()))?;
        if total > MAX_SOURCE_BYTES {
            return Err(SourceError::Rejected("仓库内容超过扫描字节上限".into()));
        }
        if let Some(path) = relative_to(&entry.path, subpath) {
            files.insert(path.into(), size);
        }
    }
    Ok(files)
}

pub(super) fn relative_to<'a>(path: &'a str, subpath: &str) -> Option<&'a str> {
    if subpath.is_empty() {
        return Some(path);
    }
    path.strip_prefix(subpath)?
        .strip_prefix('/')
        .filter(|rest| !rest.is_empty())
}

fn download_entries(
    repo: &str,
    commit: &str,
    subpath: &str,
    fetch: HttpFetch,
) -> Result<Vec<ContentEntry>, SourceError> {
    let url = format!("https://codeload.github.com/{repo}/tar.gz/{commit}");
    let bytes = fetch_bytes(&url, MAX_ARCHIVE_DOWNLOAD, fetch)?;
    extract_tar_gz_rooted(&bytes, subpath)
}

pub fn resolve_github_source(
    repo: &str,
    subpath: &str,
    ref_name: Option<&str>,
    fetch: HttpFetch,
) -> Result<Vec<SkillCandidate>, SourceError> {
    resolve_github_source_with_requests(repo, subpath, ref_name, |_, url| fetch(url))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GitHubRequest {
    Commit,
    Tree,
    Archive,
}

pub(crate) fn resolve_github_source_with_requests(
    repo: &str,
    subpath: &str,
    ref_name: Option<&str>,
    fetch: impl Fn(GitHubRequest, &str) -> Result<(u16, Vec<u8>), String>,
) -> Result<Vec<SkillCandidate>, SourceError> {
    let repo = repository(repo)?;
    let subpath = subpath.trim();
    validate_subpath(subpath)?;
    let ref_name = ref_name.map(str::trim).filter(|name| !name.is_empty());
    let commit = resolve_github_commit(&repo, ref_name.unwrap_or("HEAD"), &|url| {
        fetch(GitHubRequest::Commit, url)
    })?;
    let files = tree_files(&repo, &commit, subpath, &|url| fetch(GitHubRequest::Tree, url))?;
    let roots: Vec<_> = files
        .keys()
        .filter_map(|path| {
            if path == "SKILL.md" {
                Some("")
            } else {
                path.strip_suffix("/SKILL.md")
            }
        })
        .collect();
    if roots.len() > MAX_SOURCE_CANDIDATES {
        return Err(SourceError::Rejected(format!(
            "来源超过 {MAX_SOURCE_CANDIDATES} 个 Skill；请指定子目录"
        )));
    }
    if roots.is_empty() {
        if !subpath.is_empty() && files.is_empty() {
            return Err(SourceError::Rejected(format!(
                "仓库中不存在子目录 {subpath}"
            )));
        }
        return Ok(Vec::new());
    }
    let entries = download_entries(&repo, &commit, subpath, &|url| fetch(GitHubRequest::Archive, url))?;
    verify_tree_content(&files, &entries)?;
    let mut candidates = Vec::new();
    for root in roots {
        let content = entries
            .iter()
            .filter_map(|entry| {
                let relative = relative_to(&entry.relative_path, root)?;
                Some(ContentEntry {
                    relative_path: relative.into(),
                    ..entry.clone()
                })
            })
            .collect();
        let path = [subpath, root]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("/");
        let mut candidate =
            finish_candidate(repo.clone(), path.clone(), Some(commit.clone()), content)
                .map_err(|error| SourceError::Rejected(format!("{path}: {error}")))?;
        candidate.ref_name = ref_name.map(str::to_owned);
        candidates.push(candidate);
        check_candidate_totals(&candidates)?;
    }
    Ok(candidates)
}

fn verify_tree_content(
    files: &BTreeMap<String, u64>,
    entries: &[ContentEntry],
) -> Result<(), SourceError> {
    let actual: BTreeMap<_, _> = entries
        .iter()
        .filter(|entry| entry.kind == ContentEntryKind::File)
        .map(|entry| (entry.relative_path.clone(), entry.size()))
        .collect();
    if files != &actual {
        return Err(SourceError::Rejected(
            "下载内容与固定提交的目录树不一致；压缩包可能缺失或被截断".into(),
        ));
    }
    Ok(())
}

/// Update checks reuse the same bounded archive reader for one known Skill.
pub fn fetch_github_subtree(
    repo: &str,
    commit: &str,
    subpath: &str,
    fetch: HttpFetch,
) -> Result<SkillCandidate, SourceError> {
    let repo = repository(repo)?;
    validate_commit(commit)?;
    validate_subpath(subpath)?;
    let entries = download_entries(&repo, commit, subpath, fetch)?;
    let candidate = finish_candidate(repo, subpath.into(), Some(commit.into()), entries)?;
    if !candidate.diagnostics.is_empty() {
        return Err(SourceError::Rejected(candidate.diagnostics.join("；")));
    }
    Ok(candidate)
}
