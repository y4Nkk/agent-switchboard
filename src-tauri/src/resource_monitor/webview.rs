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
    use tokio::sync::oneshot;
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2, ICoreWebView2_2, ICoreWebView2Environment8};
    use windows::core::Interface;

    pub async fn collect(app: &AppHandle) -> InterfaceProcesses {
        let windows = app.webview_windows();
        let mut pids = HashSet::new();
        let mut failed = windows.is_empty();
        for window in windows.values() {
            match window_processes(window).await {
                Ok((found, complete)) => { pids.extend(found); failed |= !complete; }
                Err(error) => { failed = true; log::debug!("资源监控界面进程关联失败：{error}"); }
            }
        }
        if pids.is_empty() { failed = true; }
        InterfaceProcesses { processes: pids.into_iter().collect(),
            coverage: if failed { InterfaceCoverage::Partial } else { InterfaceCoverage::Verified },
            issues: if failed { vec![Issue::WebviewUnavailable] } else { Vec::new() } }
    }

    async fn window_processes(window: &WebviewWindow) -> Result<(Vec<ProcessKey>, bool), String> {
        let (sender, receiver) = oneshot::channel();
        window.with_webview(move |view| {
            let result = unsafe { view.controller().CoreWebView2() }
                .map_err(|error| error.to_string()).and_then(|core| process_ids(&core));
            let _ = sender.send(result);
        }).map_err(|error| error.to_string())?;
        match tokio::time::timeout(Duration::from_secs(2), receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("界面进程关联通道已关闭".into()),
            Err(_) => Err("界面线程尚未返回进程关联信息".into()),
        }
    }

    fn process_ids(core: &ICoreWebView2) -> Result<(Vec<ProcessKey>, bool), String> {
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
        let mut keys = Vec::new();
        let mut complete = true;
        for pid in result.map_err(|error| error.to_string())? {
            match process_key(pid) { Ok(key) => keys.push(key), Err(_) => complete = false }
        }
        Ok((keys, complete))
    }
}
