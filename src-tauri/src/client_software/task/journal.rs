use crate::{client_software::types::UpdateTask, local_state::LocalState};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal { version: u32, operation: UpdateTask }

pub fn read(state: &LocalState) -> Result<Option<UpdateTask>, String> {
    let path = state.root().join("client-software/operation.json");
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Cannot read last client update: {error}")),
    };
    let mut bytes = Vec::new(); file.take(262_145).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > 262_144 { return Err("Client update journal exceeds its size limit".into()); }
    let journal: Journal = serde_json::from_slice(&bytes).map_err(|e| format!("Client update journal is invalid: {e}"))?;
    if journal.version != 1 { return Err("Unsupported client update journal".into()); }
    Ok(Some(journal.operation))
}

pub fn save(state: &LocalState, task: &UpdateTask) -> Result<(), String> {
    let directory = state.root().join("client-software");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let temporary = directory.join("operation.tmp");
    let mut file = std::fs::File::create(&temporary).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec(&Journal { version: 1, operation: task.clone() }).map_err(|e| e.to_string())?;
    file.write_all(&bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
    drop(file);
    std::fs::rename(temporary, directory.join("operation.json")).map_err(|e| e.to_string())
}
