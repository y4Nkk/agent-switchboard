use asb_core::extensions::contracts::SourceRef;
use serde::Serialize;

use crate::extensions::sources::{
    normalize_github_repository, validate_source_ref, validate_source_subpath,
};

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSourceViewDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subpath: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ref_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    readme_url: Option<String>,
}

fn github_repository(identity: &str) -> bool {
    normalize_github_repository(identity).is_ok_and(|repo| repo.eq_ignore_ascii_case(identity))
}

fn safe_subpath(path: &str) -> bool {
    !path.chars().any(char::is_control)
        && !path.contains(['$', '%', '~'])
        && validate_source_subpath(path).is_ok()
}

fn safe_ref(name: &str) -> bool {
    validate_source_ref(name).is_ok()
        && !name.starts_with('/')
        && !name.ends_with('/')
        && !name.contains("..")
        && !name.contains("//")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_/.".contains(&b))
        && !asb_core::redact::is_secret_value(name)
}

pub(in crate::commands::extensions) fn skill_source_view(source: SourceRef) -> SkillSourceViewDto {
    let resolved_commit = source.resolved_commit.filter(|commit| {
        (7..=64).contains(&commit.len()) && commit.bytes().all(|b| b.is_ascii_hexdigit())
    });
    let mut view = SkillSourceViewDto {
        resolved_commit,
        ..SkillSourceViewDto::default()
    };
    let Some(commit) = view
        .resolved_commit
        .as_deref()
        .filter(|commit| commit.len() == 40)
    else {
        return view;
    };
    if !github_repository(&source.source_id) || !safe_subpath(&source.subpath) {
        return view;
    }
    let mut url = reqwest::Url::parse("https://github.com/").expect("static GitHub URL");
    {
        let mut path = url.path_segments_mut().expect("hierarchical URL");
        path.pop_if_empty()
            .extend(source.source_id.split('/'))
            .push("blob")
            .push(commit);
        if !source.subpath.is_empty() {
            path.extend(source.subpath.split('/'));
        }
        path.push("SKILL.md");
    }
    view.repo = Some(source.source_id);
    view.subpath = Some(source.subpath);
    view.ref_name = source.ref_name.filter(|name| safe_ref(name));
    view.readme_url = Some(url.into());
    view
}

