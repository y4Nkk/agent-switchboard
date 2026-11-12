use super::error::{blocking, observe, state, CommandError};
use super::window::apply_desktop_settings;
use crate::local_state::AppSettings;
use crate::runtime_log::RuntimeLogAction;
use crate::desktop_shortcut::DesktopSettingsState;
use tauri::Manager;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsSnapshot {
    settings: AppSettings,
    desktop_error: Option<CommandError>,
}

/// Reads application-runtime settings. This is deliberately separate from the
/// Codex / Claude common configuration contract.
#[tauri::command]
pub async fn get_app_settings(app: tauri::AppHandle) -> Result<AppSettingsSnapshot, CommandError> {
    let desktop = app.state::<DesktopSettingsState>();
    let _guard = desktop.gate.lock().await;
    let state = state(&app)?;
    let settings = blocking(move || {
        state
            .get_app_settings()
            .map_err(|error| {
                CommandError::localized(
                    "app-settings-unavailable",
                    "errors.settings.unavailable",
                    error.clone(),
                    serde_json::json!({ "detail": error }),
                )
            })
    })
    .await?;
    Ok(AppSettingsSnapshot { settings, desktop_error: desktop.error()? })
}

/// Updates application-runtime settings and returns the persisted value.
#[tauri::command]
pub async fn set_app_settings(
    app: tauri::AppHandle,
    settings: AppSettings,
) -> Result<AppSettings, CommandError> {
    let desktop = app.state::<DesktopSettingsState>();
    let _guard = desktop.gate.lock().await;
    let refresh_app = app.clone();
    let write_app = app.clone();
    let saved = observe(RuntimeLogAction::AppSettingsSaved, async move {
        let app = write_app;
        let current_state = state(&app)?;
        settings
            .validate()
            .map_err(|error| {
                let (key, params) = error.parts();
                CommandError::localized("app-settings-invalid", key, error.to_string(), params)
            })?;
        let previous = blocking(move || {
            current_state
                .get_app_settings()
                .map_err(|error| CommandError::new("app-settings-unavailable", error))
        })
        .await?;
        if let Err(error) = apply_desktop_settings(&app, &settings) {
            return Err(restore_desktop_settings(&app, &previous, error));
        }
        let state = state(&app)?;
        let saved = blocking(move || {
            state
                .set_app_settings(&settings)
                .map_err(|error| {
                    CommandError::localized(
                        "app-settings-save-failed",
                        "errors.settings.saveFailed",
                        error.clone(),
                        serde_json::json!({ "detail": error }),
                    )
                })?;
            // The cache is not a second setting store: it applies the
            // just-persisted threshold to this result and later events.
            crate::runtime_log::set_level(settings.runtime_log_level);
            Ok(settings)
        })
        .await;
        saved.map_err(|error| restore_desktop_settings(&app, &previous, error))
    })
    .await?;
    desktop.set_error(None);
    crate::tray::refresh(&refresh_app);
    Ok(saved)
}

fn restore_desktop_settings(app: &tauri::AppHandle, previous: &AppSettings, error: CommandError) -> CommandError {
    let error = match apply_desktop_settings(app, previous) {
        Ok(()) => error,
        Err(rollback) => CommandError::localized(
            "desktop-settings-rollback-failed",
            "errors.settings.rollbackFailed",
            format!("{}；恢复原桌面偏好失败，请重启应用：{}", error.message, rollback.message),
            serde_json::json!({ "message": error.message, "detail": rollback.message }),
        ),
    };
    app.state::<DesktopSettingsState>().set_error(Some(error.clone()));
    error
}

/// Installed system font families, offered by the interface-font picker.
#[tauri::command]
pub async fn list_system_fonts() -> Result<Vec<String>, CommandError> {
    blocking(|| Ok(crate::fonts::system_font_families())).await
}
