//! The native owner of checked releases and one cancellable update download.
mod state;
pub(crate) use state::UpdateManager;
use state::{DownloadState, UpdateCheck};
use crate::commands::error::{blocking, CommandError};
use std::future::Future;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::watch;

fn failure(operation: &str, error: tauri_plugin_updater::Error) -> CommandError {
    if matches!(error, tauri_plugin_updater::Error::Minisign(_)
        | tauri_plugin_updater::Error::Base64(_) | tauri_plugin_updater::Error::SignatureUtf8(_)) {
        return CommandError::localized("updater-signature-invalid", "errors.updater.signatureInvalid",
            error.to_string(), serde_json::json!({ "detail": error.to_string() }));
    }
    let (code, key) = match operation {
        "check" => ("updater-check-failed", "errors.updater.checkFailed"),
        "download" => ("updater-download-failed", "errors.updater.downloadFailed"),
        _ => ("updater-install-failed", "errors.updater.installFailed"),
    };
    CommandError::localized(code, key, error.to_string(), serde_json::json!({ "detail": error.to_string() }))
}

fn require_direct_install() -> Result<(), CommandError> {
    if crate::distribution::update_channel() == crate::distribution::UpdateChannel::MicrosoftStore {
        return Err(CommandError::keyed("updater-store-managed", "errors.updater.storeManaged",
            "此安装由 Microsoft Store 管理更新。"));
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn check_application_update(app: AppHandle) -> Result<Option<UpdateCheck>, CommandError> {
    require_direct_install()?;
    let manager = app.state::<UpdateManager>().inner().clone();
    let _guard = manager.begin_check()?;
    let builder = app.updater_builder();
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    let builder = builder.target("windows-x86_64-nsis");
    let updater = builder.build().map_err(|error| failure("check", error))?;
    let update = updater.check().await.map_err(|error| failure("check", error))?;
    manager.checked(update)
}

#[tauri::command]
pub(crate) async fn start_update_download(app: AppHandle, update_id: String) -> Result<DownloadState, CommandError> {
    require_direct_install()?;
    let lease = crate::client_software::gate::application_update().map_err(crate::client_software::error)?;
    let manager = app.state::<UpdateManager>().inner().clone();
    let (update, initial, signal) = manager.start(&update_id)?;
    let id = initial.id.clone();
    tauri::async_runtime::spawn(async move {
        let _lease = lease;
        let result = download_and_install(&manager, &id, update, signal).await;
        manager.finish(&id, result);
    });
    Ok(initial)
}

#[tauri::command]
pub(crate) async fn read_update_download(app: AppHandle, id: String) -> Result<DownloadState, CommandError> {
    app.state::<UpdateManager>().read(&id)
}

#[tauri::command]
pub(crate) async fn cancel_update_download(app: AppHandle, id: String) -> Result<DownloadState, CommandError> {
    app.state::<UpdateManager>().cancel(&id)
}

async fn until_cancelled<T>(mut signal: watch::Receiver<bool>, download: impl Future<Output = Result<T, CommandError>>)
    -> Result<Option<T>, CommandError> {
    tokio::select! {
        biased;
        _ = signal.changed() => Ok(None),
        result = download => result.map(Some),
    }
}

async fn download_and_install(manager: &UpdateManager, id: &str, update: Update,
    signal: watch::Receiver<bool>) -> Result<bool, CommandError> {
    let bytes = until_cancelled(signal, async {
        update.download(|bytes, total| manager.progress(id, bytes, total), || manager.verifying(id))
            .await.map_err(|error| failure("download", error))
    }).await?;
    let Some(bytes) = bytes else { return Ok(false); };
    if !manager.begin_install(id)? { return Ok(false); }
    blocking(move || update.install(bytes).map_err(|error| failure("install", error))).await?;
    Ok(true)
}

#[cfg(test)]
mod tests;
