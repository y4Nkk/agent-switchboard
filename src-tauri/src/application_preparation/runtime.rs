use super::{check_data, input_revision, needs_recovery, Preparation, Report, Stage, Status};
use crate::{commands::{error::CommandError, ConfigWriteGate}, desktop_shortcut::DesktopSettingsState,
    local_state::{LocalState, SettingsPreparation}};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::{path::Path, sync::{atomic::Ordering, mpsc::{Receiver, SyncSender}, Arc, Mutex}, time::Duration};
use tauri::{Emitter, Manager};

const EVENT: &str = "application-preparation-changed";

pub(crate) fn start(app: tauri::AppHandle, local: LocalState, preferences: Result<SettingsPreparation, String>,
    sender: SyncSender<()>, receiver: Receiver<()>) {
    let failure_app = app.clone();
    if let Err(error) = std::thread::Builder::new().name("application-preparation".into()).spawn(move || {
        let panic_app = app.clone();
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(app, local, preferences, sender, receiver))).is_err() {
            blocked(&panic_app, "应用准备线程异常结束，恢复材料已保留；请重新启动并查看日志".into());
            if let Some(code) = panic_app.state::<Preparation>().complete_check() { panic_app.exit(code); }
        }
    }) { blocked(&failure_app, format!("无法启动应用准备线程：{error}")); }
}

fn run(app: tauri::AppHandle, local: LocalState, preferences: Result<SettingsPreparation, String>,
    sender: SyncSender<()>, receiver: Receiver<()>) {
    let mut first = Some(preferences);
    let mut previous = String::new();
    let mut watcher = None;
    let mut service_issues = Vec::new();
    let watch_failure = Arc::new(Mutex::new(None));
    loop {
        if first.is_none() {
            if receiver.recv().is_err() { return; }
            std::thread::sleep(Duration::from_millis(120));
            while receiver.try_recv().is_ok() {}
        }
        if watch_failure.lock().ok().and_then(|mut failure| failure.take()).is_some() {
            watcher = None;
            app.state::<Preparation>().force.store(true, Ordering::Release);
        }
        let result = check_once(&app, &local, &mut first, &mut previous, &mut service_issues);
        if let Some(code) = app.state::<Preparation>().complete_check() { app.exit(code); }
        match result {
            Ok(false) => continue,
            Ok(true) => {},
            Err(error) => blocked(&app, error),
        }
        if let Err(error) = update_watch(&mut watcher, local.root(), &sender, &watch_failure) {
            blocked(&app, format!("应用数据监听不可用：{error}"));
        }
        if input_revision(&local) != previous { let _ = sender.try_send(()); }
    }
}

fn check_once(app: &tauri::AppHandle, local: &LocalState, first: &mut Option<Result<SettingsPreparation, String>>,
    previous: &mut String, service_issues: &mut Vec<CommandError>) -> Result<bool, String> {
    let preparation = app.state::<Preparation>();
    let force = preparation.force.swap(false, Ordering::AcqRel);
    let desktop = app.state::<DesktopSettingsState>();
    let _desktop_guard = desktop.gate.blocking_lock();
    let gate = app.state::<ConfigWriteGate>();
    let guard = gate.lock()?;
    if !preparation.claim(first.is_some())? { return Ok(false); }
    let input = input_revision(local);
    if first.is_none() && !force && input == *previous { return Ok(false); }
    let initialized = preparation.initialized.load(Ordering::Acquire);
    let blocking = !initialized || needs_recovery(local);
    let old_settings = preparation.snapshot().map_err(|error| error.message)?.report.settings;
    if blocking { publish(app, Stage::Preferences, Status::Running, Report::default(), true); }
    let mut report = check_data(local, first.take(), |stage, report| {
        if blocking { publish(app, stage, Status::Running, report.clone(), false); }
    });
    *previous = report.observed_revision.clone();
    drop(guard);
    if blocking { publish(app, Stage::Services, Status::Running, report.clone(), false); }
    if !initialized {
        match crate::startup::initialize_services(app, local) {
            Ok(issues) => { *service_issues = issues; preparation.initialized.store(true, Ordering::Release); }
            Err(error) => {
                report.issues.push(CommandError::new("startup-services-unavailable", error));
                publish(app, Stage::Complete, Status::Blocked, report, false);
                return Ok(true);
            }
        }
    }
    report.issues.extend(service_issues.iter().cloned());
    if report.configuration_ready {
        let errors = crate::startup::recover_configuration(app, local)?;
        if !errors.is_empty() { report.configuration_ready = false; report.issues.extend(errors); }
    }
    if let Some(settings) = &report.settings {
        if !initialized || force || old_settings.as_ref() != Some(settings) {
            if let Err(error) = crate::commands::apply_desktop_settings(app, settings) { desktop.set_error(Some(error)); }
            else { desktop.set_error(None); }
        }
        crate::runtime_log::set_level(settings.runtime_log_level);
    }
    if let Some(error) = desktop.error().map_err(|error| error.message)? { report.issues.push(error); }
    let status = if report.issues.is_empty() { Status::Ready } else { Status::Degraded };
    publish(app, Stage::Complete, status, report, !blocking);
    crate::tray::refresh(app);
    Ok(true)
}

fn publish(app: &tauri::AppHandle, stage: Stage, status: Status, report: Report, new_run: bool) {
    match app.state::<Preparation>().publish(stage, status, report, new_run) {
        Ok(snapshot) => {
            if status != Status::Running {
                for issue in &snapshot.report.issues { log::warn!("应用准备：{}：{}", issue.code, issue.message); }
                if !snapshot.report.repaired_files.is_empty() {
                    for path in &snapshot.report.backup_paths { log::info!("自动修复保留原件：{path}"); }
                    crate::runtime_log::record_success(crate::runtime_log::RuntimeLogAction::ApplicationDataRepaired);
                }
                if let Some(issue) = snapshot.report.issues.first() {
                    crate::runtime_log::record_failure(crate::runtime_log::RuntimeLogAction::ApplicationDataRepaired, issue.code);
                    crate::reveal_main_window(app);
                }
            }
            if let Err(error) = app.emit(EVENT, &snapshot) { log::warn!("准备状态通知失败：{error}"); }
        }
        Err(error) => log::error!("准备状态无法发布：{}", error.message),
    }
}

fn blocked(app: &tauri::AppHandle, error: String) {
    let mut report = app.state::<Preparation>().snapshot().map(|snapshot| snapshot.report).unwrap_or_default();
    report.issues.push(CommandError::new("application-preparation-failed", error));
    publish(app, Stage::Complete, Status::Blocked, report, false);
}

fn update_watch(watcher: &mut Option<RecommendedWatcher>, root: &Path, sender: &SyncSender<()>, failure: &Arc<Mutex<Option<String>>>) -> Result<(), notify::Error> {
    if let Some(watcher) = watcher {
        let configuration = root.join("configuration");
        let _ = watcher.unwatch(&configuration);
        if configuration.is_dir() { watcher.watch(&configuration, RecursiveMode::Recursive)?; }
    } else { *watcher = Some(watch(root, sender.clone(), Arc::clone(failure))?); }
    Ok(())
}

fn watch(root: &Path, sender: SyncSender<()>, failure: Arc<Mutex<Option<String>>>) -> Result<RecommendedWatcher, notify::Error> {
    let targets = root.to_path_buf();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| {
        let relevant = match event {
            Ok(event) => relevant_event(&event, &targets),
            Err(error) => {
                log::error!("应用数据监听失败：{error}");
                if let Ok(mut slot) = failure.lock() { *slot = Some(error.to_string()); }
                true
            }
        };
        if relevant { let _ = sender.try_send(()); }
    })?;
    watcher.watch(root, RecursiveMode::NonRecursive)?;
    let configuration = root.join("configuration");
    if configuration.is_dir() { watcher.watch(&configuration, RecursiveMode::Recursive)?; }
    Ok(watcher)
}

fn relevant_event(event: &Event, root: &Path) -> bool {
    if matches!(event.kind, notify::EventKind::Access(_)) { return false; }
    event.paths.iter().any(|path| path.strip_prefix(root).is_ok_and(|relative| {
        let first = relative.components().next().map(|part| part.as_os_str());
        ["configuration", "settings.json", "settings-recovery.json", "outbound-proxy.json",
            crate::config_store::REPAIR_JOURNAL_FILE, "profiles.json", "configuration-upgrade.json"]
            .iter().any(|name| first == Some(std::ffi::OsStr::new(name)))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_owned_data_mutations_wake_preparation() {
        let root = Path::new("fixture-root");
        let changed = notify::EventKind::Modify(notify::event::ModifyKind::Data(notify::event::DataChange::Content));
        for name in ["configuration/providers/codex/p.json", "settings.json", "outbound-proxy.json"] {
            assert!(relevant_event(&Event::new(changed).add_path(root.join(name)), root));
        }
        assert!(!relevant_event(&Event::new(changed).add_path(root.join("sessions/history.sqlite3")), root));
        let access = notify::EventKind::Access(notify::event::AccessKind::Read);
        assert!(!relevant_event(&Event::new(access).add_path(root.join("settings.json")), root));
    }

    #[test]
    fn native_watcher_observes_an_isolated_configuration_write() {
        let temporary = tempfile::tempdir().unwrap();
        std::fs::create_dir(temporary.path().join("configuration")).unwrap();
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let failure = Arc::new(Mutex::new(None));
        let _watcher = watch(temporary.path(), sender, Arc::clone(&failure)).unwrap();
        std::fs::write(temporary.path().join("configuration/test.json"), b"fixture").unwrap();
        receiver.recv_timeout(Duration::from_secs(5)).expect("the production watcher must observe the write");
        assert!(failure.lock().unwrap().is_none());
    }
}
