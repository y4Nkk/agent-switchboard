use super::{database, inventory, journal::{self, Plan, Record}, transcript, Error, Operation, Phase, Result, Scope, TARGET};
use rusqlite::TransactionBehavior;
use std::{fs::{self, File}, path::Path};

pub(super) fn execute(state: &Path, scope: &Scope, id: &str, check: &dyn Fn() -> Result<()>) -> Result<Operation> {
    check()?;
    journal::require_settled(state)?;
    let plan = journal::load_plan(state, id)?;
    if plan.version != 1 || plan.scope != *scope || plan.view.id != id || plan.view.target_provider != TARGET
        || plan.view != inventory::summary(id, scope, &plan.snapshot) {
        return Err("Session transfer preview does not match this history location".into());
    }
    if plan.view.sessions == 0 { return Err("No session provider IDs need changing".into()); }
    let now = chrono::Utc::now().timestamp();
    if now < plan.created_at || now.saturating_sub(plan.created_at) > 1800 || inventory::scan(scope)? != plan.snapshot {
        return Err(Error::Changed);
    }
    let directory = journal::directory(state, id)?;
    fs::create_dir_all(directory.parent().ok_or("Invalid backup directory")?)?;
    fs::create_dir(&directory).map_err(|error| if error.kind() == std::io::ErrorKind::AlreadyExists { Error::Changed } else { error.into() })?;
    let prepared = (|| {
        let record = stage(&directory, plan)?;
        check()?;
        if inventory::scan(scope)? != record.plan.snapshot { return Err(Error::Changed); }
        verify_backups(&directory, &record)?;
        Ok(record)
    })();
    let mut record = match prepared {
        Ok(record) => record,
        Err(cause) => {
            if let Err(cleanup) = journal::discard_staging(state, id) {
                return Err(Error::Failure(format!("{cause}; cannot remove staging {}: {cleanup}", directory.display())));
            }
            return Err(cause);
        }
    };
    journal::record(state, &record)?;
    if let Err(cause) = commit(state, scope, &mut record, check) {
        return match restore(state, scope, id, check) {
            Ok(_) => Err(Error::Failure(format!("{cause}; original provider metadata was restored"))),
            Err(recovery) => Err(Error::Recovery(format!("{cause}; restoration needs attention: {recovery}"))),
        };
    }
    journal::view(state, &record)
}

fn stage(directory: &Path, plan: Plan) -> Result<Record> {
    let database_hash = database::backup(&plan.scope, directory)?;
    let mut after = Vec::new();
    for (index, source) in plan.snapshot.files.iter().enumerate() {
        match fs::symlink_metadata(transcript::temporary(&source.path, &plan.view.id)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
            Ok(_) => return Err("A publication temporary already exists; create a new preview".into()),
        }
        let before = journal::material(directory, index, &source.path, false);
        let mut output = File::options().write(true).create_new(true).open(&before)?;
        std::io::copy(&mut File::open(&source.path)?, &mut output)?;
        output.set_permissions(fs::metadata(&source.path)?.permissions())?;
        output.sync_all()?;
        drop(output);
        if transcript::digest(&before)? != source.hash { return Err(Error::Changed); }
        let candidate = journal::material(directory, index, &source.path, true);
        transcript::candidate(&before, &candidate, &source.id, source.provider.as_deref())?;
        if transcript::inspect(&candidate)?.provider.as_deref() != Some(TARGET) { return Err(Error::Changed); }
        after.push(transcript::digest(&candidate)?);
    }
    Ok(Record { version: 1, plan, phase: Phase::Applying, after, database_hash })
}

fn commit(state: &Path, scope: &Scope, record: &mut Record, check: &dyn Fn() -> Result<()>) -> Result<()> {
    let mut connection = record.plan.snapshot.rows.as_ref().map(|_| database::open(&scope.database(), true)).transpose()?;
    let transaction = connection.as_mut().map(|db| db.transaction_with_behavior(TransactionBehavior::Exclusive)).transpose()?;
    if let Some(transaction) = &transaction {
        if Some(database::read(transaction)?) != record.plan.snapshot.rows { return Err(Error::Changed); }
    }
    let directory = journal::directory(state, &record.plan.view.id)?;
    for (index, source) in record.plan.snapshot.files.iter().enumerate() {
        check()?;
        let candidate = journal::material(&directory, index, &source.path, true);
        transcript::replace(&candidate, &source.path, &source.hash, &record.after[index], &record.plan.view.id)?;
        if transcript::digest(&source.path)? != record.after[index] { return Err(Error::Changed); }
    }
    check()?;
    if let Some(transaction) = transaction {
        database::apply(&transaction, record.plan.snapshot.rows.as_deref().unwrap_or_default())?;
        transaction.commit()?;
    }
    record.phase = Phase::Applied;
    journal::record(state, record)
}

pub(super) fn restore(state: &Path, scope: &Scope, id: &str, check: &dyn Fn() -> Result<()>) -> Result<Operation> {
    check()?;
    if journal::latest(state)?.is_none_or(|record| record.plan.view.id != id) {
        return Err("Only the latest session transfer can be restored".into());
    }
    let mut record = journal::read_record(state, scope, id)?;
    if record.phase == Phase::Restored { return journal::view(state, &record); }
    inventory::validate_targets(scope, &record.plan.snapshot)?;
    let directory = journal::directory(state, id)?;
    preflight_restore(&directory, &record)?;
    let mut connection = record.plan.snapshot.rows.as_ref().map(|_| database::open(&scope.database(), true)).transpose()?;
    let transaction = connection.as_mut().map(|db| db.transaction_with_behavior(TransactionBehavior::Exclusive)).transpose()?;
    if let Some(transaction) = &transaction { database::check_restore(transaction, record.plan.snapshot.rows.as_deref().unwrap_or_default())?; }
    record.phase = Phase::Restoring;
    journal::record(state, &record)?;
    for (index, source) in record.plan.snapshot.files.iter().enumerate() {
        check()?;
        // Applying is published only after these operation-specific names
        // were verified absent; a crash can leave our partial copy here.
        transcript::clear_temporary(&source.path, id)?;
        let current = transcript::digest(&source.path)?;
        if current == source.hash { continue; }
        if current != record.after[index] { return Err(Error::Conflict(format!("Transcript changed: {}", source.path.display()))); }
        transcript::replace(&journal::material(&directory, index, &source.path, false), &source.path, &current, &source.hash, id)?;
    }
    check()?;
    if let Some(transaction) = transaction {
        database::restore(&transaction, record.plan.snapshot.rows.as_deref().unwrap_or_default())?;
        transaction.commit()?;
    }
    record.phase = Phase::Restored;
    journal::record(state, &record)?;
    journal::view(state, &record)
}

fn preflight_restore(directory: &Path, record: &Record) -> Result<()> {
    verify_backups(directory, record)?;
    for (index, source) in record.plan.snapshot.files.iter().enumerate() {
        let current = transcript::digest(&source.path)?;
        if current != source.hash && current != record.after[index] {
            return Err(Error::Conflict(format!("Transcript changed: {}", source.path.display())));
        }
    }
    Ok(())
}

fn verify_backups(directory: &Path, record: &Record) -> Result<()> {
    if let Some(expected) = &record.database_hash {
        let backup = directory.join("state_5.sqlite");
        if &transcript::digest(&backup)? != expected
            || Some(database::read(&database::open(&backup, false)?)?) != record.plan.snapshot.rows {
            return Err(Error::Conflict(format!("Native index backup changed: {}", backup.display())));
        }
    }
    for (index, source) in record.plan.snapshot.files.iter().enumerate() {
        let before = journal::material(directory, index, &source.path, false);
        if transcript::digest(&before)? != source.hash {
            return Err(Error::Conflict(format!("Transcript backup changed: {}", before.display())));
        }
    }
    Ok(())
}
