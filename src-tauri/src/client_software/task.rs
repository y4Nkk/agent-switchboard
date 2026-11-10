mod journal;
mod output;
mod spawn;
#[cfg(test)] mod tests;
#[cfg(all(test, windows))] mod live_tests;
use super::{gate, plan, process, updater, SoftwareError, SoftwareManager, types::{Phase, UpdateTask}};
use crate::local_state::LocalState;
use std::{sync::atomic::Ordering, time::Duration};

pub fn initialize(manager: &SoftwareManager, state: &LocalState) -> Result<(), String> {
    let mut data = manager.lock()?;
    if data.initialized { return Ok(()); }
    data.task = journal::read(state)?;
    if let Some(task) = data.task.as_mut().filter(|task| task.phase.active()) {
        task.phase = Phase::Verifying;
        task.revision = task.revision.saturating_add(1);
        task.cancellable = false;
        task.error = Some(SoftwareError::from("ASB stopped during this operation. The updater was not restarted. Recheck the installation and its original manager.").into_message());
        journal::save(state, task)?;
        let task = task.clone();
        let lease = gate::update(task.plan.app)?;
        let manager = manager.clone(); let state = state.clone();
        std::thread::spawn(move || recover(manager, state, task, lease));
    }
    data.initialized = true;
    Ok(())
}

fn recover(manager: SoftwareManager, state: LocalState, task: UpdateTask, lease: gate::ExclusiveLease) {
    wait_for_updater(&manager, &state, &task);
    let result = verify(&state, &task).map(Some).map_err(SoftwareError::from);
    updater::cleanup(&state, &task.plan.id);
    drop(lease);
    if let Err(error) = finish(&manager, &state, result) { log::error!("Cannot save recovered client update: {error}"); }
}

fn wait_for_updater(manager: &SoftwareManager, state: &LocalState, task: &UpdateTask) {
    while let Some(pid) = task.process_id {
        match crate::resource_monitor::client_processes::birth(pid) {
            Ok(Some(birth)) if task.process_birth.as_ref().is_none_or(|expected| *expected == birth) => std::thread::sleep(Duration::from_secs(2)),
            Ok(_) => break,
            Err(error) => { let _ = change(&manager, &state, |t| t.error = Some(SoftwareError::from(error).into_message())); std::thread::sleep(Duration::from_secs(3)); }
        }
    }
    #[cfg(windows)]
    if task.plan.manager == super::types::Manager::Winget {
        loop {
            match super::winget::installing(task.plan.app) {
                Ok(false) => break,
                Ok(true) => std::thread::sleep(Duration::from_secs(2)),
                Err(error) => { let _ = change(&manager, &state, |t| t.error = Some(SoftwareError::from(error).into_message())); std::thread::sleep(Duration::from_secs(3)); }
            }
        }
    }
}

pub fn start(manager: &SoftwareManager, state: &LocalState, id: &str) -> Result<UpdateTask, SoftwareError> {
    let _refresh = manager.refreshing.lock().map_err(|_| "Client refresh unavailable")?;
    let prepared = manager.lock()?.prepared.clone().filter(|p| p.view.id == id).ok_or("Update preview is missing; prepare again")?;
    let lease = gate::update(prepared.view.app)?;
    plan::revalidate(state, &prepared)?;
    let mut task = UpdateTask { id: uuid::Uuid::new_v4().to_string(), revision: 0, plan: prepared.view.clone(), phase: Phase::Preparing,
        started_at: chrono::Utc::now().to_rfc3339(), finished_at: None, actual_version: None,
        cancellable: true, log: Vec::new(), error: None, process_id: None, process_birth: None };
    {
        let mut data = manager.lock()?;
        if data.task.as_ref().is_some_and(|t| t.phase.active()) { return Err("Another update is in progress".into()); }
        task.revision = data.task.as_ref().map_or(1, |previous| previous.revision.saturating_add(1));
        journal::save(state, &task)?;
        data.task = Some(task.clone()); data.prepared = None;
        manager.cancel.store(false, Ordering::SeqCst);
    }
    let manager = manager.clone(); let state = state.clone();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&manager, &state, &prepared)))
            .unwrap_or_else(|_| Err("Updater worker stopped unexpectedly; recheck before retrying".into()));
        if result.is_err() {
            if let Ok(Some(task)) = manager.task() { wait_for_updater(&manager, &state, &task); }
        }
        updater::cleanup(&state, &prepared.view.id);
        drop(lease);
        if let Err(error) = finish(&manager, &state, result) { log::error!("Cannot save client update result: {error}"); }
    });
    Ok(task)
}

fn run(manager: &SoftwareManager, state: &LocalState, prepared: &plan::Prepared) -> Result<Option<String>, SoftwareError> {
    if manager.cancel.load(Ordering::SeqCst) { return Ok(None); }
    let updater = updater::prepare(state, prepared)?;
    if manager.cancel.load(Ordering::SeqCst) { return Ok(None); }
    // Preparation can download a script; revalidate again at the mutation boundary.
    plan::revalidate(state, prepared)?;
    {
        let mut data = manager.lock()?;
        let task = data.task.as_mut().ok_or("Update task missing")?;
        if manager.cancel.load(Ordering::SeqCst) { return Ok(None); }
        task.phase = Phase::RunningUpdater; task.cancellable = false;
        task.revision = task.revision.saturating_add(1);
        journal::save(state, task)?;
    }
    let outcome = match updater {
        updater::Updater::Process(mut command) => run_process(manager, state, prepared, &mut command),
        updater::Updater::Winget { id, scope, root } => {
            #[cfg(windows)] { super::winget::update(&id, &prepared.view.target_version, &scope, &root) }
            #[cfg(not(windows))] { let _ = (id, scope, root); Err("WinGet is unavailable on this operating system".into()) }
        }
    };
    change(manager, state, |task| {
        task.phase = Phase::Verifying;
        task.error = outcome.as_ref().err().map(|error| SoftwareError::from(error.as_str()).into_message());
    })?;
    let task = manager.task()?.ok_or("Update task missing")?;
    let verification = verify(state, &task);
    match (outcome, verification) {
        (Ok(()), result) => result.map(Some).map_err(SoftwareError::from),
        (Err(error), Ok(version)) => { change(manager, state, |t| t.actual_version = Some(version))?; Err(error.into()) }
        (Err(error), Err(verification)) => Err(format!("{error}\n{verification}").into()),
    }
}

fn run_process(manager: &SoftwareManager, state: &LocalState, prepared: &plan::Prepared, command: &mut std::process::Command) -> Result<(), String> {
    // Installers deliberately have no kill-on-close Job Object and no forced timeout.
    let mut child = spawn::recorded(command, |pid, birth|
        change(manager, state, |task| { task.process_id = Some(pid); task.process_birth = Some(birth); }))?;
    let receiver = process::drain(&mut child);
    let mut output = output::Output::default();
    loop {
        while let Ok(chunk) = receiver.try_recv() {
            let _ = change(manager, state, |task| output.push(chunk, task, &prepared.proxy));
        }
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(error) => { let _ = change(manager, state, |task| task.error = Some(SoftwareError::from(error.to_string()).into_message())); None }
        };
        if let Some(status) = status {
            let deadline = std::time::Instant::now() + Duration::from_millis(300);
            while std::time::Instant::now() < deadline {
                if let Ok(chunk) = receiver.recv_timeout(Duration::from_millis(30)) {
                    let _ = change(manager, state, |task| output.push(chunk, task, &prepared.proxy));
                }
            }
            change(manager, state, |task| output.finish(task, &prepared.proxy))?;
            return if status.success() { Ok(()) } else { Err(format!("Original updater exited with {status}; see operation output")) };
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn verify(state: &LocalState, task: &UpdateTask) -> Result<String, String> {
    let actual = super::discovery::inspect(task.plan.app, &task.plan.launcher)?;
    let version = actual.version.ok_or("The updated executable could not report its version")?;
    if actual.id != task.plan.installation_id || actual.manager != task.plan.manager {
        return Err("Installation ownership or location changed; inspect the original manager".into());
    }
    let target = semver::Version::parse(&task.plan.target_version).map_err(|e| e.to_string())?;
    let installed = semver::Version::parse(&version).map_err(|e| e.to_string())?;
    if (task.plan.exact_target && installed != target) || (!task.plan.exact_target && installed < target) {
        return Err(format!("Version verification failed: expected {}, found {version}", task.plan.target_version));
    }
    let policy = crate::commands::client_settings::software_policy::read(state, task.plan.app);
    if policy.issue.is_some() || !plan::within_policy(&version, &policy) { return Err("Updated version does not satisfy the current policy".into()); }
    Ok(version)
}

fn finish(manager: &SoftwareManager, state: &LocalState, result: Result<Option<String>, SoftwareError>) -> Result<(), String> {
    change(manager, state, |task| {
        match result {
            Ok(Some(version)) => { task.phase = Phase::Completed; task.actual_version = Some(version); task.error = None; }
            Ok(None) => { task.phase = Phase::Cancelled; task.error = None; }
            Err(error) => { task.phase = if task.phase == Phase::Preparing { Phase::Failed } else { Phase::NeedsAttention }; task.error = Some(error.into_message()); }
        }
        task.cancellable = false; task.finished_at = Some(chrono::Utc::now().to_rfc3339());
    })
}

fn change(manager: &SoftwareManager, state: &LocalState, mutate: impl FnOnce(&mut UpdateTask)) -> Result<(), String> {
    let mut data = manager.lock()?;
    let task = data.task.as_mut().ok_or("Update task missing")?;
    mutate(task);
    task.revision = task.revision.saturating_add(1);
    journal::save(state, task).inspect_err(|error| {
        task.error = Some(SoftwareError::from(format!("Cannot save update progress: {}", process::scrub(error))).into_message());
    })
}

pub fn cancel(manager: &SoftwareManager, id: &str) -> Result<UpdateTask, String> {
    let data = manager.lock()?;
    let task = data.task.as_ref().filter(|task| task.id == id).ok_or("Update task missing")?;
    if task.cancellable { manager.cancel.store(true, Ordering::SeqCst); }
    Ok(task.clone())
}
