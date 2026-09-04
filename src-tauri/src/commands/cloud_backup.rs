//! Commands for the user-owned encrypted Supabase backup endpoint.

use super::error::{blocking, observe, require_write_confirmation, state, CommandError};
use crate::cloud_backup::{self, CloudBackupResult};
use crate::local_state::CloudBackupSettings;
use crate::runtime_log::RuntimeLogAction;

#[tauri::command]
pub async fn get_cloud_backup_settings(
    app: tauri::AppHandle,
) -> Result<Option<CloudBackupSettings>, CommandError> {
    let state = state(&app)?;
    blocking(move || {
        cloud_backup::settings(&state)
            .map_err(|error| CommandError::new("cloud-backup-settings-unavailable", error))
    })
    .await
}

#[tauri::command]
pub async fn set_cloud_backup_settings(
    app: tauri::AppHandle,
    settings: CloudBackupSettings,
) -> Result<CloudBackupSettings, CommandError> {
    observe(RuntimeLogAction::CloudBackupSettingsSaved, async move {
        let state = state(&app)?;
        blocking(move || {
            cloud_backup::save_settings(&state, settings.clone())
                .map_err(|error| CommandError::new("cloud-backup-settings-save-failed", error))?;
            Ok(settings)
        })
        .await
    })
    .await
}

#[tauri::command]
pub fn cloud_backup_setup_sql() -> String {
    cloud_backup::SETUP_SQL.to_string()
}

/// Verifies the supplied draft without persisting it or writing any remote
/// data. The password is used only to obtain a short-lived Auth session.
#[tauri::command]
pub async fn test_cloud_backup_connection(
    settings: CloudBackupSettings,
    account_password: String,
) -> Result<(), CommandError> {
    observe(RuntimeLogAction::CloudBackupConnectionTested, async move {
        blocking(move || {
            cloud_backup::test_connection(&settings, &account_password)
                .map_err(|error| CommandError::new("cloud-backup-connection-test-failed", error))
        })
        .await
    })
    .await
}

#[tauri::command]
pub async fn upload_cloud_backup(
    app: tauri::AppHandle,
    account_password: String,
    backup_password: String,
    confirm_write: bool,
) -> Result<CloudBackupResult, CommandError> {
    observe(RuntimeLogAction::CloudBackupUploaded, async move {
        require_write_confirmation(confirm_write, "上传云端备份")?;
        let state = state(&app)?;
        blocking(move || {
            cloud_backup::upload(&state, &account_password, &backup_password)
                .map_err(|error| CommandError::new("cloud-backup-upload-failed", error))
        })
        .await
    })
    .await
}

#[tauri::command]
pub async fn restore_cloud_backup(
    app: tauri::AppHandle,
    account_password: String,
    backup_password: String,
    confirm_write: bool,
) -> Result<CloudBackupResult, CommandError> {
    observe(RuntimeLogAction::CloudBackupRestored, async move {
        require_write_confirmation(confirm_write, "恢复云端备份")?;
        let state = state(&app)?;
        blocking(move || {
            cloud_backup::restore(&state, &account_password, &backup_password)
                .map_err(|error| CommandError::new("cloud-backup-restore-failed", error))
        })
        .await
    })
    .await
}
