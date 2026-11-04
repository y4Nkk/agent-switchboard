use super::{buckets, ingest, progress::Progress, reports, store};
use crate::session_manager::{read_index::{key, FileRevision, ReadIndex}, records, usage_sources::Manifest};
use asb_core::contracts::{ModelUsageIssue, ModelUsagePhase, ModelUsageReport};
use chrono::{DateTime, Local};

pub(crate) struct Batch { pub reports: Vec<ModelUsageReport>, pub refresh_soon: bool }

pub(crate) fn refresh(index: &ReadIndex, scope: &str, mut manifest: Manifest, now: impl FnOnce() -> DateTime<Local>,
    progress: &Progress<'_>) -> Result<Batch, String> {
    manifest.sources.retain(|source| source.converted.is_none());
    manifest.sources.sort_by(|left,right| (left.meta.app,&left.meta.session_id).cmp(&(right.meta.app,&right.meta.session_id)));
    let mut inputs = Vec::new();
    let mut sources = Vec::new();
    for source in &manifest.sources {
        for record in source.record_sources() {
            let location = match &record {
                records::Source::File(path) => key(path)?,
                records::Source::Saved(saved) => key(&(&saved.database,&saved.id))?,
            };
            sources.push(store::Source { id: key(&(source.meta.app,&source.meta.session_id,location))?,
                app: source.meta.app, session: source.meta.session_id.clone(), order: sources.len() });
            inputs.push(record);
        }
    }
    let connection = index.connection();
    buckets::calendar(connection, scope, progress)?;
    progress.sources(sources.len());
    store::reconcile(connection, scope, &sources, manifest.complete, progress)?;
    let mut issues: Vec<_> = manifest.issues.into_iter().filter(|issue| issue.conversion_id.is_none())
        .map(|issue| ModelUsageIssue { app: issue.app, message: issue.message }).collect();
    let mut refresh_soon = !manifest.complete;
    for (source, input) in sources.iter().zip(inputs) {
        progress.phase(ModelUsagePhase::Checking);
        let previous = store::previous(connection, scope, &source.id)?;
        let revision = revision(&input);
        match revision {
            Ok(revision) if previous.as_ref().is_some_and(|(old,_)| *old == revision) => {}
            Ok(revision) => match ingest::read(&input, source.app, &source.session, previous.map(|(_,cursor)| cursor), progress) {
                Ok(delta) => {
                    let captured = delta.cursor.revision.as_ref().map(FileRevision::encoded).transpose()?.unwrap_or(revision);
                    refresh_soon |= revision_of_file_changed(&input, &captured);
                    store::apply(connection, scope, source, &captured, delta, progress)?;
                }
                Err(message) => { refresh_soon = true; issues.push(ModelUsageIssue { app: source.app, message }); }
            },
            Err(message) => { refresh_soon = true; issues.push(ModelUsageIssue { app: source.app, message }); }
        }
        progress.source_done();
    }
    buckets::calendar(connection, scope, progress)?;
    progress.phase(ModelUsagePhase::Publishing);
    let transaction = connection.unchecked_transaction().map_err(store::error)?;
    issues.extend(store::issues(&transaction, scope)?);
    let reports = reports::build(&transaction, scope, &manifest.sources, sources.len() as u64, issues, now())?;
    transaction.commit().map_err(store::error)?;
    Ok(Batch { reports, refresh_soon })
}

fn revision(source: &records::Source) -> Result<String, String> {
    match source {
        records::Source::File(path) => FileRevision::read(path)?.encoded(),
        records::Source::Saved(saved) => Ok(saved.revision.clone()),
    }
}

fn revision_of_file_changed(source: &records::Source, captured: &str) -> bool {
    matches!(source, records::Source::File(_)) && revision(source).is_ok_and(|next| next != captured)
}
