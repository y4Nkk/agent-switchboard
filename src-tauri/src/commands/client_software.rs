use super::error::{blocking, state, CommandError};
use crate::client_software::{self, types::{Snapshot, UpdatePlan, UpdateTask}, SoftwareManager};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn select_client_software(app: AppHandle, client: asb_core::contracts::AppKind) -> Result<Option<Snapshot>, CommandError> {
    let extensions = if cfg!(windows) { vec!["exe".into(), "cmd".into()] } else { vec!["*".into()] };
    let Some(path) = super::window::pick_file(app.clone(), "Codex / Claude CLI".into(), extensions).await? else { return Ok(None); };
    let state = state(&app)?;
    let manager = app.state::<SoftwareManager>().inner().clone();
    blocking(move || manager.select(&state, client, path.into()).map(Some).map_err(client_software::error)).await
}

#[tauri::command]
pub async fn read_client_software(app: AppHandle) -> Result<Snapshot, CommandError> {
    let state = state(&app)?;
    let manager = app.state::<SoftwareManager>().inner().clone();
    blocking(move || manager.read(&state).map_err(client_software::error)).await
}

#[tauri::command]
pub async fn refresh_client_software(app: AppHandle, force: bool) -> Result<Snapshot, CommandError> {
    let state = state(&app)?;
    let manager = app.state::<SoftwareManager>().inner().clone();
    blocking(move || manager.refresh(&state, true, force).map_err(client_software::error)).await
}

#[tauri::command]
pub async fn prepare_client_update(app: AppHandle, installation_id: String, revision: String) -> Result<UpdatePlan, CommandError> {
    let state = state(&app)?;
    let manager = app.state::<SoftwareManager>().inner().clone();
    blocking(move || manager.prepare(&state, &installation_id, &revision).map_err(client_software::error)).await
}

#[tauri::command]
pub async fn start_client_update(app: AppHandle, plan_id: String) -> Result<UpdateTask, CommandError> {
    let state = state(&app)?;
    let manager = app.state::<SoftwareManager>().inner().clone();
    blocking(move || manager.start(&state, &plan_id).map_err(client_software::error)).await
}

#[tauri::command]
pub async fn read_client_update(app: AppHandle) -> Result<Option<UpdateTask>, CommandError> {
    app.state::<SoftwareManager>().task().map_err(client_software::error)
}

#[tauri::command]
pub async fn cancel_client_update(app: AppHandle, id: String) -> Result<UpdateTask, CommandError> {
    app.state::<SoftwareManager>().cancel(&id).map_err(client_software::error)
}
