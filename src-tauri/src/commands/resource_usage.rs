use super::error::CommandError;
use crate::resource_monitor::{types::{ReadRequest, ResourceRead}, ResourceMonitor};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn read_resource_usage(app: AppHandle, request: ReadRequest) -> Result<ResourceRead, CommandError> {
    if !crate::workspace_visibility::workspace_visible(app.clone())? {
        return Err(CommandError::keyed("resource-reader-hidden", "resources.readerHidden", "窗口已隐藏，资源读取已暂停"));
    }
    let monitor = app.state::<ResourceMonitor>().inner().clone();
    monitor.read(app, request).await.map_err(|detail| CommandError::localized(
        "resource-read-failed", "resources.error.read", "资源占用读取失败",
        serde_json::json!({ "detail": detail }),
    ))
}
