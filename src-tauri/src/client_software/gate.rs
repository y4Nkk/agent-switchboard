use asb_core::contracts::AppKind;
use std::{collections::HashMap, sync::{Mutex, OnceLock}};

#[derive(Default)]
struct Gate { owner: Option<ExclusiveOwner>, launches: HashMap<AppKind, usize> }
#[derive(Clone, Copy, PartialEq, Eq)]
enum ExclusiveOwner { ClientUpdate(AppKind), ApplicationUpdate, CodexHistory }
fn gate() -> &'static Mutex<Gate> { static GATE: OnceLock<Mutex<Gate>> = OnceLock::new(); GATE.get_or_init(Mutex::default) }

pub struct LaunchLease(AppKind);
pub fn launch(app: AppKind) -> Result<LaunchLease, String> {
    let mut gate = gate().lock().map_err(|_| "Client launch gate unavailable")?;
    if gate.owner == Some(ExclusiveOwner::ClientUpdate(app)) || (app == AppKind::Codex && gate.owner == Some(ExclusiveOwner::CodexHistory)) {
        return Err("A client operation is in progress; wait before starting a session".into());
    }
    *gate.launches.entry(app).or_default() += 1;
    Ok(LaunchLease(app))
}
impl Drop for LaunchLease {
    fn drop(&mut self) {
        if let Ok(mut gate) = gate().lock() { if let Some(count) = gate.launches.get_mut(&self.0) { *count = count.saturating_sub(1); } }
    }
}

pub struct ExclusiveLease;
pub fn update(app: AppKind) -> Result<ExclusiveLease, String> {
    let mut gate = gate().lock().map_err(|_| "Client update gate unavailable")?;
    if gate.owner.is_some() { return Err("Another exclusive client operation is in progress".into()); }
    if gate.launches.get(&app).copied().unwrap_or(0) > 0 { return Err("Close this client's ASB-launched sessions and probes before updating".into()); }
    gate.owner = Some(ExclusiveOwner::ClientUpdate(app));
    Ok(ExclusiveLease)
}
impl Drop for ExclusiveLease { fn drop(&mut self) { if let Ok(mut gate) = gate().lock() { gate.owner = None; } } }

pub fn busy() -> bool { gate().lock().map(|g| g.owner.is_some()).unwrap_or(true) }

pub fn client_updating() -> bool { gate().lock().map(|g| matches!(g.owner, Some(ExclusiveOwner::ClientUpdate(_)))).unwrap_or(true) }
pub fn history_transferring() -> bool { gate().lock().map(|g| g.owner == Some(ExclusiveOwner::CodexHistory)).unwrap_or(true) }

pub fn application_update() -> Result<ExclusiveLease, String> {
    let mut gate = gate().lock().map_err(|_| "Software update gate unavailable")?;
    if gate.owner.is_some() { return Err("Another exclusive client operation is in progress".into()); }
    gate.owner = Some(ExclusiveOwner::ApplicationUpdate);
    Ok(ExclusiveLease)
}

pub fn history_transfer() -> Result<ExclusiveLease, String> {
    let mut gate = gate().lock().map_err(|_| "Client history gate unavailable")?;
    if gate.owner.is_some() { return Err("Another exclusive client operation is in progress".into()); }
    if gate.launches.get(&AppKind::Codex).copied().unwrap_or(0) > 0 {
        return Err("Close ASB-launched Codex sessions and probes before changing history".into());
    }
    gate.owner = Some(ExclusiveOwner::CodexHistory);
    Ok(ExclusiveLease)
}
