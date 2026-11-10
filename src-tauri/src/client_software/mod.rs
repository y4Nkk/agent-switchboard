pub(crate) mod discovery;
pub(crate) mod gate;
pub(crate) mod identity;
mod error;
mod plan;
mod npm;
mod process;
mod releases;
mod task;
pub(crate) mod types;
mod updater;
#[cfg(windows)] mod winget;

use crate::{commands::error::CommandError, local_state::LocalState};
use plan::Prepared;
pub(crate) use error::SoftwareError;
pub(crate) use process::{command_path, output};
use types::{Form, Snapshot, Software, UpdatePlan, UpdateTask};
use std::{collections::HashMap, sync::{Arc, Mutex, MutexGuard, atomic::AtomicBool}};

#[derive(Default)]
struct Data {
    snapshot: Option<Snapshot>,
    prepared: Option<Prepared>,
    task: Option<UpdateTask>,
    initialized: bool,
    proxy_revision: Option<String>,
    selected: Vec<(asb_core::contracts::AppKind, std::path::PathBuf)>,
}

#[derive(Clone, Default)]
pub(crate) struct SoftwareManager {
    data: Arc<Mutex<Data>>,
    refreshing: Arc<Mutex<()>>,
    cache: Arc<releases::ReleaseCache>,
    cancel: Arc<AtomicBool>,
}

impl SoftwareManager {
    fn lock(&self) -> Result<MutexGuard<'_, Data>, String> { self.data.lock().map_err(|_| "Client software state unavailable".into()) }

    pub fn initialize(&self, state: &LocalState) -> Result<(), String> { task::initialize(self, state) }

    pub fn read(&self, state: &LocalState) -> Result<Snapshot, String> {
        self.initialize(state)?;
        let snapshot = self.lock()?.snapshot.clone();
        if let Some(mut snapshot) = snapshot {
            snapshot.task = self.lock()?.task.clone();
            return Ok(snapshot);
        }
        if gate::busy() {
            return Ok(Snapshot { revision: uuid::Uuid::new_v4().to_string(), installations: Vec::new(),
                issues: vec!["Installation discovery is paused during an exclusive client operation; refresh after it finishes".into()], task: self.task()? });
        }
        self.refresh(state, false, false)
    }

    pub fn refresh(&self, state: &LocalState, network: bool, force: bool) -> Result<Snapshot, String> {
        self.initialize(state)?;
        if gate::busy() { return Err("An exclusive client operation is in progress; refresh after it finishes".into()); }
        let _refresh = self.refreshing.lock().map_err(|_| "Client refresh unavailable")?;
        if gate::busy() { return Err("An exclusive client operation is in progress; refresh after it finishes".into()); }
        let proxy = crate::outbound_proxy::read(state.root()).map_err(|e| e.to_string())?;
        {
            let mut data = self.lock()?;
            if data.proxy_revision.as_ref() != Some(&proxy.revision) {
                self.cache.invalidate(); data.proxy_revision = Some(proxy.revision.clone());
            }
        }
        let selected = self.lock()?.selected.clone();
        let (installations, issues) = discovery::scan(&selected);
        let mut software: Vec<_> = installations.into_iter().map(|mut installation| {
            let policy = crate::commands::client_settings::software_policy::read(state, installation.app);
            if matches!(installation.manager, types::Manager::Native | types::Manager::Npm) { installation.channel = policy.channel.clone(); }
            Software { guide_url: types::installation_guide(&installation).into(), installation,
                official: None, available: None, comparison: types::Comparison::Unknown, policy,
                update_allowed: false, reason: None, release_error: None }
        }).collect();
        let errors = if network { self.fetch(&software, force, &proxy) } else { HashMap::new() };
        for item in &mut software {
            if item.installation.form != Form::Cli { item.reason = Some("hostManaged".into()); continue; }
            let (official, channel) = releases::sources(&item.installation);
            let (official, channel) = (official.key(), channel.map(|source| source.key()));
            item.official = self.cache.read(&official);
            item.available = channel.as_ref().and_then(|source| self.cache.read(source));
            item.release_error = errors.get(&official).or_else(|| channel.as_ref().and_then(|source| errors.get(source))).cloned();
            if let Some(release) = item.official.as_mut() { release.stale |= errors.contains_key(&official); }
            if let Some(release) = item.available.as_mut() { release.stale |= channel.as_ref().is_some_and(|source| errors.contains_key(source)); }
            item.comparison = releases::compare(item.installation.version.as_deref(), item.available.as_ref().map(|r| r.version.as_str()));
            if item.release_error.is_some() || item.available.as_ref().is_some_and(|r| r.stale) {
                item.comparison = types::Comparison::Unknown;
            } else if item.comparison == types::Comparison::Current && item.official.as_ref().is_some_and(|r| !r.stale)
                && releases::compare(item.available.as_ref().map(|r| r.version.as_str()), item.official.as_ref().map(|r| r.version.as_str())) == types::Comparison::UpdateAvailable {
                item.comparison = types::Comparison::ChannelBehind;
            }
            item.reason = plan::allowed(item);
            item.update_allowed = item.reason.is_none();
        }
        let mut data = self.lock()?;
        let snapshot = Snapshot { revision: uuid::Uuid::new_v4().to_string(), installations: software, issues, task: data.task.clone() };
        data.snapshot = Some(snapshot.clone());
        data.prepared = None;
        Ok(snapshot)
    }

    fn fetch(&self, items: &[Software], force: bool, proxy: &crate::outbound_proxy::OutboundProxySettings) -> HashMap<String, String> {
        let mut sources = Vec::new();
        for item in items.iter().filter(|item| item.installation.form == Form::Cli) {
            let (official, channel) = releases::sources(&item.installation);
            sources.push(official);
            sources.extend(channel);
        }
        sources.sort_by_key(releases::Source::key); sources.dedup_by_key(|source| source.key());
        let mut errors = HashMap::new();
        for batch in sources.chunks(2) {
            std::thread::scope(|scope| {
                let jobs: Vec<_> = batch.iter().map(|source| (source, scope.spawn(move || self.cache.fetch(source, force, proxy)))).collect();
                for (source, job) in jobs {
                    match job.join() {
                        Ok(Ok(_)) => {}
                        Ok(Err(error)) => { errors.insert(source.key(), error); }
                        Err(_) => { errors.insert(source.key(), "Release lookup stopped unexpectedly".into()); }
                    }
                }
            });
        }
        errors
    }

    pub fn prepare(&self, state: &LocalState, id: &str, revision: &str) -> Result<UpdatePlan, SoftwareError> {
        let _refresh = self.refreshing.lock().map_err(|_| "Client refresh unavailable")?;
        let item = {
            let data = self.lock()?;
            if data.task.as_ref().is_some_and(|task| task.phase.active()) { return Err("Another update is in progress".into()); }
            data.snapshot.as_ref().filter(|s| s.revision == revision).and_then(|s|
                s.installations.iter().find(|item| item.installation.id == id)).cloned().ok_or("Installation snapshot changed; refresh again")?
        };
        let prepared = plan::prepare(state, &item)?;
        let view = prepared.view.clone();
        self.lock()?.prepared = Some(prepared);
        Ok(view)
    }

    pub fn start(&self, state: &LocalState, id: &str) -> Result<UpdateTask, SoftwareError> { task::start(self, state, id) }
    pub fn select(&self, state: &LocalState, app: asb_core::contracts::AppKind, path: std::path::PathBuf) -> Result<Snapshot, String> {
        if gate::busy() { return Err("Wait for the current client operation to finish".into()); }
        discovery::validate_selected(app, &path)?;
        {
            let mut data = self.lock()?;
            if !data.selected.iter().any(|item| item == &(app, path.clone())) { data.selected.push((app, path)); }
        }
        self.refresh(state, true, false)
    }
    pub fn task(&self) -> Result<Option<UpdateTask>, String> { Ok(self.lock()?.task.clone()) }
    pub fn cancel(&self, id: &str) -> Result<UpdateTask, String> { task::cancel(self, id) }
}

pub(crate) fn error(failure: impl Into<SoftwareError>) -> CommandError {
    CommandError::from_localized("client-software-operation", failure.into().into_message())
}

pub(crate) fn blocks_exit() -> bool { gate::client_updating() }
