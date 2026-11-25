use super::{observation::InterfaceProcesses, types::{InterfaceCoverage, Issue}};
use tauri::AppHandle;

pub async fn collect(app: &AppHandle) -> InterfaceProcesses {
    #[cfg(debug_assertions)]
    if crate::web_development_enabled(std::env::var_os("ASB_WEB_DEVELOPMENT").as_deref()) {
        return InterfaceProcesses { processes: Vec::new(), coverage: InterfaceCoverage::BrowserExcluded,
            issues: vec![Issue::BrowserExcluded] };
    }
    #[cfg(windows)]
    { native::collect(app).await }
    #[cfg(not(windows))]
    {
        let _ = app;
        InterfaceProcesses { processes: Vec::new(), coverage: InterfaceCoverage::Partial,
            issues: vec![Issue::WebviewPartial] }
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use crate::resource_monitor::{observation::ProcessKey, platform::process_key};
    use std::{collections::HashSet, time::Duration};
    use tauri::{Manager, WebviewWindow};
    use tokio::{sync::oneshot, task::JoinSet};
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2, ICoreWebView2_2, ICoreWebView2Environment8};
    use windows::core::Interface;

    pub async fn collect(app: &AppHandle) -> InterfaceProcesses {
        let windows = app.webview_windows();
        let mut pids = HashSet::new();
        let mut failed = windows.is_empty();
        let mut pending = JoinSet::new();
        for window in windows.values() {
            match request_process_ids(window) {
                Ok(receiver) => { pending.spawn(wait_for_process_ids(receiver)); }
                Err(error) => { failed = true; log::debug!("资源监控界面进程关联失败：{error}"); }
            }
        }
        while let Some(result) = pending.join_next().await {
            match result {
                Ok(Ok((found, complete))) => { pids.extend(found); failed |= !complete; }
                Ok(Err(error)) => { failed = true; log::debug!("资源监控界面进程关联失败：{error}"); }
                Err(error) => { failed = true; log::debug!("资源监控界面进程关联任务失败：{error}"); }
            }
        }
        failed |= pids.is_empty();
        InterfaceProcesses { processes: pids.into_iter().collect(),
            coverage: if failed { InterfaceCoverage::Partial } else { InterfaceCoverage::Verified },
            issues: if failed { vec![Issue::WebviewUnavailable] } else { Vec::new() } }
    }

    type Associations = Result<(Vec<ProcessKey>, bool), String>;

    fn request_process_ids(window: &WebviewWindow) -> Result<oneshot::Receiver<Associations>, String> {
        let (sender, receiver) = oneshot::channel();
        window.with_webview(move |view| {
            let result = unsafe { view.controller().CoreWebView2() }
                .map_err(|error| error.to_string()).and_then(|core| process_ids(&core));
            let _ = sender.send(result);
        }).map_err(|error| error.to_string())?;
        Ok(receiver)
    }

    async fn wait_for_process_ids(receiver: oneshot::Receiver<Associations>) -> Associations {
        match tokio::time::timeout(Duration::from_secs(2), receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("界面进程关联通道已关闭".into()),
            Err(_) => Err("界面线程尚未返回进程关联信息".into()),
        }
    }

    fn process_ids(core: &ICoreWebView2) -> Associations {
        let result = (|| -> windows::core::Result<Vec<u32>> {
            unsafe {
                let environment = core.cast::<ICoreWebView2_2>()?.Environment()?;
                let collection = environment.cast::<ICoreWebView2Environment8>()?.GetProcessInfos()?;
                let mut count = 0;
                collection.Count(&mut count)?;
                let mut pids = Vec::new();
                for index in 0..count {
                    let mut pid = 0;
                    collection.GetValueAtIndex(index)?.ProcessId(&mut pid)?;
                    if let Ok(pid) = u32::try_from(pid) { if pid > 0 { pids.push(pid); } }
                }
                Ok(pids)
            }
        })();
        let pids = result.map_err(|error| error.to_string())?;
        let keys: Vec<_> = pids.iter().filter_map(|&pid| process_key(pid).ok()).collect();
        let complete = keys.len() == pids.len();
        Ok((keys, complete))
    }
}
