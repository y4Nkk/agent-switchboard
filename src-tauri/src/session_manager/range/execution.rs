use super::{snapshot, Action, Operations, Outcome, Plan};
use crate::session_manager::{self as sessions};
use std::{collections::BTreeSet, path::Path};

pub(super) fn run(operations: &Operations, root: &Path, plan: Plan) -> Result<(), String> {
    let (mut sources, issues) = sessions::scan_filtered_sources(root, plan.preview.request.app, None)?;
    let values = sessions::organization::load(root)?;
    for source in &mut sources { sessions::organization::apply(&mut source.meta, &values); }
    snapshot::validate(&plan, &sources)?;
    let mut done = BTreeSet::new();
    // Parents are processed first so native cascading deletion never runs again for a removed child.
    let mut requests: Vec<_> = plan.preview.items.iter().collect();
    requests.sort_by_key(|item| depth(&item.session, &sources));
    for item in requests {
        let key = (item.session.app, item.session.session_id.clone());
        if done.contains(&key) { continue; }
        let source = sessions::locate(&sources, &issues, key.0, &key.1)?;
        let result = snapshot::related(source, &sources, plan.preview.action).iter().try_for_each(|member| {
            if done.contains(&snapshot::key(member)) { return Ok(()); }
            if plan.fingerprints.get(&snapshot::key(member)) != Some(&snapshot::fingerprint(member)?) {
                return Err("会话在执行期间变化，已保留；请重新预览".into());
            }
            Ok(())
        }).and_then(|()| match plan.preview.action {
            Action::Delete => sessions::delete_source(root, source, &sources).map(|outcomes| outcomes.into_iter().map(|outcome|
                Outcome { app: outcome.app, session_id: outcome.session_id, completed: outcome.deleted, error: outcome.error }).collect()),
            Action::Archive | Action::Unarchive => {
                let archived = plan.preview.action == Action::Archive;
                let ids = if source.archived != archived { sessions::archive::set(root, source, &sources, archived)? }
                    else { vec![key.1.clone()] };
                Ok(ids.into_iter().map(|session_id| Outcome { app: key.0, session_id, completed: true, error: None }).collect())
            }
        });
        let outcomes = result.unwrap_or_else(|error| vec![Outcome {
            app: key.0, session_id: key.1.clone(), completed: false, error: Some(error),
        }]);
        let mut state = operations.lock()?;
        for outcome in outcomes {
            let key = (outcome.app, outcome.session_id.clone());
            if outcome.completed { done.insert(key.clone()); }
            if let Some(existing) = state.progress.outcomes.iter_mut().find(|value| (value.app, value.session_id.clone()) == key) {
                *existing = outcome;
            } else { state.progress.outcomes.push(outcome); }
        }
        state.progress.completed = state.progress.outcomes.len();
    }
    if plan.preview.action == Action::Delete && plan.preview.items.iter().any(|item| item.session.app == asb_core::AppKind::Claude) {
        if let Err(error) = sessions::history::reclaim_space(root) { operations.lock()?.progress.warning = Some(error); }
    }
    Ok(())
}

fn depth(meta: &sessions::SessionMeta, sources: &[sessions::SessionSource]) -> usize {
    let mut parent = meta.parent_session_id.as_deref();
    let mut seen = BTreeSet::new();
    while let Some(id) = parent {
        if !seen.insert(id) { break; }
        parent = sources.iter().find(|source| source.meta.app == meta.app && source.meta.session_id == id)
            .and_then(|source| source.meta.parent_session_id.as_deref());
    }
    seen.len()
}
