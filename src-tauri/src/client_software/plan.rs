use super::{discovery, releases, SoftwareError, types::{Comparison, Form, Installation, Manager, Owner, Policy, Software, UpdatePlan}};
use crate::{local_state::LocalState, outbound_proxy::OutboundProxySettings};
use chrono::{Duration, Utc};
mod identity;
use identity::identity_revision;

#[derive(Clone)]
pub struct Prepared {
    pub view: UpdatePlan,
    pub installation: Installation,
    pub policy_revision: String,
    pub identity_revision: String,
    pub proxy: OutboundProxySettings,
}

pub fn allowed(item: &Software) -> Option<String> {
    let install = &item.installation;
    if install.form != Form::Cli { return Some("hostManaged".into()); }
    if install.owner.is_none() { return Some("unmanaged".into()); }
    if item.policy.updates_disabled { return Some("updatesDisabled".into()); }
    if item.policy.issue.is_some() { return Some("policyUnavailable".into()); }
    let Some(release) = &item.available else { return Some("releaseUnavailable".into()); };
    let expired = chrono::DateTime::parse_from_rfc3339(&release.checked_at)
        .map(|time| Utc::now().signed_duration_since(time).num_seconds() > 3600).unwrap_or(true);
    if release.stale || expired { return Some("staleRelease".into()); }
    if item.comparison != Comparison::UpdateAvailable { return Some(match item.comparison {
        Comparison::Ahead => "ahead", Comparison::Current | Comparison::ChannelBehind => "current", _ => "versionUnavailable",
    }.into()); }
    if !within_policy(&release.version, &item.policy) { return Some("versionPolicy".into()); }
    if network_root(&install.root) { return Some("sharedInstallation".into()); }
    if administrator_required(&install.root) { return Some("administratorRequired".into()); }
    None
}

pub fn within_policy(version: &str, policy: &Policy) -> bool {
    let Ok(version) = semver::Version::parse(version) else { return false; };
    let Ok(minimum) = policy.minimum.as_deref().map(semver::Version::parse).transpose() else { return false; };
    let Ok(maximum) = policy.maximum.as_deref().map(semver::Version::parse).transpose() else { return false; };
    minimum.is_none_or(|v| version >= v) && maximum.is_none_or(|v| version <= v)
}

pub fn prepare(state: &LocalState, item: &Software) -> Result<Prepared, SoftwareError> {
    if let Some(reason) = allowed(item) { return Err(format!("Update unavailable: {reason}").into()); }
    let installation = item.installation.clone();
    crate::resource_monitor::client_processes::ensure_idle(&installation)?;
    let proxy = crate::outbound_proxy::read(state.root()).map_err(|e| e.to_string())?;
    super::updater::check_proxy(&installation, &proxy)?;
    let view = UpdatePlan { id: uuid::Uuid::new_v4().to_string(), installation_id: installation.id.clone(),
        app: installation.app, launcher: installation.launcher.clone(), manager: installation.manager,
        from_version: installation.version.clone().ok_or("Installed version unavailable")?,
        target_version: item.available.as_ref().ok_or("Release unavailable")?.version.clone(),
        exact_target: !matches!(installation.owner, Some(Owner::Homebrew { .. }))
            && !(installation.manager == Manager::Native && installation.app == asb_core::contracts::AppKind::Claude),
        channel: installation.channel.clone(),
        proxy_mode: if installation.manager == Manager::Winget { "wingetSystem".into() } else { format!("{:?}", proxy.mode).to_lowercase() },
        expires_at: (Utc::now() + Duration::minutes(5)).to_rfc3339() };
    let identity_revision = identity_revision(&installation)?;
    Ok(Prepared { view, installation, policy_revision: item.policy.revision.clone(), identity_revision, proxy })
}

pub fn revalidate(state: &LocalState, prepared: &Prepared) -> Result<Installation, SoftwareError> {
    let expiration = chrono::DateTime::parse_from_rfc3339(&prepared.view.expires_at).map_err(|e| e.to_string())?;
    if Utc::now() > expiration { return Err("Update preview expired; prepare it again".into()); }
    let mut now = discovery::inspect(prepared.view.app, &prepared.view.launcher)?;
    discovery::check_source(&mut now)?;
    let was = &prepared.installation;
    if now.id != was.id || now.executable != was.executable || now.owner != was.owner || now.version != was.version {
        return Err("Installation changed since preview; refresh and prepare again".into());
    }
    if identity_revision(&now)? != prepared.identity_revision { return Err("Installation files changed since preview; prepare again".into()); }
    let policy = crate::commands::client_settings::software_policy::read(state, now.app);
    if policy.revision != prepared.policy_revision || policy.issue.is_some() || policy.updates_disabled {
        return Err("Update policy changed since preview".into());
    }
    if crate::outbound_proxy::read(state.root()).map_err(|e| e.to_string())? != prepared.proxy {
        return Err("Network policy changed since preview".into());
    }
    if releases::compare(now.version.as_deref(), Some(&prepared.view.target_version)) != Comparison::UpdateAvailable {
        return Err("The planned operation would not upgrade this installation".into());
    }
    if network_root(&now.root) || administrator_required(&now.root) { return Err("Installation access changed; use its administrator".into()); }
    crate::resource_monitor::client_processes::ensure_idle(&now)?;
    Ok(now)
}

fn network_root(path: &std::path::Path) -> bool {
    let path = path.to_string_lossy();
    (path.starts_with(r"\\") && !path.starts_with(r"\\?\")) || path.starts_with(r"\\?\UNC\")
        || path.starts_with("/net/") || path.starts_with("/mnt/")
}

fn administrator_required(path: &std::path::Path) -> bool {
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        let Ok(metadata) = std::fs::metadata(path) else { return true; };
        metadata.uid() != unsafe { libc::geteuid() } || metadata.permissions().readonly()
    }
    #[cfg(windows)] {
        let target = super::identity::canonical_root(path);
        ["ProgramFiles", "ProgramFiles(x86)", "SystemRoot"].into_iter().filter_map(std::env::var_os)
            .map(std::path::PathBuf::from).any(|root| target.starts_with(&format!("{}/", super::identity::canonical_root(&root))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_bounds_are_inclusive_and_invalid_versions_are_not_update_targets() {
        let policy = Policy { minimum: Some("2.1.300".into()), maximum: Some("2.1.311".into()), ..Default::default() };
        assert!(within_policy("2.1.300", &policy));
        assert!(within_policy("2.1.311", &policy));
        assert!(!within_policy("2.1.299", &policy));
        assert!(!within_policy("2.1.312", &policy));
        assert!(!within_policy("latest", &policy));
    }
}
