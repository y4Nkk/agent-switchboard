use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use tauri::Manager;

struct Worker(Arc<(Mutex<bool>, Condvar)>);

pub(crate) fn flush_on_exit(app: &tauri::AppHandle) {
    let Some(local) = app.try_state::<crate::local_state::LocalState>() else { return; };
    match super::scan(local.root()) {
        Ok((_, issues)) => {
            for issue in issues { log::warn!("退出时 Claude 会话保留需注意：{}", issue.message); }
        }
        Err(error) => log::error!("退出时未能完成 Claude 会话保留：{error}"),
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Ok(mut stopped) = self.0.0.lock() { *stopped = true; self.0.1.notify_all(); }
    }
}

pub(crate) fn spawn(app: &tauri::AppHandle, root: PathBuf) {
    let stop = Arc::new((Mutex::new(false), Condvar::new()));
    let running = Arc::clone(&stop);
    match std::thread::Builder::new().name("claude-session-preservation".into()).spawn(move || run(root, running)) {
        Ok(_) => { app.manage(Worker(stop)); }
        Err(error) => log::error!("无法启动 Claude 会话保留：{error}"),
    }
}

fn run(root: PathBuf, stop: Arc<(Mutex<bool>, Condvar)>) {
    let mut previous = String::new();
    loop {
        let problem = match super::scan(&root) {
            Ok((_, issues)) => issues.into_iter().map(|issue| issue.message).collect::<Vec<_>>().join("；"),
            Err(error) => error,
        };
        if problem != previous {
            if !problem.is_empty() { log::warn!("Claude 会话保留需注意：{problem}"); }
            else if !previous.is_empty() { log::info!("Claude 会话保留已恢复"); }
            previous = problem;
        }
        let Ok(stopped) = stop.0.lock() else { return; };
        let Ok((stopped, _)) = stop.1.wait_timeout_while(stopped, Duration::from_secs(10), |value| !*value) else { return; };
        if *stopped { return; }
    }
}
