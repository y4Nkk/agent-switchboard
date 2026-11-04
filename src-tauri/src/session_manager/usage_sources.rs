//! The session owner exposes usage identities without parsing display metadata
//! or synchronizing Claude's external originals on a statistics request.
use super::{history, native, parser, sources, read_index::ReadIndex, SessionIssue, SessionMeta, SessionSource};
use asb_core::contracts::AppKind;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(crate) struct Manifest {
    pub sources: Vec<SessionSource>,
    pub issues: Vec<SessionIssue>,
    /// Only discovery failures make absence uncertain; saved-content warnings do not.
    pub complete: bool,
}

pub(crate) fn discover(root: &Path, index: &ReadIndex) -> Result<Manifest, String> {
    let codex_root = super::codex_root()?;
    let roots = super::codex_session_roots()?;
    let mut catalog = BTreeMap::new();
    let mut issues = Vec::new();
    let mut complete = true;
    match native::usage_identities(&codex_root) {
        Ok(identities) => {
            for id in identities {
                let identity = parser::SessionIdentity { id: id.clone(), parent: None, created_at: None };
                catalog.insert((AppKind::Codex, id), unregistered(root, &identity));
            }
        }
        Err(message) => { complete = false; issues.push(SessionIssue::new(AppKind::Codex, message)); }
    }
    for directory in roots {
        let files = sources::collect(&directory);
        complete &= files.issues.is_empty();
        issues.extend(files.issues.into_iter().map(|message| SessionIssue::new(AppKind::Codex, message)));
        for path in files.paths {
            match identity(index, &path) {
                Ok(identity) => {
                    let source = catalog.entry((AppKind::Codex, identity.id.clone()))
                        .or_insert_with(|| unregistered(root, &identity));
                    source.paths.push(path);
                }
                Err(message) => { complete = false; issues.push(SessionIssue::new(AppKind::Codex, message)); }
            }
        }
    }
    match history::committed_sources(root) {
        Ok((saved, warnings)) => {
            issues.extend(warnings);
            for source in saved {
                issues.extend(source.retained.iter().filter_map(|saved| saved.issue.clone())
                    .map(|message| SessionIssue::new(AppKind::Claude, message)));
                catalog.insert((AppKind::Claude, source.meta.session_id.clone()), source);
            }
        }
        Err(message) => { complete = false; issues.push(SessionIssue::new(AppKind::Claude, message)); }
    }
    Ok(Manifest { complete, sources: catalog.into_values().collect(), issues })
}

fn identity(index: &ReadIndex, path: &Path) -> Result<parser::SessionIdentity, String> {
    index.file_prefix("identity", path, &|| Ok(()), |reader|
        parser::session_identity_reader(path, reader, &|| Ok(())).map_err(|error| error.to_string()))?
        .ok_or_else(|| format!("{} 不含可识别的会话身份", path.display()))
}

fn unregistered(root: &Path, identity: &parser::SessionIdentity) -> SessionSource {
    SessionSource { meta: SessionMeta { app: AppKind::Codex, session_id: identity.id.clone(),
        title: identity.id.clone(), summary: String::new(), project_dir: None,
        created_at: identity.created_at.clone(), last_active_at: None, resume_command: None,
        alias: None, pinned: false, tags: Vec::new(), parent_session_id: identity.parent.clone(),
        can_delete: false, preservation: None }, paths: Vec::<PathBuf>::new(), native_history: None,
        converted: None, retained: Vec::new(), index_root: root.into() }
}
