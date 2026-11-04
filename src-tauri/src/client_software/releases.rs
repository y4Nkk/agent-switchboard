use super::{types::{Comparison, Installation, Owner, Release}, process};
use crate::outbound_proxy::OutboundProxySettings;
use asb_core::contracts::AppKind;
use std::{collections::HashMap, io::Read, sync::Mutex, time::{Duration, Instant}};

#[derive(Default)]
pub struct ReleaseCache(Mutex<HashMap<String, (Instant, Release)>>);

#[derive(Clone)]
pub enum Source { Http(String), Npm { installation: String, owner: Owner, channel: String }, Winget(String) }

impl Source {
    pub fn key(&self) -> String {
        match self {
            Self::Http(url) => url.clone(),
            Self::Winget(id) => format!("winget:{id}"),
            Self::Npm { installation, owner, channel } => {
                let registry = match owner { Owner::Npm { registry, .. } => registry.as_deref().unwrap_or_default(), _ => "" };
                format!("npm:{installation}:{}:{channel}", asb_switch::sha256_hex(registry))
            }
        }
    }

    fn label(&self) -> String {
        match self { Self::Npm { owner, channel, .. } => format!("{} · {channel}", super::npm::source(owner)), _ => self.key() }
    }
}

impl ReleaseCache {
    pub fn invalidate(&self) { if let Ok(mut cache) = self.0.lock() { cache.clear(); } }
    pub fn read(&self, key: &str) -> Option<Release> {
        let cache = self.0.lock().ok()?;
        let (at, release) = cache.get(key)?;
        let mut release = release.clone();
        release.stale |= at.elapsed() > Duration::from_secs(3600);
        Some(release)
    }

    pub fn fetch(&self, source: &Source, force: bool, proxy: &OutboundProxySettings) -> Result<Release, String> {
        let key = source.key();
        if !force { if let Some(release) = self.read(&key).filter(|r| !r.stale) { return Ok(release); } }
        let version = query(source, proxy).and_then(|version| {
            let parsed = semver::Version::parse(&version).map_err(|_| "Release source returned an invalid version")?;
            if !parsed.pre.is_empty() { return Err("This release channel returned a prerelease; update it manually".into()); }
            Ok(version)
        }).inspect_err(|_| {
            if let Ok(mut cache) = self.0.lock() { if let Some((_, release)) = cache.get_mut(&key) { release.stale = true; } }
        })?;
        let release = Release { version, source: source.label(), checked_at: chrono::Utc::now().to_rfc3339(), stale: false };
        let mut cache = self.0.lock().map_err(|_| "Release cache unavailable")?;
        if cache.len() >= 128 && !cache.contains_key(&key) {
            let oldest = cache.iter().min_by_key(|(_, (at, _))| *at).map(|(key, _)| key.clone());
            if let Some(oldest) = oldest { cache.remove(&oldest); }
        }
        cache.insert(key, (Instant::now(), release.clone()));
        Ok(release)
    }
}

fn query(source: &Source, proxy: &OutboundProxySettings) -> Result<String, String> {
    match source {
        Source::Http(url) => parse(url, &download(url, 1_048_576)?),
        Source::Npm { owner, channel, .. } => {
            let metadata = super::npm::metadata(owner, channel, proxy)?;
            let Owner::Npm { package, .. } = owner else { return Err("npm identity unavailable".into()); };
            if metadata["name"].as_str() != Some(package) { return Err("npm returned a different package identity".into()); }
            metadata["version"].as_str().map(str::to_string).ok_or("npm returned no version for this channel".into())
        }
        Source::Winget(id) => {
            #[cfg(windows)] { super::winget::latest(id) }
            #[cfg(not(windows))] { let _ = id; Err("WinGet is available only on Windows".into()) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_use_semantic_order_and_never_propose_downgrades() {
        assert_eq!(compare(Some("0.9.0"), Some("0.10.0")), Comparison::UpdateAvailable);
        assert_eq!(compare(Some("0.168.0-alpha.1"), Some("0.166.0")), Comparison::Ahead);
        assert_eq!(compare(Some("2.1.318"), Some("2.1.311")), Comparison::Ahead);
        assert_eq!(compare(Some("2.1.311"), Some("2.1.311")), Comparison::Current);
        assert_eq!(compare(Some("unknown"), Some("2.1.311")), Comparison::Unknown);
        assert_eq!(compare(None, Some("2.1.311")), Comparison::Unknown);
    }

    #[test]
    fn each_official_source_preserves_its_channel_contract() {
        assert_eq!(parse("https://releases.openai.com/codex/channels/latest", br#"{"tag_name":"rust-v0.166.0"}"#).unwrap(), "0.166.0");
        assert_eq!(parse("https://downloads.claude.ai/claude-code-releases/stable", b"2.1.311\n").unwrap(), "2.1.311");
        assert_eq!(parse("https://formulae.brew.sh/api/cask/claude-code.json", br#"{"version":"2.1.311"}"#).unwrap(), "2.1.311");
    }
}

pub fn sources(item: &Installation) -> (Source, Option<Source>) {
    let official = Source::Http(match item.app {
        AppKind::Codex => "https://releases.openai.com/codex/channels/latest",
        AppKind::Claude => "https://downloads.claude.ai/claude-code-releases/latest",
    }.to_string());
    let channel = match &item.owner {
        Some(owner @ Owner::Npm { registry: Some(_), .. }) => Some(Source::Npm { installation: item.id.clone(), owner: owner.clone(), channel: item.channel.clone() }),
        Some(Owner::Npm { registry: None, .. }) => None,
        Some(Owner::Homebrew { cask, .. }) => Some(Source::Http(format!("https://formulae.brew.sh/api/cask/{cask}.json"))),
        Some(Owner::Winget { package, .. }) => Some(Source::Winget(package.clone())),
        Some(Owner::Native) if item.app == AppKind::Claude => Some(Source::Http(format!("https://downloads.claude.ai/claude-code-releases/{}", item.channel))),
        Some(Owner::Native) => Some(official.clone()),
        None => None,
    };
    (official, channel)
}

pub fn download(source: &str, limit: u64) -> Result<Vec<u8>, String> {
    let url = source.split('#').next().unwrap_or(source);
    let client = crate::outbound_proxy::apply_blocking(reqwest::blocking::Client::builder())
        .timeout(Duration::from_secs(20)).connect_timeout(Duration::from_secs(8))
        .user_agent(concat!("AgentSwitchboard/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::limited(4)).build().map_err(|e| process::scrub(&e.to_string()))?;
    let response = client.get(url).send().and_then(|r| r.error_for_status()).map_err(|e| process::scrub(&e.to_string()))?;
    let mut bytes = Vec::new();
    response.take(limit + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit { return Err("Official response exceeds size limit".into()); }
    Ok(bytes)
}

fn parse(source: &str, bytes: &[u8]) -> Result<String, String> {
    if source.starts_with("https://downloads.claude.ai/") {
        return String::from_utf8(bytes.to_vec()).map(|v| v.trim().into()).map_err(|e| e.to_string());
    }
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let raw = if source.starts_with("https://releases.openai.com/") {
        value["tag_name"].as_str().and_then(|v| v.strip_prefix("rust-v"))
    } else { value["version"].as_str() };
    raw.map(str::to_string).ok_or_else(|| "Release source has no version for this channel".into())
}

pub fn compare(installed: Option<&str>, available: Option<&str>) -> Comparison {
    let Some((installed, available)) = installed.zip(available)
        .and_then(|(a, b)| Some((semver::Version::parse(a).ok()?, semver::Version::parse(b).ok()?))) else { return Comparison::Unknown; };
    match installed.cmp(&available) {
        std::cmp::Ordering::Less => Comparison::UpdateAvailable,
        std::cmp::Ordering::Equal => Comparison::Current,
        std::cmp::Ordering::Greater => Comparison::Ahead,
    }
}
