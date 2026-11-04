use asb_core::contracts::AppKind;
use std::{collections::HashMap, sync::{Mutex, OnceLock}};

#[derive(Default)]
struct Gate { update: Option<UpdateOwner>, launches: HashMap<AppKind, usize> }
#[derive(Clone, Copy, PartialEq, Eq)]
enum UpdateOwner { Client(AppKind), Application }
fn gate() -> &'static Mutex<Gate> { static GATE: OnceLock<Mutex<Gate>> = OnceLock::new(); GATE.get_or_init(Mutex::default) }

pub struct LaunchLease(AppKind);
pub fn launch(app: AppKind) -> Result<LaunchLease, String> {
    let mut gate = gate().lock().map_err(|_| "Client launch gate unavailable")?;
    if gate.update == Some(UpdateOwner::Client(app)) { return Err("This client is being updated; wait for verification before starting a session".into()); }
    *gate.launches.entry(app).or_default() += 1;
    Ok(LaunchLease(app))
}
impl Drop for LaunchLease {
    fn drop(&mut self) {
        if let Ok(mut gate) = gate().lock() { if let Some(count) = gate.launches.get_mut(&self.0) { *count = count.saturating_sub(1); } }
    }
}

pub struct UpdateLease;
pub fn update(app: AppKind) -> Result<UpdateLease, String> {
    let mut gate = gate().lock().map_err(|_| "Client update gate unavailable")?;
    if gate.update.is_some() { return Err("Another client update is in progress".into()); }
    if gate.launches.get(&app).copied().unwrap_or(0) > 0 { return Err("Close this client's ASB-launched sessions and probes before updating".into()); }
    gate.update = Some(UpdateOwner::Client(app));
    Ok(UpdateLease)
}
impl Drop for UpdateLease { fn drop(&mut self) { if let Ok(mut gate) = gate().lock() { gate.update = None; } } }

pub fn updating() -> bool { gate().lock().map(|g| g.update.is_some()).unwrap_or(true) }

pub fn client_updating() -> bool { gate().lock().map(|g| matches!(g.update, Some(UpdateOwner::Client(_)))).unwrap_or(true) }

pub fn application_update() -> Result<UpdateLease, String> {
    let mut gate = gate().lock().map_err(|_| "Software update gate unavailable")?;
    if gate.update.is_some() { return Err("Another software update is in progress; wait for it to finish".into()); }
    gate.update = Some(UpdateOwner::Application);
    Ok(UpdateLease)
}
