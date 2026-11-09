use super::{inventory::Snapshot, Error, Operation, Phase, Preview, Result, Scope};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fs::{self, File}, io::{Read, Write}, path::{Path, PathBuf}};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Plan { pub version: u32, pub scope: Scope, pub snapshot: Snapshot, pub view: Preview, pub created_at: i64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Record { pub version: u32, pub plan: Plan, pub phase: Phase, pub after: Vec<String>, pub database_hash: Option<String> }

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pointer { id: String }

fn base(state: &Path) -> PathBuf { super::super::layout::directory(state).join("provider-transfer") }

fn valid_id(id: &str) -> Result<()> {
    if uuid::Uuid::parse_str(id).ok().is_none_or(|value| value.to_string() != id) { return Err("Invalid session transfer ID".into()); }
    Ok(())
}

pub(super) fn plan_path(state: &Path) -> PathBuf { base(state).join("preview.json") }

pub(super) fn load_plan(state: &Path, id: &str) -> Result<Plan> {
    valid_id(id)?;
    let plan: Plan = load(&plan_path(state))?;
    if plan.view.id != id { return Err(Error::Changed); }
    Ok(plan)
}

pub(super) fn directory(state: &Path, id: &str) -> Result<PathBuf> {
    valid_id(id)?; Ok(base(state).join("operations").join(id))
}

pub(super) fn discard_staging(state: &Path, id: &str) -> Result<()> {
    let directory = directory(state, id)?;
    let expected = directory.parent().ok_or("Invalid staging parent")?.canonicalize()?.join(id);
    if directory.canonicalize()? != expected { return Err("Staging directory changed; materials were preserved".into()); }
    fs::remove_dir_all(expected)?;
    Ok(())
}

pub(super) fn material(directory: &Path, index: usize, path: &Path, candidate: bool) -> PathBuf {
    let suffix = if super::super::record_file::compressed(path) { "jsonl.zst" } else { "jsonl" };
    directory.join(format!("{}-{index}.{suffix}", if candidate { "after" } else { "before" }))
}

pub(super) fn load<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let mut bytes = Vec::new();
    File::open(path)?.take(64 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 64 * 1024 * 1024 { return Err("Session transfer manifest exceeds 64 MiB".into()); }
    serde_json::from_slice(&bytes).map_err(|error| error.to_string().into())
}

pub(super) fn save(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 { return Err("Session transfer manifest exceeds 64 MiB".into()); }
    fs::create_dir_all(path.parent().ok_or("Invalid session transfer directory")?)?;
    let temporary = path.with_extension("tmp");
    let mut file = File::create(&temporary)?;
    file.write_all(&bytes)?; file.sync_all()?; drop(file);
    crate::storage::publish::replace(&temporary, path)?;
    Ok(())
}

pub(super) fn record(state: &Path, record: &Record) -> Result<()> {
    let id = &record.plan.view.id;
    save(&directory(state, id)?.join("operation.json"), record)?;
    save(&base(state).join("latest.json"), &Pointer { id: id.clone() })
}

pub(super) fn read_record(state: &Path, scope: &Scope, id: &str) -> Result<Record> {
    let record: Record = load(&directory(state, id)?.join("operation.json"))?;
    validate(&record, scope, id)?;
    Ok(record)
}

fn validate(record: &Record, scope: &Scope, id: &str) -> Result<()> {
    if record.version != 1 || record.plan.version != 1 || record.plan.scope != *scope
        || record.plan.view.id != id || record.plan.view.target_provider != super::TARGET
        || record.after.len() != record.plan.snapshot.files.len()
        || record.database_hash.is_some() != record.plan.snapshot.rows.is_some()
        || record.plan.view != super::inventory::summary(id, scope, &record.plan.snapshot) {
        return Err("Session transfer manifest does not match the current history location".into());
    }
    Ok(())
}

pub(super) fn latest(state: &Path) -> Result<Option<Record>> {
    let path = base(state).join("latest.json");
    if !path.try_exists()? { return Ok(None); }
    let pointer: Pointer = load(&path)?;
    let record: Record = load(&directory(state, &pointer.id)?.join("operation.json"))?;
    validate(&record, &record.plan.scope, &pointer.id)?;
    Ok(Some(record))
}

pub(super) fn require_settled(state: &Path) -> Result<()> {
    if latest(state)?.is_some_and(|record| matches!(record.phase, Phase::Applying | Phase::Restoring)) {
        return Err(Error::Recovery("An interrupted session transfer must be restored first".into()));
    }
    Ok(())
}

pub(super) fn view(state: &Path, record: &Record) -> Result<Operation> {
    Ok(Operation { preview: record.plan.view.clone(), phase: record.phase,
        backup_directory: directory(state, &record.plan.view.id)?.display().to_string() })
}
